# Open Gait Dashboard

React + Vite frontend for the open-gait Rust backend.

## Demo walkthrough

```bash
# Terminal 1 — scripted synthetic runner (no camera)
make demo

# Terminal 2 — dashboard
cd web && npm install && npm run dev
```

Open http://localhost:5173. The backend loops a 70s gait reel; switch **Side / Front / Back** and toggle **Video on/off** while recording screenshots.

## Develop

```bash
# Terminal 1 — Rust metrics stream
cargo run --release -- --demo --preview-fps 15

# Terminal 2 — dashboard
cd web && npm run dev
```

## Build

```bash
cd web && npm run build
```

## License

GNU GPL v3.0 only — see the repository root [`LICENSE`](../LICENSE).
