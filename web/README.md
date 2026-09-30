# Open Gait Dashboard

React + Vite frontend for the open-gait Rust backend.

## Develop

```bash
# Terminal 1 — Rust metrics stream
cargo run --release

# Terminal 2 — dashboard
cd web && npm run dev
```

Open http://localhost:5173 — the UI connects to `ws://127.0.0.1:8080`.

## Build

```bash
cd web && npm run build
```
