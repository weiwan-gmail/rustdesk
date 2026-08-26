use scrap::{ImageFormat, ImageRgb};
use std::{
    collections::HashMap,
    sync::RwLock,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct LatestFrame {
    pub width: usize,
    pub height: usize,
    pub format: String,
    pub bytes: Vec<u8>,
    pub updated_at_ms: u64,
}

impl LatestFrame {
    pub fn encode_png(&self) -> Result<Vec<u8>, String> {
        let rgba = to_rgba8888(&self.bytes, self.width, self.height, &self.format)?;
        let mut png = Vec::new();
        repng::encode(&mut png, self.width as u32, self.height as u32, &rgba)
            .map_err(|e| format!("png encode failed: {e}"))?;
        Ok(png)
    }

    pub fn encode_jpeg(&self, quality: u8) -> Result<Vec<u8>, String> {
        let rgba = to_rgba8888(&self.bytes, self.width, self.height, &self.format)?;
        let mut out = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
        encoder
            .encode(
                &rgba,
                self.width as u32,
                self.height as u32,
                image::ColorType::Rgba8,
            )
            .map_err(|e| format!("jpeg encode failed: {e}"))?;
        Ok(out)
    }
}

#[derive(Default)]
pub struct FrameStore {
    frames: RwLock<HashMap<(String, usize), LatestFrame>>,
}

impl FrameStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&self, session_id: &str, display: usize, rgb: &ImageRgb) {
        let bpp = match rgb.fmt {
            ImageFormat::Raw => 3,
            ImageFormat::ARGB | ImageFormat::ABGR => 4,
        };
        let align = rgb.align().max(1);
        let stride = (rgb.w * bpp + align - 1) & !(align - 1);
        let expected = stride * rgb.h;
        let mut bytes = rgb.raw.clone();
        if bytes.len() > expected {
            bytes.truncate(expected);
        }
        // Normalize to tightly packed rows for encoding.
        if bpp == 4 && stride != rgb.w * 4 && rgb.w > 0 && rgb.h > 0 {
            let mut packed = Vec::with_capacity(rgb.w * rgb.h * 4);
            for row in 0..rgb.h {
                let start = row * stride;
                let end = start + rgb.w * 4;
                if end <= bytes.len() {
                    packed.extend_from_slice(&bytes[start..end]);
                }
            }
            bytes = packed;
        }
        let format = match rgb.fmt {
            ImageFormat::ABGR => "abgr",
            ImageFormat::ARGB => "argb",
            ImageFormat::Raw => "raw",
        }
        .to_string();
        let updated_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let frame = LatestFrame {
            width: rgb.w,
            height: rgb.h,
            format,
            bytes,
            updated_at_ms,
        };
        if let Ok(mut guard) = self.frames.write() {
            guard.insert((session_id.to_string(), display), frame);
        }
    }

    pub fn get(&self, session_id: &str, display: usize) -> Option<LatestFrame> {
        self.frames
            .read()
            .ok()
            .and_then(|g| g.get(&(session_id.to_string(), display)).cloned())
    }

    pub fn remove_session(&self, session_id: &str) {
        if let Ok(mut guard) = self.frames.write() {
            guard.retain(|(sid, _), _| sid != session_id);
        }
    }
}

fn to_rgba8888(
    bytes: &[u8],
    width: usize,
    height: usize,
    format: &str,
) -> Result<Vec<u8>, String> {
    let need = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| "frame dimensions overflow".to_string())?;
    if bytes.len() < need {
        return Err(format!(
            "frame buffer too small: got {}, need {}",
            bytes.len(),
            need
        ));
    }
    match format {
        "abgr" => {
            // Memory layout treated as B,G,R,A -> R,G,B,A
            let mut out = Vec::with_capacity(need);
            for chunk in bytes[..need].chunks_exact(4) {
                out.push(chunk[2]);
                out.push(chunk[1]);
                out.push(chunk[0]);
                out.push(chunk[3]);
            }
            Ok(out)
        }
        "argb" => {
            let mut out = Vec::with_capacity(need);
            for chunk in bytes[..need].chunks_exact(4) {
                out.push(chunk[1]);
                out.push(chunk[2]);
                out.push(chunk[3]);
                out.push(chunk[0]);
            }
            Ok(out)
        }
        _ => Ok(bytes[..need].to_vec()),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use scrap::ImageRgb;

    #[test]
    fn encode_png_from_abgr_frame() {
        let store = FrameStore::new();
        let mut rgb = ImageRgb::new(ImageFormat::ABGR, 1);
        rgb.w = 2;
        rgb.h = 2;
        // B,G,R,A per pixel
        rgb.raw = vec![
            0, 0, 255, 255, // red
            0, 255, 0, 255, // green
            255, 0, 0, 255, // blue
            255, 255, 255, 255, // white
        ];
        store.update("s1", 0, &rgb);
        let frame = store.get("s1", 0).expect("frame");
        let png = frame.encode_png().expect("png");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
        let jpeg = frame.encode_jpeg(80).expect("jpeg");
        assert!(jpeg.len() > 10);
    }
}
