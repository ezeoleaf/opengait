//! Pure biomechanical calculations for running gait analysis.
//!
//! All functions operate on 2D keypoints in image space (origin top-left,
//! Y increasing downward). Angles are returned in degrees.
//!
//! - **Side** view: sagittal metrics (knee flexion, torso lean, overstride).
//! - **Front / back** view: frontal-plane metrics (hip drop, knee valgus, crossover).

use crate::view::CameraView;
use glam::Vec2;
use serde::{Deserialize, Serialize};

/// MediaPipe BlazePose-style landmark indices used by open-gait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Landmark {
    Nose = 0,
    LeftShoulder = 11,
    RightShoulder = 12,
    LeftElbow = 13,
    RightElbow = 14,
    LeftWrist = 15,
    RightWrist = 16,
    LeftHip = 23,
    RightHip = 24,
    LeftKnee = 25,
    RightKnee = 26,
    LeftAnkle = 27,
    RightAnkle = 28,
    LeftHeel = 29,
    RightHeel = 30,
    LeftFootIndex = 31,
    RightFootIndex = 32,
}

/// Which side of the body a metric applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

/// A single 2D keypoint with optional confidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Keypoint {
    pub x: f32,
    pub y: f32,
    pub confidence: f32,
}

impl Keypoint {
    pub fn new(x: f32, y: f32, confidence: f32) -> Self {
        Self { x, y, confidence }
    }

    pub fn as_vec2(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    pub fn is_visible(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }
}

/// Full pose frame: 33 BlazePose-compatible landmarks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseFrame {
    /// Monotonic frame index from the capture pipeline.
    pub frame_index: u64,
    /// Capture timestamp in seconds (monotonic clock).
    pub timestamp_secs: f64,
    /// Exactly 33 landmarks when present; empty if inference failed.
    pub landmarks: Vec<Keypoint>,
    /// Active detector / tracking ROI (oriented square), if available.
    pub roi: Option<DetectorRoi>,
    /// Model pose-presence score in \[0, 1\] (1.0 for synthetic).
    #[serde(default = "default_presence")]
    pub pose_presence: f32,
    /// True when torso + at least one supporting leg look reliable enough for gait.
    #[serde(default = "default_true")]
    pub person_detected: bool,
}

fn default_presence() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

/// Oriented person ROI used for landmark cropping (debug overlay).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DetectorRoi {
    pub cx: f32,
    pub cy: f32,
    /// Side length of the square ROI in pixels.
    pub size: f32,
    /// Rotation in radians (0 = upright).
    pub angle_rad: f32,
}

impl PoseFrame {
    pub fn get(&self, landmark: Landmark) -> Option<&Keypoint> {
        self.landmarks.get(landmark as usize)
    }

    pub fn hip(&self, side: Side) -> Option<&Keypoint> {
        match side {
            Side::Left => self.get(Landmark::LeftHip),
            Side::Right => self.get(Landmark::RightHip),
        }
    }

    pub fn knee(&self, side: Side) -> Option<&Keypoint> {
        match side {
            Side::Left => self.get(Landmark::LeftKnee),
            Side::Right => self.get(Landmark::RightKnee),
        }
    }

    pub fn ankle(&self, side: Side) -> Option<&Keypoint> {
        match side {
            Side::Left => self.get(Landmark::LeftAnkle),
            Side::Right => self.get(Landmark::RightAnkle),
        }
    }

    pub fn shoulder(&self, side: Side) -> Option<&Keypoint> {
        match side {
            Side::Left => self.get(Landmark::LeftShoulder),
            Side::Right => self.get(Landmark::RightShoulder),
        }
    }

    /// Midpoint between left and right hips (proxy for pelvis / body height reference).
    pub fn mid_hip(&self) -> Option<Vec2> {
        let l = self.hip(Side::Left)?;
        let r = self.hip(Side::Right)?;
        Some((l.as_vec2() + r.as_vec2()) * 0.5)
    }

    /// Midpoint between left and right shoulders.
    pub fn mid_shoulder(&self) -> Option<Vec2> {
        let l = self.shoulder(Side::Left)?;
        let r = self.shoulder(Side::Right)?;
        Some((l.as_vec2() + r.as_vec2()) * 0.5)
    }
}

