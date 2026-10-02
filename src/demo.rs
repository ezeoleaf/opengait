//! Scripted synthetic runner for demos, screenshots, and offline CI.
//!
//! Cycles through gait scenarios so the dashboard shows changing metrics
//! without a camera or ONNX models. Landmark math is shared by the synthetic
//! camera (JPEG stick figure) and the synthetic pose estimator.

use crate::biomechanics::{Keypoint, Landmark};
use crate::view::CameraView;

/// Named scenarios in the looping demo reel (~70 s cycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoPhase {
    /// Clean ~175 SPM run — good baseline screenshot.
    Steady,
    /// Exaggerated overstride (ankle ahead of hip).
    Overstride,
    /// High cadence ~190 SPM.
    HighCadence,
    /// Pronounced forward torso lean.
    ForwardLean,
    /// Frontal-plane asymmetry (hip drop / valgus / crossover).
    FrontalForm,
    /// Return toward baseline.
    Recovery,
}

impl DemoPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Steady => "steady",
            Self::Overstride => "overstride",
            Self::HighCadence => "high-cadence",
            Self::ForwardLean => "forward-lean",
            Self::FrontalForm => "frontal-form",
            Self::Recovery => "recovery",
        }
    }
}

const CYCLE_SECS: f64 = 70.0;

/// Resolve the active demo phase from elapsed time (loops forever).
pub fn phase_at(t: f64) -> DemoPhase {
    let u = t.rem_euclid(CYCLE_SECS);
    if u < 12.0 {
        DemoPhase::Steady
    } else if u < 24.0 {
        DemoPhase::Overstride
    } else if u < 34.0 {
        DemoPhase::HighCadence
    } else if u < 44.0 {
        DemoPhase::ForwardLean
    } else if u < 58.0 {
        DemoPhase::FrontalForm
    } else {
        DemoPhase::Recovery
    }
}

#[derive(Debug, Clone, Copy)]
struct GaitParams {
    /// Strides per second for one foot (~ cadence_spm / 120).
    stride_hz: f64,
    torso_lean_deg: f32,
    overstride_px: f32,
    hip_drop_px: f32,
    knee_valgus_px: f32,
    crossover_px: f32,
    bounce: f32,
}

fn params_for(phase: DemoPhase) -> GaitParams {
    match phase {
        DemoPhase::Steady => GaitParams {
            stride_hz: 175.0 / 120.0,
            torso_lean_deg: 7.0,
            overstride_px: 8.0,
            hip_drop_px: 2.0,
            knee_valgus_px: 1.0,
            crossover_px: 0.0,
            bounce: 0.018,
        },
        DemoPhase::Overstride => GaitParams {
            stride_hz: 168.0 / 120.0,
            torso_lean_deg: 5.0,
            overstride_px: 42.0,
            hip_drop_px: 3.0,
            knee_valgus_px: 2.0,
            crossover_px: 2.0,
            bounce: 0.02,
        },
        DemoPhase::HighCadence => GaitParams {
            stride_hz: 192.0 / 120.0,
            torso_lean_deg: 8.0,
            overstride_px: 4.0,
            hip_drop_px: 2.0,
            knee_valgus_px: 1.0,
            crossover_px: 0.0,
            bounce: 0.014,
        },
        DemoPhase::ForwardLean => GaitParams {
            stride_hz: 178.0 / 120.0,
            torso_lean_deg: 16.0,
            overstride_px: 10.0,
            hip_drop_px: 3.0,
            knee_valgus_px: 2.0,
            crossover_px: 1.0,
            bounce: 0.02,
        },
        DemoPhase::FrontalForm => GaitParams {
            stride_hz: 172.0 / 120.0,
            torso_lean_deg: 6.0,
            overstride_px: 12.0,
            hip_drop_px: 18.0,
            knee_valgus_px: 14.0,
            crossover_px: 22.0,
            bounce: 0.022,
        },
        DemoPhase::Recovery => GaitParams {
            stride_hz: 174.0 / 120.0,
            torso_lean_deg: 6.5,
            overstride_px: 9.0,
            hip_drop_px: 4.0,
            knee_valgus_px: 3.0,
            crossover_px: 2.0,
            bounce: 0.017,
        },
    }
}

/// BlazePose-compatible landmarks for the demo runner at time `t`.
pub fn landmarks(t: f64, view: CameraView, width: f32, height: f32) -> Vec<Keypoint> {
    let phase = phase_at(t);
    let p = params_for(phase);
    match view {
        CameraView::Side => landmarks_side(t, width, height, p),
        CameraView::Front | CameraView::Back => landmarks_frontal(t, view, width, height, p),
    }
}

