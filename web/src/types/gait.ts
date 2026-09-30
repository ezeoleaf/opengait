/** BlazePose-compatible landmark + metrics types matching the Rust backend. */

export type CameraView = 'side' | 'front' | 'back'

export interface Keypoint {
  x: number
  y: number
  confidence: number
}

export interface OverstrideSample {
  ankle_hip_delta_x: number
  /** Ahead of COM in running direction (px). */
  ahead_px?: number
  /** Ahead of COM in cm when height calibrated. */
  ahead_cm?: number | null
  overstride_ratio: number
}

/** Oriented detector / tracking ROI (square in image space). */
export interface DetectorRoi {
  cx: number
  cy: number
  size: number
  angle_rad: number
}

export interface GaitMetrics {
  frame_index: number
  timestamp_secs: number
  view?: CameraView
  left_knee_flexion_deg: number | null
  right_knee_flexion_deg: number | null
  torso_lean_deg: number | null
  left_overstride: OverstrideSample | null
  right_overstride: OverstrideSample | null
  hip_drop_deg?: number | null
  shoulder_drop_deg?: number | null
  trunk_lateral_lean_deg?: number | null
  left_knee_valgus_deg?: number | null
  right_knee_valgus_deg?: number | null
  left_crossover_px?: number | null
  right_crossover_px?: number | null
  cadence_spm: number | null
  left_foot_strike: boolean
  right_foot_strike: boolean
  landmarks: Keypoint[]
  roi?: DetectorRoi | null
}

export interface MetricHistoryPoint {
  t: number
  knee: number | null
  lean: number | null
  overstride: number | null
  hipDrop: number | null
  lateralLean: number | null
  valgus: number | null
  crossover: number | null
}

export interface MetricsMessage {
  type: 'gait_metrics' | 'session'
  metrics?: GaitMetrics
  frame_width?: number
  frame_height?: number
  view?: CameraView
  facing?: 'left' | 'right' | 'auto'
  cm_per_px?: number | null
  /** Base64 JPEG (no data-URL prefix). */
  frame?: string
}

export type ConnectionStatus = 'connecting' | 'open' | 'closed' | 'error'

export type MetricStatus = 'optimal' | 'caution' | 'risk' | 'idle'

export interface RecordedFrame {
  metrics: GaitMetrics
  frameDataUrl?: string
}

/** MediaPipe BlazePose landmark indices used by the overlay. */
export const Landmark = {
  Nose: 0,
  LeftShoulder: 11,
  RightShoulder: 12,
  LeftHip: 23,
  RightHip: 24,
  LeftKnee: 25,
  RightKnee: 26,
  LeftAnkle: 27,
  RightAnkle: 28,
  LeftHeel: 29,
  RightHeel: 30,
  LeftFootIndex: 31,
  RightFootIndex: 32,
} as const

/** Skeleton bone pairs for the overlay (works for side and frontal). */
export const SKELETON_EDGES: Array<[number, number]> = [
  [Landmark.LeftShoulder, Landmark.RightShoulder],
  [Landmark.LeftShoulder, Landmark.LeftHip],
  [Landmark.RightShoulder, Landmark.RightHip],
  [Landmark.LeftHip, Landmark.RightHip],
  [Landmark.LeftHip, Landmark.LeftKnee],
  [Landmark.LeftKnee, Landmark.LeftAnkle],
  [Landmark.LeftAnkle, Landmark.LeftHeel],
  [Landmark.LeftAnkle, Landmark.LeftFootIndex],
  [Landmark.RightHip, Landmark.RightKnee],
  [Landmark.RightKnee, Landmark.RightAnkle],
  [Landmark.RightAnkle, Landmark.RightHeel],
  [Landmark.RightAnkle, Landmark.RightFootIndex],
]
