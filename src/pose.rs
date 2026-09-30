//! Pose estimation: 33 BlazePose-compatible landmarks from RGB frames.
//!
//! Default build ships a deterministic synthetic estimator for demos/tests.
//! Enable the `onnx` feature and pass `--model path/to/blazepose.onnx` for
//! real ONNX Runtime inference. Optionally pass `--detector` (or place
//! `pose_detection.onnx` next to the landmark model) for MediaPipe-style ROI.

use crate::biomechanics::{Keypoint, Landmark, PoseFrame};
use crate::camera::Frame;
use anyhow::{bail, Result};
use std::time::Instant;

/// Trait implemented by all pose backends.
///
/// Not `Send` so backends can hold non-thread-safe session state; the estimator
/// lives on the same dedicated pipeline thread as the camera.
pub trait PoseEstimator {
    fn estimate(&mut self, frame: &Frame, pipeline_start: Instant) -> Result<PoseFrame>;
}

/// Synthetic side-view runner that oscillates ankles for cadence demos.
pub struct SyntheticPose {
    cadence_hz: f64,
}

impl Default for SyntheticPose {
    fn default() -> Self {
        Self { cadence_hz: 1.5 } // ~180 SPM (both feet)
    }
}

impl PoseEstimator for SyntheticPose {
    fn estimate(&mut self, frame: &Frame, pipeline_start: Instant) -> Result<PoseFrame> {
        let t = frame.timestamp.duration_since(pipeline_start).as_secs_f64();
        let w = frame.width as f32;
        let h = frame.height as f32;

        let phase = t * std::f64::consts::TAU * self.cadence_hz;
        let left_ankle_y = h * (0.72 + 0.10 * phase.sin() as f32);
        let right_ankle_y = h * (0.72 + 0.10 * (phase + std::f64::consts::PI).sin() as f32);

        let cx = w * 0.50;
        let mut landmarks = vec![Keypoint::new(0.0, 0.0, 0.0); 33];

        set(
            &mut landmarks,
            Landmark::LeftShoulder,
            cx - 20.0,
            h * 0.22,
            0.95,
        );
        set(
            &mut landmarks,
            Landmark::RightShoulder,
            cx + 20.0,
            h * 0.22,
            0.95,
        );
        set(&mut landmarks, Landmark::LeftHip, cx - 18.0, h * 0.42, 0.95);
        set(&mut landmarks, Landmark::RightHip, cx + 18.0, h * 0.42, 0.95);

        set(
            &mut landmarks,
            Landmark::LeftKnee,
            cx - 22.0,
            h * 0.58,
            0.9,
        );
        set(
            &mut landmarks,
            Landmark::LeftAnkle,
            cx - 20.0 + 15.0 * (phase.cos() as f32),
            left_ankle_y,
            0.9,
        );
        set(
            &mut landmarks,
            Landmark::LeftHeel,
            cx - 22.0,
            left_ankle_y + 4.0,
            0.85,
        );
        set(
            &mut landmarks,
            Landmark::LeftFootIndex,
            cx - 12.0,
            left_ankle_y + 2.0,
            0.85,
        );

        set(
            &mut landmarks,
            Landmark::RightKnee,
            cx + 22.0,
            h * 0.58,
            0.9,
        );
        set(
            &mut landmarks,
            Landmark::RightAnkle,
            cx + 20.0 + 15.0 * ((phase + std::f64::consts::PI).cos() as f32),
            right_ankle_y,
            0.9,
        );
        set(
            &mut landmarks,
            Landmark::RightHeel,
            cx + 22.0,
            right_ankle_y + 4.0,
            0.85,
        );
        set(
            &mut landmarks,
            Landmark::RightFootIndex,
            cx + 12.0,
            right_ankle_y + 2.0,
            0.85,
        );

        Ok(PoseFrame {
            frame_index: frame.frame_index,
            timestamp_secs: t,
            landmarks,
            roi: None,
        })
    }
}

fn set(landmarks: &mut [Keypoint], id: Landmark, x: f32, y: f32, conf: f32) {
    landmarks[id as usize] = Keypoint::new(x, y, conf);
}

