//! Named shared-memory frame ring (Windows) with a seqlock for tear-free reads.
//!
//! Layout (little-endian):
//!
//! | offset | size | field |
//! |---|---|---|
//! | 0 | 8 | magic |
//! | 8 | 4 | version |
//! | 12 | 4 | capacity (bytes) |
//! | 16 | 8 | sequence (odd while writing) |
//! | 24 | 4 | width |
//! | 28 | 4 | height |
//! | 32 | 4 | length |
//! | 36 | 28 | reserved |
//! | 64 | capacity | RGBA8 pixels |
//!
//! Producers write metadata and pixels between two sequence increments; readers
//! retry while the sequence is odd or changes mid-copy. The ring is bounded by
//! [`MAX_FRAME_BYTES`](crate::frames::MAX_FRAME_BYTES).

pub const MAGIC: u64 = 0x4852_4D46_5241_4D45; // "HRMFRAME"
pub const VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 64;
const PAGE_SIZE: usize = 4096;
const SEQ_OFFSET: usize = 16;
const WIDTH_OFFSET: usize = 24;
const HEIGHT_OFFSET: usize = 28;
const LEN_OFFSET: usize = 32;

#[cfg(windows)]
mod windows_impl {
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::atomic::{AtomicU64, Ordering};

    use hr_core::{AppError, AppResult};

    use super::{
        HEADER_SIZE, HEIGHT_OFFSET, LEN_OFFSET, MAGIC, PAGE_SIZE, SEQ_OFFSET, VERSION, WIDTH_OFFSET,
    };
    use crate::frames::{MAX_FRAME_BYTES, validate_frame};
    use crate::state::FrameInfo;

    type Handle = *mut c_void;