fn landmarks_side(t: f64, w: f32, h: f32, p: GaitParams) -> Vec<Keypoint> {
    let mut lm = blank();
    let phase = t * std::f64::consts::TAU * p.stride_hz;
    let bounce = (p.bounce * (phase * 2.0).sin() as f32) * h;

    // Facing +X (right). Slight bob of the whole figure.
    let cx = w * 0.48;
    let hip_y = h * 0.42 + bounce;
    let lean_rad = p.torso_lean_deg.to_radians();
    // Shoulder sits ahead (+X) and above hip for forward lean.
    let torso_len = h * 0.20;
    let shoulder_x = cx + lean_rad.sin() * torso_len;
    let shoulder_y = hip_y - lean_rad.cos() * torso_len;
    let nose_x = shoulder_x + 10.0;
    let nose_y = shoulder_y - h * 0.06;

    set(&mut lm, Landmark::Nose, nose_x, nose_y, 0.95);
    set(
        &mut lm,
        Landmark::LeftShoulder,
        shoulder_x - 6.0,
        shoulder_y,
        0.95,
    );
    set(
        &mut lm,
        Landmark::RightShoulder,
        shoulder_x + 6.0,
        shoulder_y + 2.0,
        0.95,
    );
    set(&mut lm, Landmark::LeftHip, cx - 8.0, hip_y, 0.95);
    set(&mut lm, Landmark::RightHip, cx + 8.0, hip_y + 2.0, 0.95);

    // Left / right legs 180° out of phase.
    place_leg_side(
        &mut lm,
        true,
        cx - 8.0,
        hip_y,
        phase,
        p.overstride_px,
        h,
        w,
    );
    place_leg_side(
        &mut lm,
        false,
        cx + 8.0,
        hip_y + 2.0,
        phase + std::f64::consts::PI,
        p.overstride_px,
        h,
        w,
    );

    lm
}

#[allow(clippy::too_many_arguments)]
fn place_leg_side(
    lm: &mut [Keypoint],
    left: bool,
    hip_x: f32,
    hip_y: f32,
    phase: f64,
    overstride_px: f32,
    h: f32,
    _w: f32,
) {
    let swing = phase.sin() as f32;
    let thrust = phase.cos() as f32; // +1 near mid-swing / contact timing cue

    let knee_x = hip_x + thrust * (overstride_px * 0.35) - 4.0;
    let knee_y = hip_y + h * 0.16 + swing.abs() * h * 0.02;
    // Ankle: vertical bounce + horizontal overstride when foot is low.
    let ankle_y = hip_y + h * (0.30 + 0.08 * (0.5 + 0.5 * swing));
    let contact = ((swing + 1.0) * 0.5).clamp(0.0, 1.0); // high when foot low-ish
    let ankle_x = hip_x + overstride_px * contact * thrust.max(0.0).max(0.15) + thrust * 6.0;

    let (knee, ankle, heel, toe) = if left {
        (
            Landmark::LeftKnee,
            Landmark::LeftAnkle,
            Landmark::LeftHeel,
            Landmark::LeftFootIndex,
        )
    } else {
        (
            Landmark::RightKnee,
            Landmark::RightAnkle,
            Landmark::RightHeel,
            Landmark::RightFootIndex,
        )
    };

    set(lm, knee, knee_x, knee_y, 0.92);
    set(lm, ankle, ankle_x, ankle_y, 0.92);
    set(lm, heel, ankle_x - 8.0, ankle_y + 4.0, 0.88);
    set(lm, toe, ankle_x + 14.0, ankle_y + 2.0, 0.88);
}

fn landmarks_frontal(t: f64, view: CameraView, w: f32, h: f32, p: GaitParams) -> Vec<Keypoint> {
    let mut lm = blank();
    let phase = t * std::f64::consts::TAU * p.stride_hz;
    let bounce = (p.bounce * (phase * 2.0).sin() as f32) * h;
    let cx = w * 0.50;
    let hip_y = h * 0.42 + bounce;
    let shoulder_y = hip_y - h * 0.20;
    let nose_y = shoulder_y - h * 0.06;

    // Back view mirrors left/right visually but we keep anatomical L/R.
    let mirror = matches!(view, CameraView::Back);
    let sign = if mirror { -1.0_f32 } else { 1.0 };

    let hip_half = w * 0.055;
    let shoulder_half = w * 0.07;
    // Right hip drops (larger Y) during FrontalForm.
    let drop = p.hip_drop_px;

    set(&mut lm, Landmark::Nose, cx, nose_y, 0.95);
    set(
        &mut lm,
        Landmark::LeftShoulder,
        cx - sign * shoulder_half,
        shoulder_y + drop * 0.15,
        0.95,
    );
    set(
        &mut lm,
        Landmark::RightShoulder,
        cx + sign * shoulder_half,
        shoulder_y + drop * 0.45,
        0.95,
    );
    set(
        &mut lm,
        Landmark::LeftHip,
        cx - sign * hip_half,
        hip_y,
        0.95,
    );
    set(
        &mut lm,
        Landmark::RightHip,
        cx + sign * hip_half,
        hip_y + drop,
        0.95,
    );

    place_leg_frontal(
        &mut lm,
        true,
        cx - sign * hip_half,
        hip_y,
        phase,
        -sign,
        p,
        h,
    );
    place_leg_frontal(
        &mut lm,
        false,
        cx + sign * hip_half,
        hip_y + drop,
        phase + std::f64::consts::PI,
        sign,
        p,
        h,
    );

    lm
}