/// Minimum landmark confidence for "visible enough for gait".
pub const PERSON_LANDMARK_CONF: f32 = 0.45;
/// BlazePose pose-presence gate (model scalar).
pub const PERSON_PRESENCE_THRESH: f32 = 0.55;

/// Decide whether a pose is trustworthy enough to drive biomechanics.
///
/// Requires a torso (hips + shoulders) and at least one lower-limb chain
/// (hip–knee–ankle). Arms are not required for the gate, but they expand the
/// tracking ROI when visible so swinging wrists are not cropped.
pub fn assess_person_quality(landmarks: &[Keypoint], pose_presence: f32) -> bool {
    if pose_presence < PERSON_PRESENCE_THRESH {
        return false;
    }
    if landmarks.len() < 33 {
        return false;
    }
    let conf = PERSON_LANDMARK_CONF;
    let visible = |id: Landmark| {
        landmarks
            .get(id as usize)
            .is_some_and(|k| k.is_visible(conf))
    };

    let torso = visible(Landmark::LeftHip)
        && visible(Landmark::RightHip)
        && visible(Landmark::LeftShoulder)
        && visible(Landmark::RightShoulder);

    let left_leg = visible(Landmark::LeftHip)
        && visible(Landmark::LeftKnee)
        && visible(Landmark::LeftAnkle);
    let right_leg = visible(Landmark::RightHip)
        && visible(Landmark::RightKnee)
        && visible(Landmark::RightAnkle);

    torso && (left_leg || right_leg)
}

/// Per-frame biomechanical metrics emitted to the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaitMetrics {
    pub frame_index: u64,
    pub timestamp_secs: f64,
    /// Active camera viewpoint used for this sample.
    pub view: CameraView,
    /// Whether the person passed the detection / landmark quality gate.
    pub person_detected: bool,
    /// Raw pose-presence score from the landmark model (1.0 in demo mode).
    pub pose_presence: f32,
    // --- Sagittal (side) ---
    pub left_knee_flexion_deg: Option<f32>,
    pub right_knee_flexion_deg: Option<f32>,
    pub torso_lean_deg: Option<f32>,
    pub left_overstride: Option<OverstrideSample>,
    pub right_overstride: Option<OverstrideSample>,
    // --- Frontal (front / back) ---
    /// Pelvic obliquity in degrees. Positive = right hip lower than left.
    pub hip_drop_deg: Option<f32>,
    /// Shoulder line tilt. Positive = right shoulder lower than left.
    pub shoulder_drop_deg: Option<f32>,
    /// Trunk lean in the frontal plane vs vertical. Positive = lean toward +X.
    pub trunk_lateral_lean_deg: Option<f32>,
    /// Knee medial collapse (valgus). Positive = knee toward midline.
    pub left_knee_valgus_deg: Option<f32>,
    pub right_knee_valgus_deg: Option<f32>,
    /// Ankle X relative to mid-pelvis; positive = crossed toward / past midline.
    pub left_crossover_px: Option<f32>,
    pub right_crossover_px: Option<f32>,
    // --- Shared ---
    pub cadence_spm: Option<f32>,
    pub left_foot_strike: bool,
    pub right_foot_strike: bool,
    /// Landmark snapshot for skeleton overlay (may be empty).
    pub landmarks: Vec<Keypoint>,
    /// Detector / tracking ROI for debug overlay.
    pub roi: Option<DetectorRoi>,
}

/// Overstride measured at initial contact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverstrideSample {
    /// Horizontal ankle−hip delta in pixels (image X). Positive = ankle ahead of hip in +X.
    pub ankle_hip_delta_x: f32,
    /// Signed ahead-of-COM distance in running direction (px).
    pub ahead_px: f32,
    /// Same as `ahead_px` in centimetres when calibration height is set.
    pub ahead_cm: Option<f32>,
    /// Ratio of |delta_x| to estimated runner height (hip-to-ankle vertical span).
    pub overstride_ratio: f32,
}

/// Rolling state for cadence and foot-strike detection.
#[derive(Debug, Default)]
pub struct GaitTracker {
    left_ankle_y: Vec<(f64, f32)>,
    right_ankle_y: Vec<(f64, f32)>,
    left_vel_y: f32,
    right_vel_y: f32,
    left_strike_times: Vec<f64>,
    right_strike_times: Vec<f64>,
    left_smooth_y: Option<f32>,
    right_smooth_y: Option<f32>,
    last_left_strike: f64,
    last_right_strike: f64,
    window_secs: f64,
}