    const PAGE_READWRITE: u32 = 0x04;
    const FILE_MAP_ALL_ACCESS: u32 = 0x000F_001F;
    const ERROR_ALREADY_EXISTS: u32 = 183;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileMappingW(
            file: Handle,
            attributes: *mut c_void,
            protect: u32,
            size_high: u32,
            size_low: u32,
            name: *const u16,
        ) -> Handle;
        fn OpenFileMappingW(access: u32, inherit: i32, name: *const u16) -> Handle;
        fn MapViewOfFile(
            mapping: Handle,
            access: u32,
            offset_high: u32,
            offset_low: u32,
            bytes: usize,
        ) -> *mut c_void;
        fn UnmapViewOfFile(base: *const c_void) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
        fn GetLastError() -> u32;
    }

    pub struct SharedFrameRing {
        name: String,
        mapping: Handle,
        base: *mut u8,
        capacity: u32,
    }

    // The mapping handle and view are stable for the lifetime of the ring; all
    // accesses are synchronised by the seqlock.
    unsafe impl Send for SharedFrameRing {}
    unsafe impl Sync for SharedFrameRing {}

    fn wide(name: &str) -> Vec<u16> {
        name.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn last_error(context: &str) -> AppError {
        let code = unsafe { GetLastError() };
        AppError::platform(format!("{context} failed (Windows error {code})"))
    }

    impl SharedFrameRing {
        pub fn create(name: &str) -> AppResult<Self> {
            validate_name(name)?;
            let capacity = MAX_FRAME_BYTES as u32;
            let total = HEADER_SIZE + round_up(capacity as usize, PAGE_SIZE);
            let wide_name = wide(name);
            let handle = unsafe {
                CreateFileMappingW(
                    usize::MAX as Handle,
                    ptr::null_mut(),
                    PAGE_READWRITE,
                    0,
                    total as u32,
                    wide_name.as_ptr(),
                )
            };
            if handle.is_null() {
                return Err(last_error("CreateFileMappingW"));
            }
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                unsafe {
                    CloseHandle(handle);
                }
                return Err(AppError::conflict(format!(
                    "frame ring '{name}' already exists"
                )));
            }
            let base =
                unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, total) } as *mut u8;
            if base.is_null() {
                let error = last_error("MapViewOfFile");
                unsafe {
                    CloseHandle(handle);
                }
                return Err(error);
            }
            unsafe {
                ptr::write_unaligned(base.cast::<u64>(), MAGIC);
                ptr::write_unaligned(base.add(8).cast::<u32>(), VERSION);
                ptr::write_unaligned(base.add(12).cast::<u32>(), capacity);
                seq(base).store(0, Ordering::Release);
            }
            Ok(Self {
                name: name.to_string(),
                mapping: handle,
                base,
                capacity,
            })
        }

        pub fn open(name: &str) -> AppResult<Self> {
            validate_name(name)?;
            let wide_name = wide(name);
            let handle = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, wide_name.as_ptr()) };
            if handle.is_null() {
                return Err(last_error("OpenFileMappingW"));
            }
            let base = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, 0) } as *mut u8;
            if base.is_null() {
                let error = last_error("MapViewOfFile");
                unsafe {
                    CloseHandle(handle);
                }
                return Err(error);
            }
            let magic = unsafe { ptr::read_unaligned(base.cast::<u64>()) };
            let version = unsafe { ptr::read_unaligned(base.add(8).cast::<u32>()) };
            let capacity = unsafe { ptr::read_unaligned(base.add(12).cast::<u32>()) };
            if magic != MAGIC || version != VERSION || capacity as usize > MAX_FRAME_BYTES {
                unsafe {
                    UnmapViewOfFile(base.cast());
                    CloseHandle(handle);
                }
                return Err(AppError::validation("the frame ring header is invalid"));
            }
            Ok(Self {
                name: name.to_string(),
                mapping: handle,
                base,
                capacity,
            })
        }

        pub fn name(&self) -> &str {
            &self.name
        }

        pub fn capacity(&self) -> u32 {
            self.capacity
        }

        pub fn publish(&self, width: u32, height: u32, rgba: &[u8]) -> AppResult<u64> {
            validate_frame(width, height, rgba.len())?;
            let len = u32::try_from(rgba.len())
                .map_err(|_| AppError::validation("frame is too large"))?;
            if len > self.capacity {
                return Err(AppError::validation("frame exceeds the ring capacity"));
            }
            let sequence = seq(self.base);
            let value = sequence.fetch_add(1, Ordering::AcqRel);
            unsafe {
                ptr::write_unaligned(self.base.add(WIDTH_OFFSET).cast::<u32>(), width);
                ptr::write_unaligned(self.base.add(HEIGHT_OFFSET).cast::<u32>(), height);
                ptr::write_unaligned(self.base.add(LEN_OFFSET).cast::<u32>(), len);
                ptr::copy_nonoverlapping(rgba.as_ptr(), self.base.add(HEADER_SIZE), rgba.len());
            }
            sequence.fetch_add(1, Ordering::Release);
            Ok(value + 1)
        }

        pub fn read(&self) -> AppResult<Option<crate::frames::Frame>> {
            let sequence = seq(self.base);
            for _ in 0..16 {
                let before = sequence.load(Ordering::Acquire);
                if before == 0 {
                    return Ok(None);
                }
                if before % 2 == 1 {
                    std::hint::spin_loop();
                    continue;
                }
                let width =
                    unsafe { ptr::read_unaligned(self.base.add(WIDTH_OFFSET).cast::<u32>()) };
                let height =
                    unsafe { ptr::read_unaligned(self.base.add(HEIGHT_OFFSET).cast::<u32>()) };
                let len = unsafe { ptr::read_unaligned(self.base.add(LEN_OFFSET).cast::<u32>()) }
                    as usize;
                if len > self.capacity as usize {
                    return Err(AppError::validation(
                        "the frame ring reported an invalid length",
                    ));
                }
                let mut rgba = vec![0u8; len];
                unsafe {
                    ptr::copy_nonoverlapping(self.base.add(HEADER_SIZE), rgba.as_mut_ptr(), len);
                }
                let after = sequence.load(Ordering::Acquire);
                if before == after {
                    validate_frame(width, height, rgba.len())?;
                    return Ok(Some(crate::frames::Frame {
                        seq: before / 2,
                        width,
                        height,
                        rgba,
                    }));
                }
                std::hint::spin_loop();
            }
            Err(AppError::unavailable(
                "the frame ring is being written continuously",
            ))
        }

        pub fn info(&self) -> AppResult<Option<FrameInfo>> {
            let sequence = seq(self.base);
            let before = sequence.load(Ordering::Acquire);
            if before == 0 || before % 2 == 1 {
                return Ok(None);
            }
            let width = unsafe { ptr::read_unaligned(self.base.add(WIDTH_OFFSET).cast::<u32>()) };
            let height = unsafe { ptr::read_unaligned(self.base.add(HEIGHT_OFFSET).cast::<u32>()) };
            let after = sequence.load(Ordering::Acquire);
            if before != after {
                return Ok(None);
            }
            Ok(Some(FrameInfo {
                seq: before / 2,
                width,
                height,
            }))
        }
    }

    impl std::fmt::Debug for SharedFrameRing {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("SharedFrameRing")
                .field("name", &self.name)
                .field("capacity", &self.capacity)
                .finish_non_exhaustive()
        }
    }

    impl Drop for SharedFrameRing {
        fn drop(&mut self) {
            unsafe {
                if !self.base.is_null() {
                    UnmapViewOfFile(self.base.cast());
                    self.base = ptr::null_mut();
                }
                if !self.mapping.is_null() {
                    CloseHandle(self.mapping);
                    self.mapping = ptr::null_mut();
                }
            }
        }
    }

    fn seq(base: *mut u8) -> &'static AtomicU64 {
        // SAFETY: the header is page-aligned and zero-initialised by the OS
        // mapping, and the sequence lives for the lifetime of the mapping.
        unsafe { &*(base.add(SEQ_OFFSET).cast::<AtomicU64>()) }
    }

    fn round_up(value: usize, alignment: usize) -> usize {
        value.div_ceil(alignment) * alignment
    }

    fn validate_name(name: &str) -> AppResult<()> {
        if name.trim().is_empty() || name.len() > 200 {
            return Err(AppError::validation("invalid shared-memory ring name"));
        }
        if !name.starts_with("Local\\") {
            return Err(AppError::validation(
                "shared-memory ring names must use the Local\\ prefix",
            ));
        }
        if name.contains('\0') {
            return Err(AppError::validation("ring name contains a NUL byte"));
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use windows_impl::SharedFrameRing;

#[cfg(not(windows))]
#[derive(Debug)]
pub struct SharedFrameRing;

#[cfg(not(windows))]
use crate::frames::MAX_FRAME_BYTES;

#[cfg(not(windows))]
impl SharedFrameRing {
    pub fn create(_name: &str) -> AppResult<Self> {
        Err(AppError::unsupported(
            "named frame rings are only available on Windows; the heap transport is used instead",
        ))
    }

    pub fn open(_name: &str) -> AppResult<Self> {
        Err(AppError::unsupported(
            "named frame rings are only available on Windows",
        ))
    }

    pub fn name(&self) -> &str {
        ""
    }

    pub fn capacity(&self) -> u32 {
        0
    }

    pub fn publish(&self, _width: u32, _height: u32, _rgba: &[u8]) -> AppResult<u64> {
        Err(AppError::unsupported("shared frames are unavailable"))
    }

    pub fn read(&self) -> AppResult<Option<crate::frames::Frame>> {
        Ok(None)
    }

    pub fn info(&self) -> AppResult<Option<FrameInfo>> {
        Ok(None)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use super::*;
    use crate::frames::{FrameTransport, MAX_FRAME_BYTES};

    fn unique_name(prefix: &str) -> String {
        format!("Local\\HRemoteTest-{prefix}-{}", ulid::Ulid::new())
    }

    #[test]
    fn shared_ring_round_trips_between_handles() {
        let name = unique_name("roundtrip");
        let writer = SharedFrameRing::create(&name).unwrap();
        let reader = SharedFrameRing::open(&name).unwrap();
        assert_eq!(writer.capacity(), MAX_FRAME_BYTES as u32);
        assert!(reader.info().unwrap().is_none());
        let pixels = vec![7u8; 2 * 2 * 4];
        let seq = writer.publish(2, 2, &pixels).unwrap();
        assert_eq!(seq, 1);
        let frame = reader.read().unwrap().unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.rgba, pixels);
        assert_eq!(reader.info().unwrap().unwrap().width, 2);
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let name = unique_name("duplicate");
        let _ring = SharedFrameRing::create(&name).unwrap();
        assert!(SharedFrameRing::create(&name).is_err());
        assert!(SharedFrameRing::open("not-namespaced").is_err());
    }

    #[test]
    fn invalid_frames_are_rejected() {
        let name = unique_name("invalid");
        let ring = SharedFrameRing::create(&name).unwrap();
        assert!(ring.publish(0, 0, &[]).is_err());
        assert!(ring.publish(1, 1, &[0; 3]).is_err());
    }

    #[test]
    fn readers_never_observe_torn_frames() {
        let name = unique_name("torn");
        let writer = Arc::new(SharedFrameRing::create(&name).unwrap());
        let reader = Arc::new(SharedFrameRing::open(&name).unwrap());
        let producer = {
            let writer = Arc::clone(&writer);
            thread::spawn(move || {
                for value in 1u8..=64 {
                    let mut pixels = vec![value; 64 * 64 * 4];
                    for alpha in pixels.iter_mut().skip(3).step_by(4) {
                        *alpha = 255;
                    }
                    writer.publish(64, 64, &pixels).unwrap();
                }
            })
        };
        let mut reads = 0;
        while !producer.is_finished() || reads < 8 {
            if let Ok(Some(frame)) = reader.read() {
                reads += 1;
                let first = frame.rgba[0];
                assert!(
                    frame
                        .rgba
                        .chunks_exact(4)
                        .all(|pixel| pixel == [first, first, first, 255]),
                    "torn frame detected"
                );
            }
            if reads > 10_000 {
                break;
            }
        }
        producer.join().unwrap();
        assert!(reads > 0);
    }

    #[test]
    fn transport_reports_the_ring_name_on_windows() {
        let name = unique_name("transport");
        let transport = FrameTransport::create(&name).unwrap();
        assert_eq!(transport.name(), Some(name.as_str()));
        transport.publish(1, 1, &[0, 1, 2, 255]).unwrap();
        assert_eq!(transport.read().unwrap().unwrap().rgba, vec![0, 1, 2, 255]);
    }
}
