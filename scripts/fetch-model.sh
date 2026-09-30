#!/usr/bin/env bash
# Download BlazePose landmark + person-detector ONNX models into models/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST_DIR="$ROOT/models"
BASE="https://raw.githubusercontent.com/bolducmanuel/blaze_models_onnxruntime/main/pose_models"

mkdir -p "$DEST_DIR"

fetch() {
  local name="$1"
  local dest="$DEST_DIR/$name"
  if [[ -f "$dest" ]]; then
    echo "Already present: $dest"
    ls -lh "$dest"
    return
  fi
  echo "Downloading $name…"
  curl -fL --progress-bar -o "$dest" "$BASE/$2"
  ls -lh "$dest"
}

fetch "blazepose_landmark_full.onnx" "blazepose-pose_landmark_full.onnx"
fetch "pose_detection.onnx" "pose_detection.onnx"

echo
echo "Run with:"
echo "  cargo run --release --features camera,onnx -- \\"
echo "    --live --device 0 \\"
echo "    --model models/blazepose_landmark_full.onnx \\"
echo "    --detector models/pose_detection.onnx"
