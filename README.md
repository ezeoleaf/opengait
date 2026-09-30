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

- Live or synthetic capture (`nokhwa` / demo generator)
- BlazePose person detector + oriented ROI + landmark ONNX
- **Multi-view form**: side (sagittal) and front / back (frontal plane)
- Side metrics: cadence (SPM), knee flexion, torso lean, overstride (px / cm)
- Front/back metrics: hip drop, trunk lateral lean, knee valgus, crossover
- Throttled JPEG preview + skeleton overlay for the dashboard
- Side-view calibration (`--facing`, `--height-cm`)
- Web UI view switcher (sends `set_view` over WebSocket)
- Session recording, slow-mo scrub, PDF/JSON debrief in the web UI

## Models (not in git)

ONNX weights are **not committed** (see `*.onnx` in `.gitignore`). They are large binaries (~26 MB total) and come from an upstream conversion of MediaPipe models.

**Download them once:**

```bash
./scripts/fetch-model.sh
```

That writes:

| File | Role |
| --- | --- |
| `models/blazepose_landmark_full.onnx` | 33 body landmarks |
| `models/pose_detection.onnx` | Person detector → ROI |

Source: [bolducmanuel/blaze_models_onnxruntime](https://github.com/bolducmanuel/blaze_models_onnxruntime). More detail in [`models/README.md`](models/README.md).

> **Should you upload them to the repo?** Prefer **no** — keep the fetch script. Committing `.onnx` files bloats history, slows clones, and duplicates upstream. Use [Git LFS](https://git-lfs.com/) only if you need a private/offline mirror; for public open-source, download-on-setup is enough.

## Requirements

- Rust (stable)
- Node.js 20+ (for the dashboard)
- Camera permission for your terminal app (macOS: System Settings → Privacy & Security → Camera)
- Optional: `--features camera,onnx` for live inference

## Quick start

### 1. Models

```bash
./scripts/fetch-model.sh
```

### 2. Rust backend

Synthetic demo (no camera / no models):

```bash
cargo run --release
```

Live camera + ONNX:

```bash
cargo run --release --features camera,onnx -- \
  --live --device 0 \
  --model models/blazepose_landmark_full.onnx \
  --detector models/pose_detection.onnx \
  --view side \
  --facing auto \
  --height-cm 175 \
  --preview-fps 12
```

Metrics stream on **`ws://127.0.0.1:8080`** (and JSON lines on stdout). Switch **Side / Front / Back** in the dashboard header to change which plane is analysed (no restart needed).

Useful flags:

| Flag | Meaning |
| --- | --- |
| `--live` | Use webcam (needs `--features camera`) |
| `--device N` | Camera index (default `0`) |
| `--model` / `--detector` | Landmark + detector ONNX paths |
| `--view side\|front\|back` | Initial camera viewpoint (override from web UI) |
| `--facing left\|right\|auto` | Side-view run direction |
| `--height-cm` | Enables overstride in centimetres |
| `--preview-fps` | JPEG preview rate over WS (`0` = off) |
| `--no-ws` | Stdout only |

### 3. Dashboard

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
│   ├── pose.rs          # ONNX detector + landmarks
│   ├── biomechanics.rs  # Side + frontal metrics
│   ├── view.rs          # CameraView (side/front/back)
│   ├── calibration.rs   # Facing + px→cm
│   └── preview.rs       # JPEG encode for WS
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

MIT (see package metadata in `Cargo.toml`).