impl GaitTracker {
    pub fn new(window_secs: f64) -> Self {
        Self {
            window_secs,
            last_left_strike: f64::NEG_INFINITY,
            last_right_strike: f64::NEG_INFINITY,
            ..Default::default()
        }
    }

    /// Update tracker with a new pose and return computed metrics.
    ///
    /// When `pose.person_detected` is false, strike history is not advanced and
    /// kinematic metrics are cleared (cadence holds the last rolling estimate).
    pub fn update(
        &mut self,
        pose: &PoseFrame,
        calibration: Option<&crate::calibration::Calibration>,
        view: CameraView,
    ) -> GaitMetrics {
        if !pose.person_detected {
            self.prune_strikes(pose.timestamp_secs);
            let cadence_spm = cadence_from_strikes(
                &self.left_strike_times,
                &self.right_strike_times,
                self.window_secs,
            );
            return GaitMetrics {
                frame_index: pose.frame_index,
                timestamp_secs: pose.timestamp_secs,
                view,
                person_detected: false,
                pose_presence: pose.pose_presence,
                left_knee_flexion_deg: None,
                right_knee_flexion_deg: None,
                torso_lean_deg: None,
                left_overstride: None,
                right_overstride: None,
                hip_drop_deg: None,
                shoulder_drop_deg: None,
                trunk_lateral_lean_deg: None,
                left_knee_valgus_deg: None,
                right_knee_valgus_deg: None,
                left_crossover_px: None,
                right_crossover_px: None,
                cadence_spm,
                left_foot_strike: false,
                right_foot_strike: false,
                landmarks: pose.landmarks.clone(),
                roi: pose.roi,
            };
        }

        let conf = 0.3;

        let left_knee = pose
            .hip(Side::Left)
            .zip(pose.knee(Side::Left))
            .zip(pose.ankle(Side::Left))
            .filter(|((h, k), a)| h.is_visible(conf) && k.is_visible(conf) && a.is_visible(conf))
            .map(|((h, k), a)| angle_degrees(h.as_vec2(), k.as_vec2(), a.as_vec2()));

        let right_knee = pose
            .hip(Side::Right)
            .zip(pose.knee(Side::Right))
            .zip(pose.ankle(Side::Right))
            .filter(|((h, k), a)| h.is_visible(conf) && k.is_visible(conf) && a.is_visible(conf))
            .map(|((h, k), a)| angle_degrees(h.as_vec2(), k.as_vec2(), a.as_vec2()));

        let torso_lean = if view == CameraView::Side {
            pose.mid_hip()
                .zip(pose.mid_shoulder())
                .map(|(hip, shoulder)| {
                    let raw = torso_lean_degrees(hip, shoulder);
                    match calibration {
                        Some(c) => c.signed_torso_lean(raw),
                        None => raw,
                    }
                })
        } else {
            None
        };

        let left_strike = self.detect_strike(Side::Left, pose);
        let right_strike = self.detect_strike(Side::Right, pose);

        let (left_overstride, right_overstride) = if view == CameraView::Side {
            (
                if left_strike {
                    compute_overstride(pose, Side::Left, conf, calibration)
                } else {
                    None
                },
                if right_strike {
                    compute_overstride(pose, Side::Right, conf, calibration)
                } else {
                    None
                },
            )
        } else {
            (None, None)
        };

        let frontal = if view.is_frontal() {
            compute_frontal_metrics(pose, conf)
        } else {
            FrontalMetrics::default()
        };

        if left_strike {
            self.left_strike_times.push(pose.timestamp_secs);
        }
        if right_strike {
            self.right_strike_times.push(pose.timestamp_secs);
        }
        self.prune_strikes(pose.timestamp_secs);

        let cadence_spm = cadence_from_strikes(
            &self.left_strike_times,
            &self.right_strike_times,
            self.window_secs,
        );

        GaitMetrics {
            frame_index: pose.frame_index,
            timestamp_secs: pose.timestamp_secs,
            view,
            person_detected: true,
            pose_presence: pose.pose_presence,
            left_knee_flexion_deg: left_knee,
            right_knee_flexion_deg: right_knee,
            torso_lean_deg: torso_lean,
            left_overstride,
            right_overstride,
            hip_drop_deg: frontal.hip_drop_deg,
            shoulder_drop_deg: frontal.shoulder_drop_deg,
            trunk_lateral_lean_deg: frontal.trunk_lateral_lean_deg,
            left_knee_valgus_deg: frontal.left_knee_valgus_deg,
            right_knee_valgus_deg: frontal.right_knee_valgus_deg,
            left_crossover_px: frontal.left_crossover_px,
            right_crossover_px: frontal.right_crossover_px,
            cadence_spm,
            left_foot_strike: left_strike,
            right_foot_strike: right_strike,
            landmarks: pose.landmarks.clone(),
            roi: pose.roi,
        }
    }

