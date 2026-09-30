# Pose models for open-gait

BlazePose needs **two** ONNX files:

1. **Detector** (`pose_detection.onnx`) — finds a person ROI (224×224)
2. **Landmarks** (`blazepose_landmark_full.onnx`) — 33 keypoints inside that ROI (256×256)

## Download

```bash
./scripts/fetch-model.sh
```

## Run

```bash
cargo run --release --features camera,onnx -- \
  --live --device 0 \
  --model models/blazepose_landmark_full.onnx \
  --detector models/pose_detection.onnx
```

If `--detector` is omitted, open-gait looks for `pose_detection.onnx` next to the landmark model automatically.

## Why the detector matters

Running landmarks on the full frame is jittery (and inflates cadence). The detector crops a person-centered square; landmarks then track that ROI with EMA smoothing between detector refreshes (~every 20 frames).
