# Roadmap — open-gait (Rust)

Real-time running gait analysis: camera → BlazePose ONNX → biomechanics → JSON / WebSocket.

## Done

- [x] Cargo binary with modular layout (`camera`, `pose`, `biomechanics`, `main`)
- [x] Synthetic camera + synthetic pose for offline demos / CI
- [x] Live capture via `nokhwa` (`--features camera -- --live`)
- [x] BlazePose landmark ONNX inference (`--features onnx --model …`)
- [x] Person detector ROI + EMA tracking (`pose_detection.onnx`)
- [x] Oriented / rotated ROI (MediaPipe affine warp)
- [x] Throttled JPEG preview frames over WebSocket
- [x] Side-view calibration (`--facing`, `--height-cm` → overstride cm)
- [x] Landmark coord auto-detect (normalized vs 256-pixel space)
- [x] MediaPipe-style `[-1, 1]` preprocessing for detector / landmarks
- [x] Biomechanics: knee flexion, torso lean, overstride, cadence (SPM)
- [x] Front / back (frontal-plane) metrics: hip drop, lateral lean, knee valgus, crossover
- [x] Runtime view switching via CLI `--view` and WS `set_view` from the dashboard
- [x] Scripted synthetic demo reel (`--demo` / default offline) for screenshots & video
- [x] Static project site (`site/`) + GitHub Pages workflow
- [x] Foot-strike detection with EMA ankle smoothing + refractory window
- [x] JSON line stream on stdout + `ws://127.0.0.1:8080` metrics broadcast
- [x] Frame width/height in WS envelope for dashboard scaling
- [x] `scripts/fetch-model.sh` for landmark + detector ONNX
- [x] Unit tests for core biomechanics helpers
- [x] README with architecture overview + quickstart

## Next

- [ ] Landmark temporal filter (1€ / Kalman) before biomechanics
- [ ] Device picker CLI (`--list-cameras`) and clearer macOS permission errors

## Later

- [ ] MoveNet / YOLO-Pose backends behind the same `PoseEstimator` trait
- [ ] Multi-person select (treadmill lane / closest ROI)
- [ ] Offline video file input (`--video run.mp4`) for batch analysis
- [ ] Session recording to disk (JSONL + contact-frame stills)
- [ ] Tauri / sidecar packaging for a single native install
- [ ] Homebrew formula via `homebrew-tap`
- [ ] Benchmarks: target sustained 60 FPS @ 720p on Apple Silicon
- [ ] Optional GPU EP for `ort` (CoreML / CUDA)