    fn detect_strike(&mut self, side: Side, pose: &PoseFrame) -> bool {
        let Some(ankle) = pose.ankle(side).filter(|k| k.is_visible(0.5)) else {
            return false;
        };

        let t = pose.timestamp_secs;
        // EMA smooth ankle Y to kill detector jitter before peak picking.
        let (smooth_slot, history, prev_vel, last_strike) = match side {
            Side::Left => (
                &mut self.left_smooth_y,
                &mut self.left_ankle_y,
                &mut self.left_vel_y,
                &mut self.last_left_strike,
            ),
            Side::Right => (
                &mut self.right_smooth_y,
                &mut self.right_ankle_y,
                &mut self.right_vel_y,
                &mut self.last_right_strike,
            ),
        };

        let y = match *smooth_slot {
            Some(prev) => prev * 0.7 + ankle.y * 0.3,
            None => ankle.y,
        };
        *smooth_slot = Some(y);

        // Refractory: max ~210 SPM ⇒ ≥0.285s between same-foot strikes.
        if t - *last_strike < 0.28 {
            history.push((t, y));
            let cutoff = t - self.window_secs;
            history.retain(|&(ts, _)| ts >= cutoff);
            return false;
        }

        history.push((t, y));
        let cutoff = t - self.window_secs;
        history.retain(|&(ts, _)| ts >= cutoff);

        if history.len() < 5 {
            return false;
        }

        let n = history.len();
        let (t0, y0) = history[n - 3];
        let (t1, y1) = history[n - 2];
        let (t2, y2) = history[n - 1];

        let dt01 = (t1 - t0).max(1e-6) as f32;
        let dt12 = (t2 - t1).max(1e-6) as f32;
        let v01 = (y1 - y0) / dt01;
        let v12 = (y2 - y1) / dt12;
        // Image Y increases downward: positive velocity = moving down.
        let accel = (v12 - v01) / ((dt01 + dt12) * 0.5);

        // Require a clearer reversal with minimum vertical travel in the window.
        let recent_span = history[n - 5..]
            .iter()
            .map(|&(_, yy)| yy)
            .fold((f32::MAX, f32::MIN), |(lo, hi), yy| (lo.min(yy), hi.max(yy)));
        let travel = recent_span.1 - recent_span.0;
        if travel < 8.0 {
            *prev_vel = v12;
            return false;
        }

        let was_descending = *prev_vel > 80.0; // px/s downward
        let now_ascending = v12 < -50.0;
        let accel_flip = *prev_vel > 40.0 && v12 < -20.0 && accel < -500.0;

        *prev_vel = v12;

        let hit = was_descending && (now_ascending || accel_flip);
        if hit {
            *last_strike = t;
        }
        hit
    }

    fn prune_strikes(&mut self, now: f64) {
        let cutoff = now - self.window_secs;
        self.left_strike_times.retain(|&t| t >= cutoff);
        self.right_strike_times.retain(|&t| t >= cutoff);
    }
}

/// Angle at `vertex` formed by points `a → vertex → c`, in degrees `[0, 180]`.
///
/// Used for knee flexion: Hip → Knee → Ankle.
pub fn angle_degrees(a: Vec2, vertex: Vec2, c: Vec2) -> f32 {
    let v1 = a - vertex;
    let v2 = c - vertex;
    let len1 = v1.length();
    let len2 = v2.length();
    if len1 < 1e-6 || len2 < 1e-6 {
        return 0.0;
    }
    let cos = v1.dot(v2) / (len1 * len2);
    cos.clamp(-1.0, 1.0).acos().to_degrees()
}

