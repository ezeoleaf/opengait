//! Throttled JPEG preview encoding for the WebSocket dashboard.

use crate::camera::Frame;
use anyhow::{Context, Result};
use base64::Engine;
use jpeg_encoder::{ColorType, Encoder};

/// Encode a downscaled JPEG preview as a raw base64 string (no data-URL prefix).
pub fn encode_preview_jpeg(frame: &Frame, max_width: u32, quality: u8) -> Result<String> {
    let src_w = frame.width.max(1);
    let src_h = frame.height.max(1);
    let scale = if src_w > max_width {
        max_width as f32 / src_w as f32
    } else {
        1.0
    };
    let out_w = ((src_w as f32 * scale).round() as u32).max(1);
    let out_h = ((src_h as f32 * scale).round() as u32).max(1);

    let mut rgb = vec![0u8; (out_w * out_h * 3) as usize];
    let sw = src_w as usize;
    let sh = src_h as usize;
    for y in 0..out_h as usize {
        let sy = ((y as f32 + 0.5) / scale - 0.5).clamp(0.0, (sh - 1) as f32) as usize;
        for x in 0..out_w as usize {
            let sx = ((x as f32 + 0.5) / scale - 0.5).clamp(0.0, (sw - 1) as f32) as usize;
            let si = (sy * sw + sx) * 3;
            let di = (y * out_w as usize + x) * 3;
            rgb[di] = frame.data[si];
            rgb[di + 1] = frame.data[si + 1];
            rgb[di + 2] = frame.data[si + 2];
        }
    }

    let mut buf = Vec::new();
    let enc = Encoder::new(&mut buf, quality.clamp(1, 100));
    enc.encode(&rgb, out_w as u16, out_h as u16, ColorType::Rgb)
        .context("jpeg encode")?;
    Ok(base64::engine::general_purpose::STANDARD.encode(buf))
}
