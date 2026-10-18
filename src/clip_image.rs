//! Bounded clipboard images, stored as PNG without adding an image dependency.
//! Windows DIB parsing lives here so untrusted headers can be tested headlessly.

use crate::clipboard::{b64_decode, b64_encode};
use serde::{Deserialize, Serialize};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipImage {
    pub width: u32,
    pub height: u32,
    pub png_b64: String,
}

fn dimensions_ok(w: u32, h: u32) -> bool {
    w > 0 && h > 0 && u64::from(w) * u64::from(h) <= MAX_IMAGE_PIXELS
}

impl ClipImage {
    pub fn from_png(bytes: &[u8]) -> Option<Self> {
        // Inspect dimensions before the PNG decoder allocates a pixel buffer.
        if bytes.len() > MAX_IMAGE_BYTES
            || bytes.get(..8)? != b"\x89PNG\r\n\x1a\n"
            || bytes.get(12..16)? != b"IHDR"
        {
            return None;
        }
        let w = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
        if !dimensions_ok(w, h) {
            return None;
        }
        let pm = Pixmap::decode_png(bytes).ok()?;
        Self::from_pixmap(&pm)
    }

    pub fn from_pixmap(pm: &Pixmap) -> Option<Self> {
        if !dimensions_ok(pm.width(), pm.height()) {
            return None;
        }
        let png = pm.encode_png().ok()?;
        if png.len() > MAX_IMAGE_BYTES {
            return None;
        }
        Some(Self {
            width: pm.width(),
            height: pm.height(),
            png_b64: b64_encode(&png),
        })
    }