/// Angle between a vertical reference (straight up in image space = −Y)
/// and the Hip → Shoulder vector. Positive = leaning forward (toward +X if
/// the runner faces right; sign follows shoulder.x − hip.x).
pub fn torso_lean_degrees(hip: Vec2, shoulder: Vec2) -> f32 {
    let torso = shoulder - hip;
    if torso.length_squared() < 1e-12 {
        return 0.0;
    }
    // Vertical up in image coords.
    let vertical = Vec2::new(0.0, -1.0);
    let torso_n = torso.normalize();
    let cos = vertical.dot(torso_n).clamp(-1.0, 1.0);
    let mut deg = cos.acos().to_degrees();
    // Signed lean: positive when shoulder is ahead of hip in +X.
    if torso.x < 0.0 {
        deg = -deg;
    }
    deg
}

/// Horizontal overstride at contact: ankle X relative to hip X, normalized
/// by estimated leg length (hip-to-ankle vertical distance).
pub fn overstride_ratio(hip: Vec2, ankle: Vec2) -> OverstrideSample {
    let delta_x = ankle.x - hip.x;
    let height = (ankle.y - hip.y).abs().max(1.0);
    OverstrideSample {
        ankle_hip_delta_x: delta_x,
        ahead_px: delta_x,
        ahead_cm: None,
        overstride_ratio: delta_x.abs() / height,
    }
}

#[derive(Default)]
struct FrontalMetrics {
    hip_drop_deg: Option<f32>,
    shoulder_drop_deg: Option<f32>,
    trunk_lateral_lean_deg: Option<f32>,
    left_knee_valgus_deg: Option<f32>,
    right_knee_valgus_deg: Option<f32>,
    left_crossover_px: Option<f32>,
    right_crossover_px: Option<f32>,
}

fn compute_frontal_metrics(pose: &PoseFrame, conf: f32) -> FrontalMetrics {
    let mut out = FrontalMetrics::default();

    if let (Some(lh), Some(rh)) = (
        pose.hip(Side::Left).filter(|k| k.is_visible(conf)),
        pose.hip(Side::Right).filter(|k| k.is_visible(conf)),
    ) {
        let dx = (rh.x - lh.x).abs().max(1.0);
        // Positive = right hip lower (larger Y).
        out.hip_drop_deg = Some(((rh.y - lh.y) / dx).atan().to_degrees());
    }

    if let (Some(ls), Some(rs)) = (
        pose.shoulder(Side::Left).filter(|k| k.is_visible(conf)),
        pose.shoulder(Side::Right).filter(|k| k.is_visible(conf)),
    ) {
        let dx = (rs.x - ls.x).abs().max(1.0);
        out.shoulder_drop_deg = Some(((rs.y - ls.y) / dx).atan().to_degrees());
    }

    if let (Some(hip), Some(shoulder)) = (pose.mid_hip(), pose.mid_shoulder()) {
        out.trunk_lateral_lean_deg = Some(trunk_lateral_lean_degrees(hip, shoulder));
    }

    let mid_x = pose.mid_hip().map(|v| v.x);
    out.left_knee_valgus_deg = knee_valgus_deg(pose, Side::Left, conf, mid_x);
    out.right_knee_valgus_deg = knee_valgus_deg(pose, Side::Right, conf, mid_x);

    if let Some(mx) = mid_x {
        out.left_crossover_px = pose
            .ankle(Side::Left)
            .filter(|k| k.is_visible(conf))
            .map(|a| crossover_px(a.x, mx, Side::Left));
        out.right_crossover_px = pose
            .ankle(Side::Right)
            .filter(|k| k.is_visible(conf))
            .map(|a| crossover_px(a.x, mx, Side::Right));
    }

    out
}

/// Frontal trunk lean: angle between vertical (−Y) and hip→shoulder, signed by X.
pub fn trunk_lateral_lean_degrees(hip: Vec2, shoulder: Vec2) -> f32 {
    let torso = shoulder - hip;
    if torso.length_squared() < 1e-12 {
        return 0.0;
    }
    let vertical = Vec2::new(0.0, -1.0);
    let torso_n = torso.normalize();
    let cos = vertical.dot(torso_n).clamp(-1.0, 1.0);
    let mut deg = cos.acos().to_degrees();
    if torso.x < 0.0 {
        deg = -deg;
    }
    deg
}

