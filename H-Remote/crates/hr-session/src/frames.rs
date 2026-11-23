//! Bounded frame stores used by the embedded engine.
//!
//! The engine always produces one latest frame; consumers never block the
//! producer. Frames larger than [`MAX_FRAME_BYTES`] are rejected so a malicious
//! or broken server cannot exhaust memory.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use hr_core::{AppError, AppResult};

use crate::state::FrameInfo;

pub const MAX_WIDTH: u32 = 4096;
pub const MAX_HEIGHT: u32 = 4096;
pub const MAX_FRAME_BYTES: usize = (MAX_WIDTH as usize) * (MAX_HEIGHT as usize) * 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8 pixels.
    pub rgba: Vec<u8>,
}

impl Frame {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> AppResult<Self> {
        validate_frame(width, height, rgba.len())?;
        Ok(Self {
            seq: 0,
            width,
            height,
            rgba,
        })
    }
}

pub fn validate_frame(width: u32, height: u32, len: usize) -> AppResult<()> {
    if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
        return Err(AppError::validation(format!(
            "invalid frame dimensions {width}x{height}"
        )));
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| AppError::validation("frame dimensions overflow"))?;
    if expected != len {
        return Err(AppError::validation(format!(
            "frame buffer has {len} bytes, expected {expected}"
        )));
    }
    if expected > MAX_FRAME_BYTES {
        return Err(AppError::validation(
            "frame is larger than the bounded maximum",
        ));
    }
    Ok(())
}

/// In-process latest-frame store. Used on non-Windows hosts and in tests.
#[derive(Debug, Default)]
pub struct HeapFrameStore {
    seq: AtomicU64,
    frame: RwLock<Option<Frame>>,
}

impl HeapFrameStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, width: u32, height: u32, rgba: &[u8]) -> AppResult<u64> {
        validate_frame(width, height, rgba.len())?;
        let seq = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
        let mut guard = self
            .frame
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        *guard = Some(Frame {
            seq,
            width,
            height,
            rgba: rgba.to_vec(),
        });
        Ok(seq)
    }

    pub fn read(&self) -> Option<Frame> {
        self.frame
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    pub fn info(&self) -> Option<FrameInfo> {
        self.frame
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .as_ref()
            .map(|frame| FrameInfo {
                seq: frame.seq,
                width: frame.width,
                height: frame.height,
            })
    }
}

/// Chooses the platform transport: a named shared-memory ring on Windows, a
/// bounded heap store elsewhere. Both expose the same latest-frame semantics.
#[derive(Debug)]
pub enum FrameTransport {
    Heap(Arc<HeapFrameStore>),
    Shared(crate::shared::SharedFrameRing),
}

impl FrameTransport {
    /// Creates a transport. `ring_name` is only used by the Windows backend.
    pub fn create(ring_name: &str) -> AppResult<Self> {
        #[cfg(windows)]
        {
            Ok(Self::Shared(crate::shared::SharedFrameRing::create(
                ring_name,
            )?))
        }
        #[cfg(not(windows))]
        {
            let _ = ring_name;
            Ok(Self::Heap(Arc::new(HeapFrameStore::new())))
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Heap(_) => None,
            Self::Shared(ring) => Some(ring.name()),
        }
    }

    pub fn publish(&self, width: u32, height: u32, rgba: &[u8]) -> AppResult<u64> {
        match self {
            Self::Heap(store) => store.publish(width, height, rgba),
            Self::Shared(ring) => ring.publish(width, height, rgba),
        }
    }

    pub fn read(&self) -> AppResult<Option<Frame>> {
        match self {
            Self::Heap(store) => Ok(store.read()),
            Self::Shared(ring) => ring.read(),
        }
    }

    pub fn info(&self) -> AppResult<Option<FrameInfo>> {
        match self {
            Self::Heap(store) => Ok(store.info()),
            Self::Shared(ring) => ring.info(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, value: u8) -> Vec<u8> {
        vec![value; (width * height * 4) as usize]
    }

    #[test]
    fn heap_store_round_trips_and_increments_sequence() {
        let store = HeapFrameStore::new();
        assert!(store.read().is_none());
        let first = store.publish(2, 2, &solid(2, 2, 1)).unwrap();
        let second = store.publish(2, 2, &solid(2, 2, 2)).unwrap();
        assert_eq!(first, 1);
        assert_eq!(second, 2);
        let frame = store.read().unwrap();
        assert_eq!((frame.seq, frame.width, frame.height), (2, 2, 2));
        assert_eq!(frame.rgba, solid(2, 2, 2));
        assert_eq!(store.info().unwrap().seq, 2);
    }

    #[test]
    fn invalid_frames_are_rejected_without_changing_the_last_frame() {
        let store = HeapFrameStore::new();
        store.publish(2, 2, &solid(2, 2, 9)).unwrap();
        assert!(store.publish(0, 2, &[]).is_err());
        assert!(store.publish(2, 2, &[0; 3]).is_err());
        assert!(store.publish(MAX_WIDTH + 1, 1, &[0; 4]).is_err());
        assert_eq!(store.read().unwrap().rgba, solid(2, 2, 9));
    }

    #[test]
    fn frame_new_validates_lengths() {
        assert!(Frame::new(1, 1, vec![0; 4]).is_ok());
        assert!(Frame::new(1, 1, vec![0; 3]).is_err());
        assert!(Frame::new(1, 1, vec![0; 8]).is_err());
    }

    #[test]
    fn unbounded_allocations_are_rejected() {
        let pixels = (MAX_FRAME_BYTES / 4) + 1;
        assert!(validate_frame(pixels as u32, 1, MAX_FRAME_BYTES + 4).is_err());
    }

    #[test]
    fn transport_falls_back_to_heap_off_windows() {
        let transport = FrameTransport::create("Local\\HRemoteTest-transport").unwrap();
        transport.publish(1, 1, &[1, 2, 3, 255]).unwrap();
        let frame = transport.read().unwrap().unwrap();
        assert_eq!(frame.rgba, vec![1, 2, 3, 255]);
        assert_eq!(transport.info().unwrap().unwrap().width, 1);
        if cfg!(not(windows)) {
            assert!(transport.name().is_none());
        }
    }
}
