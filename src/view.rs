//! Camera viewpoint for gait analysis (side / front / back).

use serde::{Deserialize, Serialize};

/// Where the camera sits relative to the runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CameraView {
    /// Sagittal plane — classic treadmill side camera.
    #[default]
    Side,
    /// Frontal plane, runner facing the camera.
    Front,
    /// Frontal plane, camera behind the runner.
    Back,
}

impl CameraView {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "side" | "s" | "sagittal" => Some(Self::Side),
            "front" | "f" | "anterior" => Some(Self::Front),
            "back" | "b" | "posterior" | "rear" => Some(Self::Back),
            _ => None,
        }
    }

    pub fn is_frontal(self) -> bool {
        matches!(self, Self::Front | Self::Back)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Side => "side",
            Self::Front => "front",
            Self::Back => "back",
        }
    }
}