/// Positive = knee collapsed toward the anatomical midline (valgus).
pub fn knee_valgus_deg(
    pose: &PoseFrame,
    side: Side,
    conf: f32,
    mid_x: Option<f32>,
) -> Option<f32> {
    let hip = pose.hip(side).filter(|k| k.is_visible(conf))?;
    let knee = pose.knee(side).filter(|k| k.is_visible(conf))?;
    let ankle = pose.ankle(side).filter(|k| k.is_visible(conf))?;
    let mid = mid_x?;

    let h = hip.as_vec2();
    let k = knee.as_vec2();
    let a = ankle.as_vec2();
    let line = a - h;
    let len2 = line.length_squared().max(1e-6);
    let t = ((k - h).dot(line) / len2).clamp(0.0, 1.0);
    let proj = h + line * t;
    let offset_x = k.x - proj.x;
    // Left limb: midline is to the right (+X) → medial offset is +offset_x.
    // Right limb: midline is to the left (−X) → medial offset is −offset_x.
    let medial = match side {
        Side::Left => offset_x,
        Side::Right => -offset_x,
    };
    let _ = mid; // reserved for future midline-aware scaling
    Some((medial / line.length().max(1.0)).atan().to_degrees())
}

/// Positive when the ankle has crossed toward / past the pelvic midline.
pub fn crossover_px(ankle_x: f32, mid_x: f32, side: Side) -> f32 {
    match side {
        Side::Left => ankle_x - mid_x,  // left ankle moving right past mid → positive
        Side::Right => mid_x - ankle_x, // right ankle moving left past mid → positive
    }
}

fn compute_overstride(
    pose: &PoseFrame,
    side: Side,
    conf: f32,
    calibration: Option<&crate::calibration::Calibration>,
) -> Option<OverstrideSample> {
    let hip = pose.hip(side).filter(|k| k.is_visible(conf))?;
    let ankle = pose.ankle(side).filter(|k| k.is_visible(conf))?;
    let mut sample = overstride_ratio(hip.as_vec2(), ankle.as_vec2());
    if let Some(cal) = calibration {
        sample.ahead_px = cal.overstride_signed_px(sample.ankle_hip_delta_x);
        sample.ahead_cm = cal.px_to_cm(sample.ahead_px);
    }
    Some(sample)
}

/// Cadence in steps per minute from strike timestamps in a rolling window.
///
/// Counts left + right foot strikes; requires at least 2 total strikes.
pub fn cadence_from_strikes(left: &[f64], right: &[f64], window_secs: f64) -> Option<f32> {
    let total = left.len() + right.len();
    if total < 2 || window_secs <= 0.0 {
        return None;
    }
    // Use span of observed strikes when shorter than the nominal window
    // so early estimates are not artificially low.
    let mut times: Vec<f64> = left.iter().chain(right.iter()).copied().collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let span = (times.last().unwrap() - times.first().unwrap()).max(0.5);
    let effective = span.min(window_secs).max(0.5);
    let steps_per_sec = (total as f64 - 1.0) / effective;
    Some((steps_per_sec * 60.0) as f32)
}