/// Build a pose estimator. Uses ONNX when `model_path` is set and the
/// `onnx` feature is enabled; otherwise falls back to synthetic poses.
pub fn open_estimator(
    model_path: Option<&str>,
    detector_path: Option<&str>,
) -> Result<Box<dyn PoseEstimator>> {
    match model_path {
        Some(path) => {
            #[cfg(feature = "onnx")]
            {
                return Ok(Box::new(OnnxPose::load(path, detector_path)?));
            }
            #[cfg(not(feature = "onnx"))]
            {
                let _ = (path, detector_path);
                bail!(
                    "ONNX model requested but the `onnx` feature is not enabled. \
                     Rebuild with `--features onnx` or omit `--model`."
                );
            }
        }
        None => Ok(Box::new(SyntheticPose::default())),
    }
}

#[cfg(feature = "onnx")]
mod onnx_backend {
    use super::*;
    use ort::session::Session;
    use ort::value::TensorRef;
    use std::path::Path;

    const DETECT_SIZE: usize = 224;
    const LANDMARK_SIZE: usize = 256;
    const LANDMARK_STRIDE: usize = 5;
    const BODY_LANDMARKS: usize = 33;
    const DETECT_EVERY: u32 = 20;
    const SCORE_THRESH: f32 = 0.5;

    /// Oriented square ROI in original frame pixels (MediaPipe-style).
    #[derive(Clone, Copy, Debug)]
    struct OrientedRoi {
        cx: f32,
        cy: f32,
        size: f32,
        /// Radians; 0 = upright (hip→shoulder along −Y).
        angle: f32,
    }

    impl OrientedRoi {
        fn clamp_to_frame(mut self, fw: f32, fh: f32) -> Self {
            self.size = self.size.clamp(32.0, fw.max(fh) * 1.5);
            self.cx = self.cx.clamp(0.0, fw);
            self.cy = self.cy.clamp(0.0, fh);
            self
        }

        fn ema(self, other: Self, alpha: f32) -> Self {
            let sin = self.angle.sin() * (1.0 - alpha) + other.angle.sin() * alpha;
            let cos = self.angle.cos() * (1.0 - alpha) + other.angle.cos() * alpha;
            Self {
                cx: self.cx * (1.0 - alpha) + other.cx * alpha,
                cy: self.cy * (1.0 - alpha) + other.cy * alpha,
                size: self.size * (1.0 - alpha) + other.size * alpha,
                angle: sin.atan2(cos),
            }
        }

        /// Map a point in model pixel space → original frame pixels.
        fn model_to_frame(&self, mx: f32, my: f32, model_size: f32) -> (f32, f32) {
            let half = model_size * 0.5;
            let s = self.size / model_size;
            let lx = (mx - half) * s;
            let ly = (my - half) * s;
            let (ca, sa) = (self.angle.cos(), self.angle.sin());
            (self.cx + lx * ca - ly * sa, self.cy + lx * sa + ly * ca)
        }
    }

    pub struct OnnxPose {
        detector: Option<Session>,
        detector_input: String,
        landmark: Session,
        landmark_input: String,
        anchors: Vec<[f32; 2]>,
        roi: Option<OrientedRoi>,
        frames_since_detect: u32,
    }

    impl OnnxPose {
        pub fn load(landmark_path: &str, detector_path: Option<&str>) -> Result<Self> {
            let landmark = Session::builder()?
                .commit_from_file(landmark_path)
                .map_err(|e| anyhow::anyhow!("load landmark ONNX: {e}"))?;
            let landmark_input = landmark
                .inputs()
                .first()
                .map(|i| i.name().to_string())
                .unwrap_or_else(|| "input_1".into());

            let detector_file = resolve_detector_path(landmark_path, detector_path);
            let (detector, detector_input) = if let Some(det_path) = detector_file {
                let session = Session::builder()?
                    .commit_from_file(&det_path)
                    .map_err(|e| anyhow::anyhow!("load detector ONNX ({det_path}): {e}"))?;
                let name = session
                    .inputs()
                    .first()
                    .map(|i| i.name().to_string())
                    .unwrap_or_else(|| "input_1".into());
                tracing::info!("pose detector loaded from {det_path}");
                (Some(session), name)
            } else {
                tracing::warn!(
                    "no pose detector found; falling back to full-frame letterbox (jittery). \
                     Pass --detector models/pose_detection.onnx or run ./scripts/fetch-model.sh"
                );
                (None, String::new())
            };

            Ok(Self {
                detector,
                detector_input,
                landmark,
                landmark_input,
                anchors: generate_anchors(),
                roi: None,
                frames_since_detect: DETECT_EVERY,
            })
        }

