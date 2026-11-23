//! Bounded icon decode cache. Stored icons are base64 PNG/JPEG; decoded RGBA is
//! cached with both an entry and a byte cap so a large icon library cannot grow
//! memory without bound.

use std::collections::VecDeque;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hr_core::{AppError, AppResult};
use image::ImageFormat;

const MAX_ENCODED_BYTES: usize = 2 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIMENSION: u32 = 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct IconData {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
}

impl fmt::Debug for IconData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IconData")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.rgba.len())
            .finish()
    }
}

impl IconData {
    pub fn decode(base64_value: &str) -> AppResult<Self> {
        let encoded = base64_value
            .trim()
            .strip_prefix("data:")
            .and_then(|value| value.split_once(',').map(|(_, data)| data))
            .unwrap_or(base64_value.trim());
        if encoded.len() > MAX_ENCODED_BYTES {
            return Err(AppError::validation("icon is too large"));
        }
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| AppError::validation(format!("invalid icon base64: {error}")))?;
        if bytes.len() > MAX_DECODED_BYTES {
            return Err(AppError::validation("decoded icon is too large"));
        }
        let format = image::guess_format(&bytes)
            .map_err(|error| AppError::validation(format!("unknown icon format: {error}")))?;
        if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
            return Err(AppError::validation("icon must be PNG or JPEG"));
        }
        let image = image::load_from_memory_with_format(&bytes, format)
            .map_err(|error| AppError::validation(format!("cannot decode icon: {error}")))?;
        if image.width() > MAX_DIMENSION || image.height() > MAX_DIMENSION {
            return Err(AppError::validation(format!(
                "icon dimensions must be at most {MAX_DIMENSION}px"
            )));
        }
        let rgba = image.to_rgba8();
        Ok(Self {
            width: rgba.width(),
            height: rgba.height(),
            rgba: Arc::new(rgba.into_raw()),
        })
    }

    pub fn byte_len(&self) -> usize {
        self.rgba.len()
    }
}

struct CacheState {
    entries: VecDeque<(u64, Arc<IconData>)>,
    total_bytes: usize,
}

pub struct IconCache {
    state: Mutex<CacheState>,
    max_entries: usize,
    max_bytes: usize,
}

impl Default for IconCache {
    fn default() -> Self {
        Self::new(64, 32 * 1024 * 1024)
    }
}

impl IconCache {
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            state: Mutex::new(CacheState {
                entries: VecDeque::new(),
                total_bytes: 0,
            }),
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
        }
    }

    pub fn len(&self) -> usize {
        self.state
            .lock()
            .expect("icon cache poisoned")
            .entries
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get_or_decode(&self, base64_value: &str) -> AppResult<Arc<IconData>> {
        let key = hash_value(base64_value);
        {
            let state = self.state.lock().expect("icon cache poisoned");
            if let Some((_, data)) = state
                .entries
                .iter()
                .find(|(candidate, _)| *candidate == key)
            {
                return Ok(Arc::clone(data));
            }
        }

        let decoded = Arc::new(IconData::decode(base64_value)?);
        let mut state = self.state.lock().expect("icon cache poisoned");
        if let Some((_, existing)) = state
            .entries
            .iter()
            .find(|(candidate, _)| *candidate == key)
        {
            return Ok(Arc::clone(existing));
        }
        while !state.entries.is_empty()
            && (state.entries.len() >= self.max_entries
                || state.total_bytes + decoded.byte_len() > self.max_bytes)
        {
            if let Some((_, removed)) = state.entries.pop_front() {
                state.total_bytes = state.total_bytes.saturating_sub(removed.byte_len());
            }
        }
        if decoded.byte_len() > self.max_bytes {
            return Err(AppError::validation("icon is larger than the cache budget"));
        }
        state.total_bytes += decoded.byte_len();
        state.entries.push_back((key, Arc::clone(&decoded)));
        Ok(decoded)
    }

    pub fn clear(&self) {
        let mut state = self.state.lock().expect("icon cache poisoned");
        state.entries.clear();
        state.total_bytes = 0;
    }
}