/// Peak detection on a Y-series: local minima where the ankle is lowest
/// (largest Y in image space) — used as an alternate foot-contact cue.
#[allow(dead_code)]
pub fn detect_vertical_minima(samples: &[(f64, f32)], min_prominence: f32) -> Vec<f64> {
    let mut peaks = Vec::new();
    if samples.len() < 3 {
        return peaks;
    }
    for i in 1..samples.len() - 1 {
        let (_, y_prev) = samples[i - 1];
        let (t, y) = samples[i];
        let (_, y_next) = samples[i + 1];
        // Local maximum in Y = lowest point in the frame.
        if y >= y_prev && y >= y_next {
            let prominence = y - y_prev.min(y_next);
            if prominence >= min_prominence {
                peaks.push(t);
            }
        }
    }
    peaks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::CameraView;
    use approx::assert_relative_eq;

    #[test]
    fn right_angle_at_vertex() {
        let a = Vec2::new(0.0, 0.0);
        let v = Vec2::new(0.0, 1.0);
        let c = Vec2::new(1.0, 1.0);
        assert_relative_eq!(angle_degrees(a, v, c), 90.0, epsilon = 1e-3);
    }

    #[test]
    fn straight_leg_is_180() {
        let hip = Vec2::new(0.0, 0.0);
        let knee = Vec2::new(0.0, 1.0);
        let ankle = Vec2::new(0.0, 2.0);
        assert_relative_eq!(angle_degrees(hip, knee, ankle), 180.0, epsilon = 1e-3);
    }

    #[test]
    fn acute_knee_flexion() {
        let hip = Vec2::new(0.0, 0.0);
        let knee = Vec2::new(0.0, 1.0);
        let ankle = Vec2::new(0.5, 1.0);
        let deg = angle_degrees(hip, knee, ankle);
        assert_relative_eq!(deg, 90.0, epsilon = 1e-3);
    }

    #[test]
    fn degenerate_points_return_zero() {
        let p = Vec2::new(1.0, 1.0);
        assert_eq!(angle_degrees(p, p, Vec2::new(2.0, 2.0)), 0.0);
    }

    #[test]
    fn torso_lean_upright_is_near_zero() {
        let hip = Vec2::new(100.0, 200.0);
        let shoulder = Vec2::new(100.0, 100.0); // directly above in image space
        assert_relative_eq!(torso_lean_degrees(hip, shoulder), 0.0, epsilon = 1e-3);
    }

    #[test]
    fn torso_lean_forward_positive() {
        let hip = Vec2::new(100.0, 200.0);
        let shoulder = Vec2::new(120.0, 100.0);
        let lean = torso_lean_degrees(hip, shoulder);
        assert!(lean > 5.0, "expected forward lean, got {lean}");
    }

    #[test]
    fn overstride_ratio_scales_with_height() {
        let hip = Vec2::new(100.0, 100.0);
        let ankle = Vec2::new(130.0, 200.0); // delta_x=30, height=100
        let sample = overstride_ratio(hip, ankle);
        assert_relative_eq!(sample.ankle_hip_delta_x, 30.0, epsilon = 1e-3);
        assert_relative_eq!(sample.overstride_ratio, 0.3, epsilon = 1e-3);
    }

    #[test]
    fn cadence_from_even_strikes() {
        // 10 steps over 5 seconds → 108 SPM using (n-1)/span * 60
        let left: Vec<f64> = (0..5).map(|i| i as f64).collect();
        let right: Vec<f64> = (0..5).map(|i| i as f64 + 0.5).collect();
        let spm = cadence_from_strikes(&left, &right, 5.0).unwrap();
        // span = 4.5, total strikes = 10, (10-1)/4.5 * 60 = 120
        assert_relative_eq!(spm, 120.0, epsilon = 1.0);
    }

    #[test]
    fn cadence_needs_two_strikes() {
        assert!(cadence_from_strikes(&[1.0], &[], 5.0).is_none());
    }

    #[test]
    fn vertical_minima_detects_bottoms() {
        // Y increases downward; bottoms are local maxima in Y.
        let samples = vec![
            (0.0, 10.0),
            (0.1, 20.0),
            (0.2, 30.0), // bottom
            (0.3, 20.0),
            (0.4, 10.0),
            (0.5, 25.0),
            (0.6, 35.0), // bottom
            (0.7, 20.0),
        ];
        let peaks = detect_vertical_minima(&samples, 5.0);
        assert_eq!(peaks, vec![0.2, 0.6]);
    }

    #[test]
    fn foot_strike_on_velocity_reversal() {
        let mut tracker = GaitTracker::new(5.0);
        // Simulate ankle descending then ascending (image Y).
        let mut landmarks = vec![Keypoint::new(0.0, 0.0, 0.0); 33];
        landmarks[Landmark::LeftAnkle as usize] = Keypoint::new(50.0, 100.0, 1.0);
        landmarks[Landmark::LeftHip as usize] = Keypoint::new(50.0, 50.0, 1.0);
        landmarks[Landmark::LeftKnee as usize] = Keypoint::new(50.0, 75.0, 1.0);
        landmarks[Landmark::RightAnkle as usize] = Keypoint::new(60.0, 100.0, 1.0);
        landmarks[Landmark::RightHip as usize] = Keypoint::new(60.0, 50.0, 1.0);
        landmarks[Landmark::RightKnee as usize] = Keypoint::new(60.0, 75.0, 1.0);
        landmarks[Landmark::LeftShoulder as usize] = Keypoint::new(50.0, 20.0, 1.0);
        landmarks[Landmark::RightShoulder as usize] = Keypoint::new(60.0, 20.0, 1.0);

        let ys = [100.0, 120.0, 140.0, 160.0, 150.0, 130.0];
        let mut saw_strike = false;
        for (i, &y) in ys.iter().enumerate() {
            landmarks[Landmark::LeftAnkle as usize].y = y;
            let pose = PoseFrame {
                frame_index: i as u64,
                timestamp_secs: i as f64 * 0.016,
                landmarks: landmarks.clone(),
                roi: None,
                pose_presence: 1.0,
                person_detected: true,
            };
            let m = tracker.update(&pose, None, CameraView::Side);
            if m.left_foot_strike {
                saw_strike = true;
            }
        }
        assert!(saw_strike, "expected foot strike when ankle reverse at bottom");
    }

    #[test]
    fn crossover_positive_when_past_midline() {
        assert!(crossover_px(110.0, 100.0, Side::Left) > 0.0);
        assert!(crossover_px(90.0, 100.0, Side::Right) > 0.0);
    }

    #[test]
    fn trunk_lateral_lean_signs_with_x() {
        let hip = Vec2::new(100.0, 200.0);
        let shoulder = Vec2::new(120.0, 100.0);
        assert!(trunk_lateral_lean_degrees(hip, shoulder) > 0.0);
    }

    #[test]
    fn frontal_view_fills_hip_drop() {
        let mut landmarks = vec![Keypoint::new(0.0, 0.0, 0.0); 33];
        landmarks[Landmark::LeftHip as usize] = Keypoint::new(80.0, 100.0, 1.0);
        landmarks[Landmark::RightHip as usize] = Keypoint::new(120.0, 110.0, 1.0);
        landmarks[Landmark::LeftShoulder as usize] = Keypoint::new(85.0, 40.0, 1.0);
        landmarks[Landmark::RightShoulder as usize] = Keypoint::new(115.0, 40.0, 1.0);
        landmarks[Landmark::LeftKnee as usize] = Keypoint::new(80.0, 150.0, 1.0);
        landmarks[Landmark::RightKnee as usize] = Keypoint::new(120.0, 150.0, 1.0);
        landmarks[Landmark::LeftAnkle as usize] = Keypoint::new(80.0, 200.0, 1.0);
        landmarks[Landmark::RightAnkle as usize] = Keypoint::new(120.0, 200.0, 1.0);
        let pose = PoseFrame {
            frame_index: 0,
            timestamp_secs: 0.0,
            landmarks,
            roi: None,
            pose_presence: 1.0,
            person_detected: true,
        };
        let mut tracker = GaitTracker::new(5.0);
        let m = tracker.update(&pose, None, CameraView::Front);
        assert!(m.hip_drop_deg.is_some());
        assert!(m.hip_drop_deg.unwrap() > 0.0); // right hip lower
        assert!(m.torso_lean_deg.is_none()); // sagittal lean unused in front view
        assert_eq!(m.view, CameraView::Front);
        assert!(m.person_detected);
    }

    #[test]
    fn person_quality_requires_torso_and_leg() {
        let mut landmarks = vec![Keypoint::new(0.0, 0.0, 0.0); 33];
        assert!(!assess_person_quality(&landmarks, 1.0));
        landmarks[Landmark::LeftHip as usize] = Keypoint::new(80.0, 100.0, 1.0);
        landmarks[Landmark::RightHip as usize] = Keypoint::new(120.0, 100.0, 1.0);
        landmarks[Landmark::LeftShoulder as usize] = Keypoint::new(85.0, 40.0, 1.0);
        landmarks[Landmark::RightShoulder as usize] = Keypoint::new(115.0, 40.0, 1.0);
        landmarks[Landmark::LeftKnee as usize] = Keypoint::new(80.0, 150.0, 1.0);
        landmarks[Landmark::LeftAnkle as usize] = Keypoint::new(80.0, 200.0, 1.0);
        assert!(assess_person_quality(&landmarks, 0.9));
        assert!(!assess_person_quality(&landmarks, 0.2));
    }

    #[test]
    fn skipped_when_person_not_detected() {
        let landmarks = vec![Keypoint::new(0.0, 0.0, 0.0); 33];
        let pose = PoseFrame {
            frame_index: 0,
            timestamp_secs: 1.0,
            landmarks,
            roi: None,
            pose_presence: 0.1,
            person_detected: false,
        };
        let mut tracker = GaitTracker::new(5.0);
        let m = tracker.update(&pose, None, CameraView::Side);
        assert!(!m.person_detected);
        assert!(m.torso_lean_deg.is_none());
        assert!(!m.left_foot_strike);
    }
}