        fn detect_person(&mut self, frame: &Frame) -> Result<Option<OrientedRoi>> {
            let Some(session) = self.detector.as_mut() else {
                return Ok(None);
            };
            let (input, _) = preprocess_letterbox_full(frame, DETECT_SIZE)?;

            let shape = [1i64, DETECT_SIZE as i64, DETECT_SIZE as i64, 3];
            let outputs = session.run(ort::inputs![
                self.detector_input.as_str() => TensorRef::from_array_view((shape, input.as_slice()))?
            ])?;

            let mut coords: Option<Vec<f32>> = None;
            let mut scores: Option<Vec<f32>> = None;
            for (_name, value) in outputs.iter() {
                if let Ok((shape, data)) = value.try_extract_tensor::<f32>() {
                    let shape: Vec<i64> = shape.iter().copied().collect();
                    if shape.len() == 3 && shape[1] == 2254 && shape[2] == 12 {
                        coords = Some(data.to_vec());
                    } else if data.len() == 2254 {
                        scores = Some(data.to_vec());
                    }
                }
            }

            let (Some(coords), Some(scores)) = (coords, scores) else {
                bail!("detector outputs missing (expected 2254×12 coords + 2254 scores)");
            };

            let fw = frame.width as f32;
            let fh = frame.height as f32;
            let scale = DETECT_SIZE as f32;

            let mut best_i = None;
            let mut best_score = SCORE_THRESH;
            for (i, &raw) in scores.iter().enumerate().take(self.anchors.len()) {
                let s = sigmoid_if_logit(raw);
                if s > best_score {
                    best_score = s;
                    best_i = Some(i);
                }
            }

            let Some(i) = best_i else {
                return Ok(None);
            };

            let base = i * 12;
            let anchor = self.anchors[i];
            let mut kps = [[0.0f32; 2]; 4];
            for k in 0..4 {
                let ox = coords[base + 4 + k * 2];
                let oy = coords[base + 4 + k * 2 + 1];
                kps[k][0] = (ox / scale + anchor[0]) * fw;
                kps[k][1] = (oy / scale + anchor[1]) * fh;
            }

            // kp0=hip, kp1=scale, kp2=shoulder (MediaPipe alignment points).
            let hip = kps[0];
            let scale_kp = kps[1];
            let shoulder = kps[2];
            let dist = ((hip[0] - scale_kp[0]).hypot(hip[1] - scale_kp[1])).max(1.0);
            let size = (2.0 * dist * 1.25).max(64.0);
            let cx = (hip[0] + shoulder[0]) * 0.5;
            let cy = (hip[1] + shoulder[1]) * 0.5;
            let dx = shoulder[0] - hip[0];
            let dy = shoulder[1] - hip[1];
            // Rotate so hip→shoulder aligns with image −Y (upright torso).
            let angle = dx.atan2(-dy);
            let roi = OrientedRoi {
                cx,
                cy,
                size,
                angle,
            };
            tracing::debug!(best_score, ?roi, "detector oriented ROI");
            Ok(Some(roi.clamp_to_frame(fw, fh)))
        }
    }

    fn resolve_detector_path(landmark_path: &str, explicit: Option<&str>) -> Option<String> {
        if let Some(p) = explicit {
            return Some(p.to_string());
        }
        let parent = Path::new(landmark_path).parent().unwrap_or_else(|| Path::new("."));
        let candidate = parent.join("pose_detection.onnx");
        if candidate.is_file() {
            Some(candidate.to_string_lossy().into_owned())
        } else {
            None
        }
    }