#[allow(clippy::too_many_arguments)]
fn place_leg_frontal(
    lm: &mut [Keypoint],
    left: bool,
    hip_x: f32,
    hip_y: f32,
    phase: f64,
    toward_mid_sign: f32,
    p: GaitParams,
    h: f32,
) {
    let swing = phase.sin() as f32;
    let lift = ((swing + 1.0) * 0.5) * h * 0.06;
    // Valgus: knee drifts toward midline.
    let knee_x = hip_x + toward_mid_sign * p.knee_valgus_px;
    let knee_y = hip_y + h * 0.16 - lift * 0.3;
    // Crossover: ankle past midline when positive.
    let ankle_x = hip_x + toward_mid_sign * (p.knee_valgus_px * 0.4 + p.crossover_px * 0.55);
    let ankle_y = hip_y + h * 0.32 - lift;

    let (knee, ankle, heel, toe) = if left {
        (
            Landmark::LeftKnee,
            Landmark::LeftAnkle,
            Landmark::LeftHeel,
            Landmark::LeftFootIndex,
        )
    } else {
        (
            Landmark::RightKnee,
            Landmark::RightAnkle,
            Landmark::RightHeel,
            Landmark::RightFootIndex,
        )
    };

    set(lm, knee, knee_x, knee_y, 0.92);
    set(lm, ankle, ankle_x, ankle_y, 0.92);
    set(lm, heel, ankle_x - 3.0, ankle_y + 5.0, 0.88);
    set(lm, toe, ankle_x + 3.0, ankle_y + 5.0, 0.88);
}

fn blank() -> Vec<Keypoint> {
    vec![Keypoint::new(0.0, 0.0, 0.0); 33]
}

fn set(landmarks: &mut [Keypoint], id: Landmark, x: f32, y: f32, conf: f32) {
    landmarks[id as usize] = Keypoint::new(x, y, conf);
}

/// RGB8 frame with treadmill lane + stick figure matching `landmarks()`.
pub fn render_frame(t: f64, view: CameraView, width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut buf = vec![0u8; w * h * 3];

    // Deep slate background with subtle vertical gradient.
    for y in 0..h {
        let g = 18u8.saturating_add((y as u32 * 10 / height.max(1)) as u8);
        for x in 0..w {
            let i = (y * w + x) * 3;
            buf[i] = g;
            buf[i + 1] = g.saturating_add(4);
            buf[i + 2] = g.saturating_add(10);
        }
    }

    // Ground / treadmill belt.
    let ground_y = (h as f32 * 0.82) as usize;
    fill_rect(&mut buf, w, h, 0, ground_y, w, h - ground_y, [30, 36, 48]);
    // Belt stripes scrolling with time.
    let scroll = ((t * 180.0) as isize).rem_euclid(80) as usize;
    let mut x = scroll;
    while x < w {
        fill_rect(&mut buf, w, h, x, ground_y + 4, 28, 6, [45, 55, 72]);
        x += 80;
    }

    // Phase color chip (top-left) — useful when recording without reading logs.
    let phase = phase_at(t);
    let chip = match phase {
        DemoPhase::Steady => [34, 211, 238],
        DemoPhase::Overstride => [248, 113, 113],
        DemoPhase::HighCadence => [74, 222, 128],
        DemoPhase::ForwardLean => [251, 191, 36],
        DemoPhase::FrontalForm => [167, 139, 250],
        DemoPhase::Recovery => [148, 163, 184],
    };
    fill_rect(&mut buf, w, h, 24, 24, 120, 14, chip);
    // Second chip encodes view.
    let view_chip = match view {
        CameraView::Side => [34, 211, 238],
        CameraView::Front => [74, 222, 128],
        CameraView::Back => [251, 191, 36],
    };
    fill_rect(&mut buf, w, h, 24, 44, 64, 10, view_chip);

    let lm = landmarks(t, view, width as f32, height as f32);
    draw_skeleton(&mut buf, w, h, &lm);

    buf
}

