//! Camera capture optimized for high frame-rate side-view treadmill streams.
//!
//! Default build uses a synthetic demo generator so the pipeline runs without
//! hardware. Enable the `camera` feature for live capture via `nokhwa`.

use crate::demo;
use crate::view::CameraView;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A single captured video frame (RGB8, tightly packed).
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// RGB8 pixels; consumed by the ONNX backend when enabled.
    #[allow(dead_code)]
    pub data: Vec<u8>,
    pub frame_index: u64,
    #[allow(dead_code)]
    pub timestamp: Instant,
}

impl Frame {
    #[allow(dead_code)]
    pub fn byte_len(&self) -> usize {
        (self.width as usize) * (self.height as usize) * 3
    }
}

/// Capture settings targeting 60 FPS @ 720p.
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    #[allow(dead_code)]
    pub device_index: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 60,
            device_index: 0,
        }
    }
}

/// Trait for frame sources (live camera or synthetic).
///
/// Not `Send`: `nokhwa::Camera` is `!Send` on macOS, so the capture loop must
/// own the camera on a dedicated OS thread (see `main`).
pub trait FrameSource {
    fn next_frame(&mut self) -> Result<Option<Frame>>;
    fn config(&self) -> &CaptureConfig;
}

/// Synthetic demo runner with scripted gait phases (see `demo` module).
pub struct SyntheticCamera {
    config: CaptureConfig,
    frame_index: u64,
    started: Instant,
    period: Duration,
    next_due: Instant,
    view: Arc<Mutex<CameraView>>,
}

impl SyntheticCamera {
    pub fn new(config: CaptureConfig, view: Arc<Mutex<CameraView>>) -> Self {
        let period = Duration::from_secs_f64(1.0 / config.fps.max(1) as f64);
        let now = Instant::now();
        Self {
            config,
            frame_index: 0,
            started: now,
            period,
            next_due: now,
            view,
        }
    }
}

impl FrameSource for SyntheticCamera {
    fn next_frame(&mut self) -> Result<Option<Frame>> {
        let now = Instant::now();
        if now < self.next_due {
            std::thread::sleep(self.next_due.saturating_duration_since(now));
        }
        self.next_due += self.period;

        let t = self.frame_index as f64 / self.config.fps.max(1) as f64;
        let view = *self.view.lock().unwrap();
        let data = demo::render_frame(t, view, self.config.width, self.config.height);
        let frame = Frame {
            width: self.config.width,
            height: self.config.height,
            data,
            frame_index: self.frame_index,
            timestamp: self.started + Duration::from_secs_f64(t),
        };
        self.frame_index += 1;
        Ok(Some(frame))
    }

    fn config(&self) -> &CaptureConfig {
        &self.config
    }
}

/// Open the best available frame source for the given config.
pub fn open_capture(
    config: CaptureConfig,
    prefer_live: bool,
    view: Arc<Mutex<CameraView>>,
) -> Result<Box<dyn FrameSource>> {
    if prefer_live {
        #[cfg(feature = "camera")]
        {
            let _ = view;
            return Ok(Box::new(NokhwaCamera::open(config)?));
        }
        #[cfg(not(feature = "camera"))]
        {
            anyhow::bail!(
                "live camera requested but the `camera` feature is disabled.\n\
                 Rebuild with: cargo run --release --features camera,onnx -- --live …\n\
                 Or run the synthetic demo: cargo run --release -- --demo"
            );
        }
    }
    Ok(Box::new(SyntheticCamera::new(config, view)))
}

/// One row from `--list-cameras`.
#[derive(Debug, Clone)]
pub struct CameraDeviceInfo {
    pub index: String,
    pub name: String,
    pub description: String,
}

/// Enumerate cameras for the CLI device picker.
pub fn list_cameras() -> Result<Vec<CameraDeviceInfo>> {
    #[cfg(feature = "camera")]
    {
        live::list_cameras_impl()
    }
    #[cfg(not(feature = "camera"))]
    {
        anyhow::bail!(
            "camera listing requires the `camera` feature.\n\
             Rebuild with: cargo run --release --features camera -- --list-cameras"
        )
    }
}

#[cfg(feature = "camera")]
mod live {
    use super::*;
    use anyhow::{bail, Context};
    use nokhwa::pixel_format::RgbFormat;
    use nokhwa::query;
    use nokhwa::utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType};
    use nokhwa::Camera;

    pub fn list_cameras_impl() -> Result<Vec<CameraDeviceInfo>> {
        let infos = query(ApiBackend::Auto).map_err(|e| {
            anyhow::anyhow!("failed to query cameras: {e}\n{}", mac_permission_hint())
        })?;
        if infos.is_empty() {
            bail!("no cameras found.\n{}", mac_permission_hint());
        }
        Ok(infos
            .into_iter()
            .map(|info| CameraDeviceInfo {
                index: format!("{}", info.index()),
                name: info.human_name(),
                description: info.description().to_string(),
            })
            .collect())
    }

    fn mac_permission_hint() -> &'static str {
        "On macOS: System Settings → Privacy & Security → Camera — enable access for \
         Terminal (or iTerm / your IDE). Quit and reopen the app after granting permission. \
         Use `--list-cameras` to confirm the device index, then `--live --device N`."
    }

    fn map_open_error(err: impl std::fmt::Display, device: u32) -> anyhow::Error {
        let msg = err.to_string();
        let lower = msg.to_ascii_lowercase();
        if lower.contains("permission")
            || lower.contains("not authorized")
            || lower.contains("denied")
            || lower.contains("-10814")
            || lower.contains("could not")
        {
            anyhow::anyhow!(
                "could not open camera device {device}: {msg}\n{}",
                mac_permission_hint()
            )
        } else {
            anyhow::anyhow!(
                "could not open camera device {device}: {msg}\n\
                 Tip: run with `--list-cameras` to see available indexes.\n{}",
                mac_permission_hint()
            )
        }
    }

    pub struct NokhwaCamera {
        camera: Camera,
        config: CaptureConfig,
        frame_index: u64,
        started: Instant,
    }

    impl NokhwaCamera {
        pub fn open(config: CaptureConfig) -> Result<Self> {
            let index = CameraIndex::Index(config.device_index);
            let requested =
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
            let mut camera = Camera::new(index, requested)
                .map_err(|e| map_open_error(e, config.device_index))?;
            camera
                .set_resolution(nokhwa::utils::Resolution::new(config.width, config.height))
                .ok();
            camera.set_frame_rate(config.fps).ok();
            camera
                .open_stream()
                .map_err(|e| map_open_error(e, config.device_index))
                .context("start camera stream")?;
            Ok(Self {
                camera,
                config,
                frame_index: 0,
                started: Instant::now(),
            })
        }
    }

    impl FrameSource for NokhwaCamera {
        fn next_frame(&mut self) -> Result<Option<Frame>> {
            let buffer = self
                .camera
                .frame()
                .map_err(|e| map_open_error(e, self.config.device_index))?;
            let decoded = buffer.decode_image::<RgbFormat>().context("decode RGB")?;
            let frame = Frame {
                width: decoded.width(),
                height: decoded.height(),
                data: decoded.into_raw(),
                frame_index: self.frame_index,
                timestamp: self.started + self.started.elapsed(),
            };
            self.frame_index += 1;
            Ok(Some(frame))
        }

        fn config(&self) -> &CaptureConfig {
            &self.config
        }
    }
}

#[cfg(feature = "camera")]
pub use live::NokhwaCamera;
