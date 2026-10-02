demo:
	cargo run --release -- \
	  --demo \
	  --facing right \
	  --preview-fps 15 \
	  --preview-quality 72

site:
	cd site && python3 -m http.server 4173

run:
	cargo run --release --features camera,onnx -- \
	  --live --device 0 \
	  --model models/blazepose_landmark_full.onnx \
	  --detector models/pose_detection.onnx \
	  --facing auto \
	  --height-cm 175 \
	  --preview-fps 12

clean:
	cargo clean

build:
	cargo build --release

clean-release:
	cargo clean --release

clippy:
	cargo clippy

test:
	cargo test

web:
	cd web && npm run dev

.PHONY: demo site run clean build clean-release clippy test web