    pub fn pixmap(&self) -> Option<Pixmap> {
        if !dimensions_ok(self.width, self.height)
            || self.png_b64.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4
        {
            return None;
        }
        let bytes = b64_decode(&self.png_b64)?;
        if bytes.get(..8)? != b"\x89PNG\r\n\x1a\n"
            || bytes.get(12..16)? != b"IHDR"
            || u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?) != self.width
            || u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?) != self.height
        {
            return None;
        }
        let pm = Pixmap::decode_png(&bytes).ok()?;
        (pm.width() == self.width && pm.height() == self.height).then_some(pm)
    }

    pub fn thumbnail(&self) -> Option<Pixmap> {
        let src = self.pixmap()?;
        let scale = (72.0 / src.width() as f32)
            .min(48.0 / src.height() as f32)
            .min(1.0);
        let mut dst = Pixmap::new(
            (src.width() as f32 * scale).round().max(1.0) as u32,
            (src.height() as f32 * scale).round().max(1.0) as u32,
        )?;
        dst.draw_pixmap(
            0,
            0,
            src.as_ref(),
            &PixmapPaint {
                quality: FilterQuality::Bilinear,
                ..Default::default()
            },
            Transform::from_scale(scale, scale),
            None,
        );
        Some(dst)
    }

    /// BITMAPINFOHEADER / V4 / V5, 24-bit RGB or 32-bit RGB/bitfields.
    /// DIB rows may be padded or bottom-up. No native handles are used here.
    pub fn from_dib(bytes: &[u8]) -> Option<Self> {
        let u16_at = |i| Some(u16::from_le_bytes(bytes.get(i..i + 2)?.try_into().ok()?));
        let u32_at = |i| Some(u32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?));
        let header = u32_at(0)? as usize;
        if !matches!(header, 40 | 108 | 124) || bytes.len() < header {
            return None;
        }
        let w = u32_at(4)? as i32;
        let h = u32_at(8)? as i32;
        let width = u32::try_from(w).ok()?;
        let height = h.checked_abs()? as u32;
        let bits = u16_at(14)?;
        let compression = u32_at(16)?;
        if !dimensions_ok(width, height)
            || u16_at(12)? != 1
            || !matches!((bits, compression), (24, 0) | (32, 0) | (32, 3))
        {
            return None;
        }
        let mut offset = header;
        let mut masks = [0x00ff0000u32, 0x0000ff00, 0x000000ff, 0];
        if compression == 3 {
            for (i, mask) in masks[..3].iter_mut().enumerate() {
                *mask = u32_at(40 + i * 4)?;
            }
            if header == 40 {
                offset += 12;
            } else {
                masks[3] = u32_at(52)?;
            }
            if masks[..3].contains(&0) {
                return None;
            }
            for (i, mask) in masks.iter().enumerate() {
                if *mask != 0 {
                    let normalized = mask >> mask.trailing_zeros();
                    if normalized & normalized.wrapping_add(1) != 0 {
                        return None;
                    }
                }
                if masks[..i].iter().any(|prior| prior & mask != 0) {
                    return None;
                }
            }
        }
        // Optional color table, including for high-color DIBs.
        offset = offset.checked_add((u32_at(32)? as usize).checked_mul(4)?)?;
        let stride = (width as usize * bits as usize).div_ceil(32) * 4;
        let end = offset.checked_add(stride.checked_mul(height as usize)?)?;
        bytes.get(offset..end)?;
        let mut pm = Pixmap::new(width, height)?;
        let component = |pixel: u32, mask: u32| -> u8 {
            if mask == 0 {
                return 255;
            }
            let shift = mask.trailing_zeros();
            let max = u64::from(mask >> shift);
            ((u64::from((pixel & mask) >> shift) * 255 + max / 2) / max) as u8
        };
        for y in 0..height as usize {
            let row = if h > 0 { height as usize - 1 - y } else { y };
            for x in 0..width as usize {
                let p = offset + row * stride + x * (bits as usize / 8);
                let (r, g, b, a) = if bits == 24 {
                    (bytes[p + 2], bytes[p + 1], bytes[p], 255)
                } else {
                    let pixel = u32_at(p)?;
                    (
                        component(pixel, masks[0]),
                        component(pixel, masks[1]),
                        component(pixel, masks[2]),
                        component(pixel, masks[3]),
                    )
                };
                let dst = &mut pm.data_mut()[(y * width as usize + x) * 4..][..4];
                for (d, c) in dst[..3].iter_mut().zip([r, g, b]) {
                    *d = ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
                }
                dst[3] = a;
            }
        }
        Self::from_pixmap(&pm)
    }

    /// Opaque 24-bit CF_DIB for apps without PNG support. Composite alpha on
    /// white; the registered PNG format preserves the original transparency.
    pub fn to_dib(&self) -> Option<Vec<u8>> {
        let pm = self.pixmap()?;
        let stride = (self.width as usize * 3).div_ceil(4) * 4;
        let mut out = vec![0u8; 40 + stride * self.height as usize];
        out[0..4].copy_from_slice(&40u32.to_le_bytes());
        out[4..8].copy_from_slice(&(self.width as i32).to_le_bytes());
        out[8..12].copy_from_slice(&(self.height as i32).to_le_bytes());
        out[12..14].copy_from_slice(&1u16.to_le_bytes());
        out[14..16].copy_from_slice(&24u16.to_le_bytes());
        out[20..24].copy_from_slice(&((stride * self.height as usize) as u32).to_le_bytes());
        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                let p = &pm.data()[(y * self.width as usize + x) * 4..][..4];
                let dst = 40 + (self.height as usize - 1 - y) * stride + x * 3;
                for (i, c) in [p[2], p[1], p[0]].into_iter().enumerate() {
                    out[dst + i] = c.saturating_add(255 - p[3]);
                }
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dib_roundtrip_handles_padding_and_bottom_up_rows() {
        let mut pm = Pixmap::new(3, 2).unwrap();
        pm.data_mut().copy_from_slice(&[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9,
            255,
        ]);
        let image = ClipImage::from_pixmap(&pm).unwrap();
        let dib = image.to_dib().unwrap();
        assert_eq!(
            ClipImage::from_dib(&dib).unwrap().pixmap().unwrap().data(),
            pm.data()
        );
        assert_eq!(
            ClipImage::from_png(&b64_decode(&image.png_b64).unwrap()),
            Some(image)
        );
    }

    #[test]
    fn rejects_truncated_and_huge_images_before_allocation() {
        let image = ClipImage::from_pixmap(&Pixmap::new(2, 2).unwrap()).unwrap();
        let dib = image.to_dib().unwrap();
        for n in 0..dib.len() {
            assert!(ClipImage::from_dib(&dib[..n]).is_none());
        }
        let mut huge = dib.clone();
        huge[4..8].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(ClipImage::from_dib(&huge).is_none());
        let mut png = b64_decode(&image.png_b64).unwrap();
        png[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(ClipImage::from_png(&png).is_none());
    }

    #[test]
    fn top_down_bgrx_is_opaque_and_bitfields_preserve_alpha() {
        let mut dib = vec![0u8; 128];
        dib[0..4].copy_from_slice(&124u32.to_le_bytes());
        dib[4..8].copy_from_slice(&1i32.to_le_bytes());
        dib[8..12].copy_from_slice(&(-1i32).to_le_bytes());
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        dib[124..128].copy_from_slice(&[30, 20, 10, 0]);
        assert_eq!(
            ClipImage::from_dib(&dib).unwrap().pixmap().unwrap().data(),
            &[10, 20, 30, 255]
        );
        dib[16..20].copy_from_slice(&3u32.to_le_bytes());
        for (i, mask) in [0xff0000u32, 0xff00, 0xff, 0xff000000].iter().enumerate() {
            dib[40 + i * 4..44 + i * 4].copy_from_slice(&mask.to_le_bytes());
        }
        dib[127] = 128;
        assert_eq!(
            ClipImage::from_dib(&dib).unwrap().pixmap().unwrap().data(),
            &[5, 10, 15, 128]
        );
        dib[40..44].copy_from_slice(&0xff00u32.to_le_bytes());
        assert!(ClipImage::from_dib(&dib).is_none(), "overlapping masks");
    }
}
