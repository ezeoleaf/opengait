# open-gait

Open-source **real-time running gait analysis**: camera (side / front / back) → BlazePose ONNX → biomechanics metrics → JSON / WebSocket dashboard (**Open Gait Dashboard**).

```text
Camera (60 FPS) ──► Detector ROI ──► Landmark pose (33 pts)
                                            │
                                            ▼
                                   Biomechanics by view
                                   (side: cadence, knee, lean, overstride
                                    front/back: hip drop, valgus, crossover)
                                            │
                          ┌─────────────────┴─────────────────┐
                          ▼                                   ▼
                     stdout JSON                      ws://127.0.0.1:8080
                                                              │
                                                              ▼
                                                    web/ (React dashboard)
```

## Features

- Live capture or **scripted synthetic demo** (no camera / models required)
- BlazePose person detector + oriented ROI + landmark ONNX
- **Multi-view form**: side (sagittal) and front / back (frontal plane)
- Side metrics: cadence (SPM), knee flexion, torso lean, overstride (px / cm)
- Front/back metrics: hip drop, trunk lateral lean, knee valgus, crossover
- Throttled JPEG preview + skeleton overlay (toggle video off for privacy)
- Side-view calibration (`--facing`, `--height-cm`)
- Web UI view switcher (sends `set_view` over WebSocket)
- Session recording, slow-mo scrub, PDF/JSON debrief in the web UI

## Screenshots
<img width="1169" height="994" alt="Screenshot 2026-10-02 at 22 37 55" src="https://github.com/user-attachments/assets/199cd9dc-da7f-4b3c-b993-cd2b5b567735" />

<img width="1133" height="987" alt="Screenshot 2026-10-02 at 22 38 05" src="https://github.com/user-attachments/assets/69272518-90f9-40a4-8429-e15e3bdb7cfe" />


## Demo reel (screenshots & video)

The default non-live build runs a **70-second looping demo** with a stick-figure runner and changing gait scenarios — ideal for recording a repo walkthrough without a treadmill setup.

| Phase | Approx. window | What changes |
| --- | --- | --- |
| Steady | 0–12 s | ~175 SPM baseline |
| Overstride | 12–24 s | ankle ahead of hip |
| High cadence | 24–34 s | ~190 SPM |
| Forward lean | 34–44 s | larger torso lean |
| Frontal form | 44–58 s | hip drop / valgus / crossover (switch UI to **Front**) |
| Recovery | 58–70 s | back toward baseline |

```bash
# Terminal 1 — synthetic demo backend (JPEG preview on)
make demo
# or: cargo run --release -- --demo --preview-fps 15 --facing right

# Terminal 2 — dashboard
make web
# or: cd web && npm install && npm run dev
```

Open http://localhost:5173. Use the header **Side / Front / Back** control during the *frontal-form* phase for frontal metrics cards. Tip: leave **Video on** while filming; use **Video off** for skeleton-only shots.

## Models (not in git)

ONNX weights are **not committed** (see `*.onnx` in `.gitignore`). They are large binaries (~26 MB total) and come from an upstream conversion of MediaPipe models.

**Download them once** (only needed for live camera analysis):

```bash
./scripts/fetch-model.sh
```

| File | Role |
| --- | --- |
| `models/blazepose_landmark_full.onnx` | 33 body landmarks |
| `models/pose_detection.onnx` | Person detector → ROI |

Source: [bolducmanuel/blaze_models_onnxruntime](https://github.com/bolducmanuel/blaze_models_onnxruntime). More detail in [`models/README.md`](models/README.md).

## Requirements

- Rust (stable)
- Node.js 20+ (for the dashboard)
- Camera permission for your terminal app when using `--live` (macOS: System Settings → Privacy & Security → Camera)
- Optional: `--features camera,onnx` for live inference

## Quick start

### Synthetic demo (no camera / no models)

```bash
cargo run --release -- --demo --preview-fps 15 --facing right
```

### Live camera + ONNX

```bash
./scripts/fetch-model.sh

cargo run --release --features camera,onnx -- \
  --live --device 0 \
  --model models/blazepose_landmark_full.onnx \
  --detector models/pose_detection.onnx \
  --view side \
  --facing auto \
  --height-cm 175 \
  --preview-fps 12
```

Metrics stream on **`ws://127.0.0.1:8080`**. Switch **Side / Front / Back** in the dashboard header to change which plane is analysed (no restart needed).

Useful flags:

| Flag | Meaning |
| --- | --- |
| `--demo` | Scripted synthetic reel (also default without `--live` / `--model`) |
| `--live` | Use webcam (needs `--features camera`) |
| `--device N` | Camera index (default `0`) |
| `--model` / `--detector` | Landmark + detector ONNX paths |
| `--view side\|front\|back` | Initial camera viewpoint (override from web UI) |
| `--facing left\|right\|auto` | Side-view run direction |
| `--height-cm` | Enables overstride in centimetres |
| `--preview-fps` | JPEG preview rate over WS (`0` = off) |
| `--no-ws` | Stdout only |

### Dashboard

```bash
cd web
npm install
npm run dev
```

Open http://localhost:5173 — it connects to the local WebSocket automatically.

## Layout

```text
open-gait/
├── src/                 # Rust binary
│   ├── main.rs          # CLI, WS server, pipeline
│   ├── camera.rs        # Capture (live / synthetic)
│   ├── demo.rs          # Scripted gait reel + stick-figure frames
│   ├── pose.rs          # ONNX detector + landmarks
│   ├── biomechanics.rs  # Side + frontal metrics
│   ├── view.rs          # CameraView (side/front/back)
│   ├── calibration.rs   # Facing + px→cm
│   └── preview.rs       # JPEG encode for WS
├── site/                # Project landing page (GitHub Pages)
├── models/              # ONNX files (downloaded, not committed)
├── scripts/fetch-model.sh
├── web/                 # Open Gait React dashboard
├── ROADMAP.md           # Rust backlog
└── web/ROADMAP.md       # Frontend backlog
```

## Development

```bash
cargo test
cargo check --features camera,onnx

cd web && npm run build
```

## License

This project is licensed under the **GNU General Public License v3.0 only** — see [`LICENSE`](LICENSE) and the `license` field in `Cargo.toml`.
