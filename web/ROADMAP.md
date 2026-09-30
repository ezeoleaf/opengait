# Roadmap — Taper Gait (web)

React + Vite dashboard for open-gait: WebSocket metrics, skeleton overlay, recording, debrief.

## Done

- [x] Vite + React + TypeScript + Tailwind v4 scaffold
- [x] Dark Taper aesthetic (`#0F172A` / `#1E293B`, neon cyan/green overlays)
- [x] `useGaitStream` — WS client, RAF-coalesced updates, reconnect
- [x] Live metric cards: cadence, overstride, knee flexion @ IC, torso lean
- [x] Status badges (Optimal / Caution / High Risk) + cadence sparkline
- [x] Canvas skeleton overlay (~60 FPS): bones, angle arcs, overstride guide
- [x] Source resolution from backend `frame_width` / `frame_height`
- [x] Run recording (≤30s) with slow-mo scrubber (0.1x–1x) + frame step
- [x] Foot-contact snap navigation
- [x] Generate gait report (PDF + JSON) + Gotaper.app export CTA
- [x] Live camera preview under the skeleton (JPEG from backend WS)
- [x] Detector ROI rectangle on the canvas (debug alignment)
- [x] Metric history charts (knee / lean / overstride)

## Next

- [ ] Connection / model status panel (synthetic vs live, detector loaded, FPS)
- [ ] Persist last WS URL + thresholds in `localStorage`
- [ ] Empty / offline states with clearer “start the Rust backend” copy

## Later

- [ ] Compare two recorded clips side-by-side
- [ ] Coach annotations on contact frames (notes pinned to timeline)
- [ ] Upload session JSON to Gotaper.app (authenticated)
- [ ] PWA / installable dashboard
- [ ] Accessibility pass (keyboard scrubber, reduced-motion, contrast)
- [ ] i18n of chrome UI
- [ ] Tauri shell wrapping this UI + the Rust binary as one desktop app