pub fn hash_value(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Merges up to four icons into one square `.ico` file (P2-69).
pub fn merge_icons_ico(icons: &[IconData], size: u32) -> AppResult<Vec<u8>> {
    if icons.is_empty() {
        return Err(AppError::validation("no icons to merge"));
    }
    let size = size.clamp(16, 256);
    let columns = if icons.len() == 1 { 1 } else { 2 };
    let rows = icons.len().div_ceil(columns) as u32;
    let cell_width = size / columns as u32;
    let cell_height = size / rows;
    let mut canvas = image::RgbaImage::new(size, size);
    for (index, icon) in icons.iter().take(4).enumerate() {
        let source =
            image::RgbaImage::from_raw(icon.width, icon.height, icon.rgba.as_ref().clone())
                .ok_or_else(|| AppError::validation("invalid icon buffer"))?;
        let resized = image::imageops::resize(
            &source,
            cell_width,
            cell_height,
            image::imageops::FilterType::Lanczos3,
        );
        let x = (index as u32 % columns as u32) * cell_width;
        let y = (index as u32 / columns as u32) * cell_height;
        image::imageops::overlay(&mut canvas, &resized, i64::from(x), i64::from(y));
    }
    let image = ico::IconImage::from_rgba_data(size, size, canvas.into_raw());
    let entry = ico::IconDirEntry::encode(&image)
        .map_err(|error| AppError::internal(format!("cannot encode ico: {error}")))?;
    let mut directory = ico::IconDir::new(ico::ResourceType::Icon);
    directory.add_entry(entry);
    let mut output = Vec::new();
    directory
        .write(&mut output)
        .map_err(|error| AppError::internal(format!("cannot write ico: {error}")))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png_base64(width: u32, height: u32) -> String {
        let image = RgbaImage::from_pixel(width, height, Rgba([10, 20, 30, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("encode png");
        STANDARD.encode(bytes.into_inner())
    }

    #[test]
    fn decodes_png_and_data_urls() {
        let encoded = png_base64(2, 3);
        let icon = IconData::decode(&encoded).unwrap();
        assert_eq!((icon.width, icon.height), (2, 3));
        assert_eq!(icon.rgba.len(), 2 * 3 * 4);

        let data_url = format!("data:image/png;base64,{encoded}");
        assert_eq!(IconData::decode(&data_url).unwrap().width, 2);
    }

    #[test]
    fn rejects_invalid_and_oversized_inputs() {
        assert!(IconData::decode("not-base64").is_err());
        assert!(IconData::decode(&STANDARD.encode(b"not an image")).is_err());
        assert!(IconData::decode(&"A".repeat(MAX_ENCODED_BYTES + 1)).is_err());
    }

    #[test]
    fn cache_is_bounded_and_evicts_old_entries() {
        let cache = IconCache::new(2, 64 * 1024 * 1024);
        let a = png_base64(1, 1);
        let b = png_base64(2, 2);
        let c = png_base64(3, 3);
        let first = cache.get_or_decode(&a).unwrap();
        assert!(Arc::ptr_eq(&first, &cache.get_or_decode(&a).unwrap()));
        cache.get_or_decode(&b).unwrap();
        cache.get_or_decode(&c).unwrap();
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get_or_decode(&a).unwrap().width, 1);
    }

    #[test]
    fn cache_rejects_items_above_the_byte_budget() {
        let cache = IconCache::new(10, 8);
        let encoded = png_base64(4, 4);
        assert!(cache.get_or_decode(&encoded).is_err());
        assert!(cache.is_empty());
    }

    #[test]
    fn merged_icons_produce_a_valid_ico() {
        let icons = vec![
            IconData::decode(&png_base64(8, 8)).unwrap(),
            IconData::decode(&png_base64(8, 8)).unwrap(),
        ];
        let bytes = merge_icons_ico(&icons, 32).unwrap();
        let directory = ico::IconDir::read(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(directory.entries().len(), 1);
        assert_eq!(directory.entries()[0].width(), 32);
        assert_eq!(directory.entries()[0].height(), 32);
        assert!(merge_icons_ico(&[], 32).is_err());
    }
}