    impl PoseEstimator for OnnxPose {
        fn estimate(&mut self, frame: &Frame, pipeline_start: Instant) -> Result<PoseFrame> {
            let fw = frame.width as f32;
            let fh = frame.height as f32;

            let need_detect = self.detector.is_some()
                && (self.roi.is_none() || self.frames_since_detect >= DETECT_EVERY);
            if need_detect {
                if let Some(det) = self.detect_person(frame)? {
                    self.roi = Some(match self.roi {
                        Some(prev) => prev.ema(det, 0.45).clamp_to_frame(fw, fh),
                        None => det.clamp_to_frame(fw, fh),
                    });
                    self.frames_since_detect = 0;
                } else {
                    self.frames_since_detect = self.frames_since_detect.saturating_add(1);
                }
            } else {
                self.frames_since_detect = self.frames_since_detect.saturating_add(1);
            }

            let (input, meta) = match self.roi {
                Some(roi) => preprocess_oriented_roi(frame, roi, LANDMARK_SIZE)?,
                None => preprocess_letterbox_full(frame, LANDMARK_SIZE)?,
            };

            let shape = [1i64, LANDMARK_SIZE as i64, LANDMARK_SIZE as i64, 3];
            let outputs = self.landmark.run(ort::inputs![
                self.landmark_input.as_str() => TensorRef::from_array_view((shape, input.as_slice()))?
            ])?;

            let mut landmarks = None;
            let mut pose_presence = 1.0f32;
            for (name, value) in outputs.iter() {
                if let Ok((_shape, data)) = value.try_extract_tensor::<f32>() {
                    if data.len() == 1 {
                        pose_presence = sigmoid_if_logit(data[0]);
                    } else if data.len() >= BODY_LANDMARKS * LANDMARK_STRIDE && landmarks.is_none()
                    {
                        landmarks = Some(parse_blazepose_landmarks(data, fw, fh, &meta)?);
                    }
                    let _ = name;
                }
            }

            let mut landmarks = landmarks.ok_or_else(|| {
                anyhow::anyhow!(
                    "no BlazePose landmark tensor (expected ≥{} floats)",
                    BODY_LANDMARKS * LANDMARK_STRIDE
                )
            })?;

            if pose_presence < 0.4 {
                for kp in &mut landmarks {
                    kp.confidence *= pose_presence;
                }
            }

            if let Some(tracked) = roi_from_landmarks(&landmarks, fw, fh) {
                self.roi = Some(match self.roi {
                    Some(prev) => prev.ema(tracked, 0.25).clamp_to_frame(fw, fh),
                    None => tracked.clamp_to_frame(fw, fh),
                });
            }

            Ok(PoseFrame {
                frame_index: frame.frame_index,
                timestamp_secs: frame.timestamp.duration_since(pipeline_start).as_secs_f64(),
                landmarks,
                roi: self.roi.map(|r| crate::biomechanics::DetectorRoi {
                    cx: r.cx,
                    cy: r.cy,
                    size: r.size,
                    angle_rad: r.angle,
                }),
            })
        }
    }

    fn generate_anchors() -> Vec<[f32; 2]> {
        let mut anchors = Vec::with_capacity(2254);
        for stride in [8usize, 16, 32, 32, 32] {
            let grid = DETECT_SIZE / stride;
            for r in 0..grid {
                for c in 0..grid {
                    let ax = (c as f32 + 0.5) / grid as f32;
                    let ay = (r as f32 + 0.5) / grid as f32;
                    anchors.push([ax, ay]);
                    anchors.push([ax, ay]);
                }
            }
        }
        debug_assert_eq!(anchors.len(), 2254);
        anchors
    }

    enum CropMeta {
        /// Full-frame letterbox (axis-aligned).
        Letterbox {
            scale: f32,
            pad_x: f32,
            pad_y: f32,
            model_size: f32,
        },
        /// Oriented ROI affine warp.
        Oriented { roi: OrientedRoi, model_size: f32 },
    }

