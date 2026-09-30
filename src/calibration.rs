//! Side-view calibration: runner facing direction and pixel → centimetre scale.

use crate::biomechanics::{Keypoint, Landmark, PoseFrame};
use serde::{Deserialize, Serialize};

/// Which way the runner faces in the image (side-view camera).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Facing {
    /// Runner faces image −X (typical treadmill cam on the right side of the belt).
    Left,
    /// Runner faces image +X.
    Right,
    /// Infer from nose / shoulder horizontal offset each frame.
    #[default]
    Auto,
}

impl Facing {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "left" | "l" => Some(Self::Left),
            "right" | "r" => Some(Self::Right),
            "auto" | "a" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// Persistent calibration knobs for a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calibration {
    pub facing: Facing,
    /// Optional known runner stature in centimetres (used to derive cm/px).
    pub height_cm: Option<f32>,
    /// Smoothed centimetres per pixel (None until height + pose available).
    pub cm_per_px: Option<f32>,
    /// Last resolved facing (Auto → Left/Right).
    pub resolved_facing: Facing,
}

impl Calibration {
    pub fn new(facing: Facing, height_cm: Option<f32>) -> Self {
        Self {
            facing,
            height_cm,
            cm_per_px: None,
            resolved_facing: match facing {
                Facing::Auto => Facing::Right,
                other => other,
            },
        }
    }

    /// Update facing (if Auto) and cm/px from the current pose.
    pub fn update(&mut self, pose: &PoseFrame) {
        if self.facing == Facing::Auto {
            if let Some(f) = infer_facing(pose) {
                // Light hysteresis via majority — just adopt for now.
                self.resolved_facing = f;
            }
        } else {
            self.resolved_facing = self.facing;
        }

        if let Some(height_cm) = self.height_cm {
            if let Some(px_h) = estimate_person_height_px(pose) {
                if px_h > 40.0 {
                    let sample = height_cm / px_h;
                    self.cm_per_px = Some(match self.cm_per_px {
                        Some(prev) => prev * 0.9 + sample * 0.1,
                        None => sample,
                    });
                }
            }
        }
    }

    /// Convert image-space Δx (ankle ahead of hip in +X) into “ahead of COM”
    /// along the running direction, in centimetres when calibrated.
    pub fn overstride_signed_px(&self, ankle_hip_delta_x: f32) -> f32 {
        match self.resolved_facing {
            Facing::Right | Facing::Auto => ankle_hip_delta_x,
            Facing::Left => -ankle_hip_delta_x,
        }
    }

    pub fn px_to_cm(&self, px: f32) -> Option<f32> {
        self.cm_per_px.map(|s| px * s)
    }

    /// Torso lean positive when leaning into the running direction.
    pub fn signed_torso_lean(&self, raw_lean_deg: f32) -> f32 {
        match self.resolved_facing {
            Facing::Right | Facing::Auto => raw_lean_deg,
            Facing::Left => -raw_lean_deg,
        }
    }
}

fn kp(pose: &PoseFrame, id: Landmark) -> Option<&Keypoint> {
    pose.get(id).filter(|k| k.is_visible(0.4))
}

fn infer_facing(pose: &PoseFrame) -> Option<Facing> {
    let nose = kp(pose, Landmark::Nose)?;
    let mid_hip = pose.mid_hip()?;
    // Nose ahead of hips in +X ⇒ facing right.
    if nose.x >= mid_hip.x {
        Some(Facing::Right)
    } else {
        Some(Facing::Left)
    }
}

/// Approximate standing height in pixels from nose (or mid-shoulder) to lower ankle.
fn estimate_person_height_px(pose: &PoseFrame) -> Option<f32> {
    let top = kp(pose, Landmark::Nose)
        .map(|k| k.y)
        .or_else(|| pose.mid_shoulder().map(|v| v.y))?;
    let left_a = kp(pose, Landmark::LeftAnkle).map(|k| k.y);
    let right_a = kp(pose, Landmark::RightAnkle).map(|k| k.y);
    let bottom = match (left_a, right_a) {
        (Some(a), Some(b)) => a.max(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return None,
    };
    let h = (bottom - top).abs();
    if h > 1.0 {
        Some(h)
    } else {
        None
    }
}

// Need Nose in Landmark enum — add it.
