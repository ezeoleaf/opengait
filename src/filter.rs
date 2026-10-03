//! One Euro Filter for landmark streams (Casiez et al., CHI 2012).
//!
//! Smooths jittery ONNX landmarks before biomechanics without adding much lag
//! during fast motion. Applied per-axis (x, y) for each of the 33 joints.

use crate::biomechanics::PoseFrame;

/// Tunable One Euro parameters (sensible defaults for ~30–60 FPS pose).
#[derive(Debug, Clone, Copy)]
pub struct OneEuroParams {
    /// Minimum cutoff frequency (Hz). Lower = smoother when still.
    pub min_cutoff: f32,
    /// Speed coefficient. Higher = less lag when moving fast.
    pub beta: f32,
    /// Cutoff for the derivative low-pass (Hz).
    pub d_cutoff: f32,
}

impl Default for OneEuroParams {
    fn default() -> Self {
        Self {
            min_cutoff: 1.0,
            beta: 0.007,
            d_cutoff: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LowPass {
    hat: Option<f32>,
}

impl LowPass {
    fn new() -> Self {
        Self { hat: None }
    }

    fn filter(&mut self, value: f32, alpha: f32) -> f32 {
        let out = match self.hat {
            Some(prev) => alpha * value + (1.0 - alpha) * prev,
            None => value,
        };
        self.hat = Some(out);
        out
    }

    fn reset(&mut self) {
        self.hat = None;
    }
}

#[derive(Debug, Clone)]
struct OneEuro1D {
    params: OneEuroParams,
    x_filter: LowPass,
    dx_filter: LowPass,
    last_time: Option<f64>,
    last_raw: Option<f32>,
}

impl OneEuro1D {
    fn new(params: OneEuroParams) -> Self {
        Self {
            params,
            x_filter: LowPass::new(),
            dx_filter: LowPass::new(),
            last_time: None,
            last_raw: None,
        }
    }

    fn reset(&mut self) {
        self.x_filter.reset();
        self.dx_filter.reset();
        self.last_time = None;
        self.last_raw = None;
    }

    fn filter(&mut self, value: f32, t: f64) -> f32 {
        let te = match self.last_time {
            Some(prev) if t > prev => (t - prev) as f32,
            _ => 1.0 / 60.0,
        }
        .max(1e-6);

        let dx = match self.last_raw {
            Some(prev) => (value - prev) / te,
            None => 0.0,
        };
        let edx = self
            .dx_filter
            .filter(dx, alpha(te, self.params.d_cutoff));
        let cutoff = self.params.min_cutoff + self.params.beta * edx.abs();
        let out = self.x_filter.filter(value, alpha(te, cutoff));
        self.last_time = Some(t);
        self.last_raw = Some(value);
        out
    }
}

fn alpha(te: f32, cutoff: f32) -> f32 {
    let tau = 1.0 / (2.0 * std::f32::consts::PI * cutoff.max(1e-6));
    1.0 / (1.0 + tau / te)
}

/// Per-landmark (x, y) One Euro state for a full BlazePose skeleton.
#[derive(Debug)]
pub struct LandmarkFilter {
    params: OneEuroParams,
    xy: Vec<(OneEuro1D, OneEuro1D)>,
    enabled: bool,
}

impl LandmarkFilter {
    pub fn new(params: OneEuroParams) -> Self {
        let xy = (0..33)
            .map(|_| (OneEuro1D::new(params), OneEuro1D::new(params)))
            .collect();
        Self {
            params,
            xy,
            enabled: true,
        }
    }

    pub fn disabled() -> Self {
        let mut f = Self::new(OneEuroParams::default());
        f.enabled = false;
        f
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn reset(&mut self) {
        for (fx, fy) in &mut self.xy {
            fx.reset();
            fy.reset();
        }
    }

    /// Filter landmark positions in place. Low-confidence joints are left raw
    /// and their filter state is reset so they do not pull neighbors.
    pub fn apply(&mut self, pose: &mut PoseFrame) {
        if !self.enabled {
            return;
        }
        if !pose.person_detected {
            self.reset();
            return;
        }
        let t = pose.timestamp_secs;
        for (i, kp) in pose.landmarks.iter_mut().enumerate() {
            if i >= self.xy.len() {
                break;
            }
            if kp.confidence < 0.35 {
                self.xy[i].0.reset();
                self.xy[i].1.reset();
                continue;
            }
            let (fx, fy) = &mut self.xy[i];
            kp.x = fx.filter(kp.x, t);
            kp.y = fy.filter(kp.y, t);
        }
        let _ = self.params;
    }
}

/// Convenience: clone landmarks through a one-shot filter (tests / offline).
#[cfg(test)]
pub fn filter_keypoints(
    filter: &mut LandmarkFilter,
    landmarks: &[crate::biomechanics::Keypoint],
    t: f64,
) -> Vec<crate::biomechanics::Keypoint> {
    let mut pose = PoseFrame {
        frame_index: 0,
        timestamp_secs: t,
        landmarks: landmarks.to_vec(),
        roi: None,
        pose_presence: 1.0,
        person_detected: true,
    };
    filter.apply(&mut pose);
    pose.landmarks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biomechanics::Keypoint;

    #[test]
    fn smooths_jitter_but_tracks_step() {
        let mut f = LandmarkFilter::new(OneEuroParams {
            min_cutoff: 1.0,
            beta: 0.01,
            d_cutoff: 1.0,
        });
        let mut prev = 0.0f32;
        // Quiet frame train around 100 with noise.
        for i in 0..30 {
            let noisy = 100.0 + if i % 2 == 0 { 3.0 } else { -3.0 };
            let mut lm = vec![Keypoint::new(0.0, 0.0, 0.0); 33];
            lm[0] = Keypoint::new(noisy, 50.0, 1.0);
            let out = filter_keypoints(&mut f, &lm, i as f64 * 0.016);
            prev = out[0].x;
        }
        // After settling, output should be closer to 100 than the ±3 spikes.
        assert!((prev - 100.0).abs() < 2.5, "got {prev}");

        // Fast step: filter should move toward the new value.
        let mut lm = vec![Keypoint::new(0.0, 0.0, 0.0); 33];
        lm[0] = Keypoint::new(200.0, 50.0, 1.0);
        let mut last = prev;
        for i in 30..50 {
            let out = filter_keypoints(&mut f, &lm, i as f64 * 0.016);
            last = out[0].x;
        }
        assert!(last > 150.0, "filter should track step, got {last}");
    }

    #[test]
    fn resets_when_person_lost() {
        let mut f = LandmarkFilter::new(OneEuroParams::default());
        let mut pose = PoseFrame {
            frame_index: 0,
            timestamp_secs: 0.0,
            landmarks: vec![Keypoint::new(10.0, 10.0, 1.0); 33],
            roi: None,
            pose_presence: 1.0,
            person_detected: true,
        };
        f.apply(&mut pose);
        pose.person_detected = false;
        f.apply(&mut pose); // reset
        pose.person_detected = true;
        pose.landmarks[0] = Keypoint::new(500.0, 500.0, 1.0);
        pose.timestamp_secs = 1.0;
        f.apply(&mut pose);
        // After reset, first sample passes through nearly unchanged.
        assert!((pose.landmarks[0].x - 500.0).abs() < 1.0);
    }
}