const BONES: &[(Landmark, Landmark)] = &[
    (Landmark::LeftShoulder, Landmark::RightShoulder),
    (Landmark::LeftShoulder, Landmark::LeftHip),
    (Landmark::RightShoulder, Landmark::RightHip),
    (Landmark::LeftHip, Landmark::RightHip),
    (Landmark::LeftShoulder, Landmark::Nose),
    (Landmark::RightShoulder, Landmark::Nose),
    (Landmark::LeftHip, Landmark::LeftKnee),
    (Landmark::LeftKnee, Landmark::LeftAnkle),
    (Landmark::LeftAnkle, Landmark::LeftHeel),
    (Landmark::LeftAnkle, Landmark::LeftFootIndex),
    (Landmark::RightHip, Landmark::RightKnee),
    (Landmark::RightKnee, Landmark::RightAnkle),
    (Landmark::RightAnkle, Landmark::RightHeel),
    (Landmark::RightAnkle, Landmark::RightFootIndex),
];

fn draw_skeleton(buf: &mut [u8], w: usize, h: usize, lm: &[Keypoint]) {
    let bone = [34, 211, 238];
    let joint = [74, 222, 128];
    for &(a, b) in BONES {
        let ka = lm[a as usize];
        let kb = lm[b as usize];
        if ka.confidence < 0.3 || kb.confidence < 0.3 {
            continue;
        }
        draw_line(
            buf,
            w,
            h,
            ka.x as i32,
            ka.y as i32,
            kb.x as i32,
            kb.y as i32,
            bone,
            3,
        );
    }
    for id in [
        Landmark::Nose,
        Landmark::LeftShoulder,
        Landmark::RightShoulder,
        Landmark::LeftHip,
        Landmark::RightHip,
        Landmark::LeftKnee,
        Landmark::RightKnee,
        Landmark::LeftAnkle,
        Landmark::RightAnkle,
    ] {
        let k = lm[id as usize];
        if k.confidence < 0.3 {
            continue;
        }
        paint_dot(buf, w, h, k.x as usize, k.y as usize, 5, joint);
    }
}

fn fill_rect(
    buf: &mut [u8],
    w: usize,
    h: usize,
    x: usize,
    y: usize,
    rw: usize,
    rh: usize,
    rgb: [u8; 3],
) {
    for yy in y..(y + rh).min(h) {
        for xx in x..(x + rw).min(w) {
            let i = (yy * w + xx) * 3;
            buf[i] = rgb[0];
            buf[i + 1] = rgb[1];
            buf[i + 2] = rgb[2];
        }
    }
}

fn paint_dot(buf: &mut [u8], w: usize, h: usize, cx: usize, cy: usize, radius: usize, rgb: [u8; 3]) {
    let r2 = (radius * radius) as isize;
    for dy in -(radius as isize)..=(radius as isize) {
        for dx in -(radius as isize)..=(radius as isize) {
            if dx * dx + dy * dy > r2 {
                continue;
            }
            let x = cx as isize + dx;
            let y = cy as isize + dy;
            if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
                continue;
            }
            let i = ((y as usize) * w + x as usize) * 3;
            buf[i] = rgb[0];
            buf[i + 1] = rgb[1];
            buf[i + 2] = rgb[2];
        }
    }
}

fn draw_line(
    buf: &mut [u8],
    w: usize,
    h: usize,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    rgb: [u8; 3],
    thickness: i32,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        for t in -thickness..=thickness {
            let x = x0;
            let y = y0 + t;
            if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                let i = (y as usize * w + x as usize) * 3;
                buf[i] = rgb[0];
                buf[i + 1] = rgb[1];
                buf[i + 2] = rgb[2];
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_cycles() {
        assert_eq!(phase_at(0.0), DemoPhase::Steady);
        assert_eq!(phase_at(15.0), DemoPhase::Overstride);
        assert_eq!(phase_at(70.0), DemoPhase::Steady);
    }

    #[test]
    fn landmarks_have_visible_hips() {
        let lm = landmarks(1.0, CameraView::Side, 1280.0, 720.0);
        assert!(lm[Landmark::LeftHip as usize].confidence > 0.5);
        assert!(lm[Landmark::RightAnkle as usize].confidence > 0.5);
        let front = landmarks(40.0, CameraView::Front, 1280.0, 720.0);
        assert!(front[Landmark::RightHip as usize].y > front[Landmark::LeftHip as usize].y);
    }

    #[test]
    fn render_frame_size() {
        let buf = render_frame(0.5, CameraView::Side, 320, 180);
        assert_eq!(buf.len(), 320 * 180 * 3);
    }
}