    fn preprocess_oriented_roi(
        frame: &Frame,
        roi: OrientedRoi,
        model_size: usize,
    ) -> Result<(Vec<f32>, CropMeta)> {
        let src_w = frame.width as usize;
        let src_h = frame.height as usize;
        let ms = model_size as f32;
        let mut canvas = vec![0f32; model_size * model_size * 3];

        for v in 0..model_size {
            for u in 0..model_size {
                let (sx, sy) = roi.frame_to_model_inv_sample(u as f32 + 0.5, v as f32 + 0.5, ms);
                let out_i = (v * model_size + u) * 3;
                if sx < 0.0 || sy < 0.0 || sx >= src_w as f32 - 1.0 || sy >= src_h as f32 - 1.0 {
                    continue;
                }
                let x0 = sx.floor() as usize;
                let y0 = sy.floor() as usize;
                let x1 = (x0 + 1).min(src_w - 1);
                let y1 = (y0 + 1).min(src_h - 1);
                let fx = sx - x0 as f32;
                let fy = sy - y0 as f32;
                for c in 0..3 {
                    let p00 = frame.data[(y0 * src_w + x0) * 3 + c] as f32;
                    let p10 = frame.data[(y0 * src_w + x1) * 3 + c] as f32;
                    let p01 = frame.data[(y1 * src_w + x0) * 3 + c] as f32;
                    let p11 = frame.data[(y1 * src_w + x1) * 3 + c] as f32;
                    let top = p00 + (p10 - p00) * fx;
                    let bot = p01 + (p11 - p01) * fx;
                    canvas[out_i + c] = (top + (bot - top) * fy) / 127.5 - 1.0;
                }
            }
        }

        Ok((
            canvas,
            CropMeta::Oriented {
                roi,
                model_size: ms,
            },
        ))
    }

    impl OrientedRoi {
        /// For each output pixel (u,v), where to sample in the source frame.
        fn frame_to_model_inv_sample(&self, u: f32, v: f32, model_size: f32) -> (f32, f32) {
            // u,v in model space → frame (same as model_to_frame).
            self.model_to_frame(u, v, model_size)
        }
    }

    fn preprocess_letterbox_full(frame: &Frame, model_size: usize) -> Result<(Vec<f32>, CropMeta)> {
        let src_w = frame.width as usize;
        let src_h = frame.height as usize;
        let scale = (model_size as f32 / src_w as f32).min(model_size as f32 / src_h as f32);
        let new_w = ((src_w as f32 * scale).round() as usize)
            .max(1)
            .min(model_size);
        let new_h = ((src_h as f32 * scale).round() as usize)
            .max(1)
            .min(model_size);
        let pad_x = (model_size - new_w) / 2;
        let pad_y = (model_size - new_h) / 2;

        let mut canvas = vec![0f32; model_size * model_size * 3];
        for dy in 0..new_h {
            let sy = ((dy as f32 + 0.5) / scale - 0.5).clamp(0.0, (src_h - 1) as f32);
            let y0 = sy.floor() as usize;
            let y1 = (y0 + 1).min(src_h - 1);
            let fy = sy - y0 as f32;
            for dx in 0..new_w {
                let sx = ((dx as f32 + 0.5) / scale - 0.5).clamp(0.0, (src_w - 1) as f32);
                let x0 = sx.floor() as usize;
                let x1 = (x0 + 1).min(src_w - 1);
                let fx = sx - x0 as f32;
                let out_i = ((pad_y + dy) * model_size + (pad_x + dx)) * 3;
                for c in 0..3 {
                    let p00 = frame.data[(y0 * src_w + x0) * 3 + c] as f32;
                    let p10 = frame.data[(y0 * src_w + x1) * 3 + c] as f32;
                    let p01 = frame.data[(y1 * src_w + x0) * 3 + c] as f32;
                    let p11 = frame.data[(y1 * src_w + x1) * 3 + c] as f32;
                    let top = p00 + (p10 - p00) * fx;
                    let bot = p01 + (p11 - p01) * fx;
                    let v = top + (bot - top) * fy;
                    canvas[out_i + c] = v / 127.5 - 1.0;
                }
            }
        }

        Ok((
            canvas,
            CropMeta::Letterbox {
                scale,
                pad_x: pad_x as f32,
                pad_y: pad_y as f32,
                model_size: model_size as f32,
            },
        ))
    }

