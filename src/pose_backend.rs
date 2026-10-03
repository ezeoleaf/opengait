//! Alternate pose backends behind the shared [`crate::pose::PoseEstimator`] trait.
//!
//! - **blazepose** — current production path (detector ROI + 33 landmarks).
//! - **movenet** / **yolo** — COCO-17 remapping helpers are ready; ONNX graphs
//!   are not wired yet (selecting them returns a clear error).

use crate::biomechanics::{Keypoint, Landmark};
use anyhow::{bail, Result};

/// Which landmark model family to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PoseBackendKind {
    #[default]
    BlazePose,
    MoveNet,
    YoloPose,
}

impl PoseBackendKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "blazepose" | "blaze" | "mediapipe" => Some(Self::BlazePose),
            "movenet" | "move-net" | "mn" => Some(Self::MoveNet),
            "yolo" | "yolo-pose" | "yolov8" | "yolo11" => Some(Self::YoloPose),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::BlazePose => "blazepose",
            Self::MoveNet => "movenet",
            Self::YoloPose => "yolo",
        }
    }
}

/// COCO / MoveNet / YOLO-Pose 17-keypoint indices.
#[derive(Debug, Clone, Copy)]
#[repr(usize)]
#[allow(dead_code)] // used by upcoming MoveNet / YOLO ONNX loaders + tests
pub enum CocoLandmark {
    Nose = 0,
    LeftEye = 1,
    RightEye = 2,
    LeftEar = 3,
    RightEar = 4,
    LeftShoulder = 5,
    RightShoulder = 6,
    LeftElbow = 7,
    RightElbow = 8,
    LeftWrist = 9,
    RightWrist = 10,
    LeftHip = 11,
    RightHip = 12,
    LeftKnee = 13,
    RightKnee = 14,
    LeftAnkle = 15,
    RightAnkle = 16,
}

/// Map 17 COCO keypoints into a 33-slot BlazePose-compatible buffer.
///
/// Unmapped BlazePose slots (face mesh extras, hands detail, feet) stay at
/// zero confidence so gait metrics that need heels/toes fall back gracefully.
#[allow(dead_code)] // used by upcoming MoveNet / YOLO ONNX loaders + tests
pub fn coco17_to_blazepose33(coco: &[Keypoint]) -> Vec<Keypoint> {
    let mut out = vec![Keypoint::new(0.0, 0.0, 0.0); 33];
    let get = |i: CocoLandmark| coco.get(i as usize).copied().unwrap_or(Keypoint::new(0.0, 0.0, 0.0));

    out[Landmark::Nose as usize] = get(CocoLandmark::Nose);
    out[Landmark::LeftShoulder as usize] = get(CocoLandmark::LeftShoulder);
    out[Landmark::RightShoulder as usize] = get(CocoLandmark::RightShoulder);
    out[Landmark::LeftElbow as usize] = get(CocoLandmark::LeftElbow);
    out[Landmark::RightElbow as usize] = get(CocoLandmark::RightElbow);
    out[Landmark::LeftWrist as usize] = get(CocoLandmark::LeftWrist);
    out[Landmark::RightWrist as usize] = get(CocoLandmark::RightWrist);
    out[Landmark::LeftHip as usize] = get(CocoLandmark::LeftHip);
    out[Landmark::RightHip as usize] = get(CocoLandmark::RightHip);
    out[Landmark::LeftKnee as usize] = get(CocoLandmark::LeftKnee);
    out[Landmark::RightKnee as usize] = get(CocoLandmark::RightKnee);
    out[Landmark::LeftAnkle as usize] = get(CocoLandmark::LeftAnkle);
    out[Landmark::RightAnkle as usize] = get(CocoLandmark::RightAnkle);

    // Approximate heels / foot index from ankles (no dedicated COCO points).
    let la = get(CocoLandmark::LeftAnkle);
    let ra = get(CocoLandmark::RightAnkle);
    if la.confidence > 0.0 {
        out[Landmark::LeftHeel as usize] = Keypoint::new(la.x - 4.0, la.y + 4.0, la.confidence * 0.7);
        out[Landmark::LeftFootIndex as usize] =
            Keypoint::new(la.x + 8.0, la.y + 2.0, la.confidence * 0.7);
    }
    if ra.confidence > 0.0 {
        out[Landmark::RightHeel as usize] = Keypoint::new(ra.x - 4.0, ra.y + 4.0, ra.confidence * 0.7);
        out[Landmark::RightFootIndex as usize] =
            Keypoint::new(ra.x + 8.0, ra.y + 2.0, ra.confidence * 0.7);
    }

    out
}

/// Selected backend is not wired to an ONNX session yet.
pub fn unsupported_backend(kind: PoseBackendKind) -> Result<()> {
    bail!(
        "pose backend `{}` is not implemented yet — COCO→BlazePose remapping \
         is ready in `pose_backend`; use `--backend blazepose` (default) for now. \
         Tracked in ROADMAP.md.",
        kind.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_backends() {
        assert_eq!(PoseBackendKind::parse("blazepose"), Some(PoseBackendKind::BlazePose));
        assert_eq!(PoseBackendKind::parse("MoveNet"), Some(PoseBackendKind::MoveNet));
        assert_eq!(PoseBackendKind::parse("yolov8"), Some(PoseBackendKind::YoloPose));
        assert_eq!(PoseBackendKind::parse("nope"), None);
    }

    #[test]
    fn coco_remap_copies_hips() {
        let mut coco = vec![Keypoint::new(0.0, 0.0, 0.0); 17];
        coco[CocoLandmark::LeftHip as usize] = Keypoint::new(10.0, 20.0, 0.9);
        coco[CocoLandmark::RightHip as usize] = Keypoint::new(30.0, 20.0, 0.9);
        coco[CocoLandmark::LeftAnkle as usize] = Keypoint::new(12.0, 80.0, 0.8);
        let bp = coco17_to_blazepose33(&coco);
        assert_eq!(bp[Landmark::LeftHip as usize].x, 10.0);
        assert_eq!(bp[Landmark::RightHip as usize].x, 30.0);
        assert!(bp[Landmark::LeftHeel as usize].confidence > 0.0);
        assert_eq!(bp.len(), 33);
    }
}