    fn parse_blazepose_landmarks(
        data: &[f32],
        frame_w: f32,
        frame_h: f32,
        meta: &CropMeta,
    ) -> Result<Vec<Keypoint>> {
        if data.len() < BODY_LANDMARKS * LANDMARK_STRIDE {
            bail!(
                "expected at least {} pose floats, got {}",
                BODY_LANDMARKS * LANDMARK_STRIDE,
                data.len()
            );
        }

        let model_size = match meta {
            CropMeta::Letterbox { model_size, .. } | CropMeta::Oriented { model_size, .. } => {
                *model_size
            }
        };

        let sample = [0usize, 11, 12, 23, 24];
        let mut max_abs = 0.0f32;
        for &i in &sample {
            let base = i * LANDMARK_STRIDE;
            max_abs = max_abs.max(data[base].abs()).max(data[base + 1].abs());
        }
        let coords_are_pixels = max_abs > 1.5;

        let mut landmarks = Vec::with_capacity(BODY_LANDMARKS);
        for i in 0..BODY_LANDMARKS {
            let base = i * LANDMARK_STRIDE;
            let nx = data[base];
            let ny = data[base + 1];
            let visibility = sigmoid_if_logit(data.get(base + 3).copied().unwrap_or(1.0));
            let presence = sigmoid_if_logit(data.get(base + 4).copied().unwrap_or(visibility));
            let conf = visibility.min(presence).clamp(0.0, 1.0);

            let model_x = if coords_are_pixels {
                nx
            } else {
                nx * model_size
            };
            let model_y = if coords_are_pixels {
                ny
            } else {
                ny * model_size
            };

            let (x, y) = match meta {
                CropMeta::Letterbox {
                    scale,
                    pad_x,
                    pad_y,
                    ..
                } => {
                    let x = (model_x - pad_x) / scale;
                    let y = (model_y - pad_y) / scale;
                    (x, y)
                }
                CropMeta::Oriented { roi, model_size } => {
                    roi.model_to_frame(model_x, model_y, *model_size)
                }
            };

            landmarks.push(Keypoint::new(
                x.clamp(-frame_w * 0.05, frame_w * 1.05),
                y.clamp(-frame_h * 0.05, frame_h * 1.05),
                conf,
            ));
        }
        Ok(landmarks)
    }

    fn roi_from_landmarks(landmarks: &[Keypoint], fw: f32, fh: f32) -> Option<OrientedRoi> {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut n = 0;
        for kp in landmarks {
            if kp.confidence < 0.4 {
                continue;
            }
            min_x = min_x.min(kp.x);
            min_y = min_y.min(kp.y);
            max_x = max_x.max(kp.x);
            max_y = max_y.max(kp.y);
            n += 1;
        }
        if n < 6 {
            return None;
        }
        let bw = (max_x - min_x).max(1.0);
        let bh = (max_y - min_y).max(1.0);
        let side = bw.max(bh) * 1.35;
        let cx = (min_x + max_x) * 0.5;
        let cy = (min_y + max_y) * 0.5;

        // Angle from mid-hip → mid-shoulder when available.
        let l_hip = landmarks.get(23).filter(|k| k.confidence >= 0.4);
        let r_hip = landmarks.get(24).filter(|k| k.confidence >= 0.4);
        let l_sh = landmarks.get(11).filter(|k| k.confidence >= 0.4);
        let r_sh = landmarks.get(12).filter(|k| k.confidence >= 0.4);
        let angle = match (l_hip, r_hip, l_sh, r_sh) {
            (Some(lh), Some(rh), Some(ls), Some(rs)) => {
                let hx = (lh.x + rh.x) * 0.5;
                let hy = (lh.y + rh.y) * 0.5;
                let sx = (ls.x + rs.x) * 0.5;
                let sy = (ls.y + rs.y) * 0.5;
                (sx - hx).atan2(-(sy - hy))
            }
            _ => 0.0,
        };

        Some(
            OrientedRoi {
                cx,
                cy,
                size: side,
                angle,
            }
            .clamp_to_frame(fw, fh),
        )
    }

    fn sigmoid_if_logit(v: f32) -> f32 {
        if (0.0..=1.0).contains(&v) {
            v
        } else {
            1.0 / (1.0 + (-v.clamp(-100.0, 100.0)).exp())
        }
    }
}

#[cfg(feature = "onnx")]
pub use onnx_backend::OnnxPose;
