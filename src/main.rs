//! open-gait — real-time running gait analysis.
//!
//! Captures a camera stream, runs pose estimation, computes biomechanics for
//! side / front / back views, and streams JSON (+ optional JPEG) to a local
//! WebSocket dashboard. The dashboard can switch viewpoint at runtime.

mod biomechanics;
mod calibration;
mod camera;
mod demo;
mod pose;
mod preview;
mod view;

use crate::biomechanics::GaitTracker;
use crate::calibration::{Calibration, Facing};
use crate::camera::{open_capture, CaptureConfig};
use crate::pose::open_estimator;
use crate::preview::encode_preview_jpeg;
use crate::view::CameraView;
use anyhow::Result;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::accept_async;
use tracing::{info, warn};

#[derive(Debug, Parser)]
#[command(name = "open-gait", about = "Real-time running gait analysis")]
struct Args {
    /// Camera device index (used with `--live` / `--features camera`).
    #[arg(long, default_value_t = 0)]
    device: u32,

    /// Capture width in pixels.
    #[arg(long, default_value_t = 1280)]
    width: u32,

    /// Capture height in pixels.
    #[arg(long, default_value_t = 720)]
    height: u32,

    /// Target capture frame rate.
    #[arg(long, default_value_t = 60)]
    fps: u32,

    /// Prefer a live camera instead of the synthetic source.
    #[arg(long)]
    live: bool,

    /// Scripted synthetic demo reel (default when not using `--live` / `--model`).
    /// Cycles steady → overstride → high cadence → lean → frontal form.
    #[arg(long)]
    demo: bool,

    /// Optional path to a BlazePose / MoveNet `.onnx` landmark model (`--features onnx`).
    #[arg(long)]
    model: Option<String>,

    /// Optional BlazePose person detector ONNX. Defaults to `pose_detection.onnx`
    /// next to `--model` when present.
    #[arg(long)]
    detector: Option<String>,

    /// Camera viewpoint: side | front | back (overridable from the web UI).
    #[arg(long, default_value = "side")]
    view: String,

    /// Runner facing direction for side view: left | right | auto.
    #[arg(long, default_value = "auto")]
    facing: String,

    /// Known runner height in centimetres (enables overstride in cm).
    #[arg(long)]
    height_cm: Option<f32>,

    /// JPEG preview stream rate over WebSocket (0 = off).
    #[arg(long, default_value_t = 12)]
    preview_fps: u32,

    /// Max width of JPEG preview frames.
    #[arg(long, default_value_t = 640)]
    preview_width: u32,

    /// JPEG quality 1–100.
    #[arg(long, default_value_t = 60)]
    preview_quality: u8,

    /// Cadence / strike rolling window in seconds.
    #[arg(long, default_value_t = 5.0)]
    window_secs: f64,

    /// Also emit one JSON object per line on stdout (never includes JPEG).
    #[arg(long, default_value_t = false)]
    stdout_json: bool,

    /// Bind address for the metrics WebSocket server.
    #[arg(long, default_value = "127.0.0.1:8080")]
    ws_addr: String,

    /// Disable the WebSocket server (stdout only).
    #[arg(long)]
    no_ws: bool,
}

/// Live session knobs shared between the WS server and the capture pipeline.
#[derive(Debug, Clone)]
struct SessionConfig {
    /// Shared with synthetic camera/pose so dashboard `set_view` updates the figure.
    view: Arc<Mutex<CameraView>>,
    facing: Facing,
    height_cm: Option<f32>,
    /// When false, JPEG frames are not encoded or sent over WebSocket.
    preview_enabled: bool,
}

/// Envelope sent to dashboards over WebSocket / stdout.
#[derive(Debug, Clone, Serialize)]
struct MetricsMessage {
    #[serde(rename = "type")]
    kind: &'static str,
    metrics: biomechanics::GaitMetrics,
    frame_width: u32,
    frame_height: u32,
    view: CameraView,
    facing: Facing,
    cm_per_px: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame: Option<String>,
}

/// Client → server commands (from the Open Gait Dashboard).
#[derive(Debug, Deserialize)]
struct ClientCommand {
    #[serde(rename = "type")]
    kind: String,
    view: Option<String>,
    facing: Option<String>,
    /// Enable / disable JPEG video preview over WebSocket.
    preview: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "open_gait=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    let view = CameraView::parse(&args.view)
        .ok_or_else(|| anyhow::anyhow!("invalid --view {:?} (use side|front|back)", args.view))?;
    let facing = Facing::parse(&args.facing).ok_or_else(|| {
        anyhow::anyhow!("invalid --facing {:?} (use left|right|auto)", args.facing)
    })?;

    let use_demo = args.demo || (!args.live && args.model.is_none());
    let view_slot = Arc::new(Mutex::new(view));
    let session = Arc::new(Mutex::new(SessionConfig {
        view: Arc::clone(&view_slot),
        facing,
        height_cm: args.height_cm.or(if use_demo { Some(175.0) } else { None }),
        preview_enabled: args.preview_fps > 0,
    }));

    if use_demo {
        info!(
            "synthetic demo reel enabled (70s cycle: steady → overstride → high-cadence → lean → frontal-form → recovery)"
        );
    }

    let (tx, _) = broadcast::channel::<String>(256);

    if !args.no_ws {
        let addr: SocketAddr = args.ws_addr.parse()?;
        let tx_ws = tx.clone();
        let session_ws = Arc::clone(&session);
        tokio::spawn(async move {
            if let Err(e) = run_ws_server(addr, tx_ws, session_ws).await {
                warn!("websocket server stopped: {e:#}");
            }
        });
        info!("metrics websocket listening on ws://{addr}");
    }

    let capture_cfg = CaptureConfig {
        width: args.width,
        height: args.height,
        fps: args.fps,
        device_index: args.device,
    };

    let model = args.model.clone();
    let detector = args.detector.clone();
    let live = args.live;
    let window_secs = args.window_secs;
    let stdout_json = args.stdout_json;
    let preview_fps = args.preview_fps;
    let preview_width = args.preview_width;
    let preview_quality = args.preview_quality;
    let tx_pipe = tx.clone();
    let session_pipe = Arc::clone(&session);
    let view_pipe = Arc::clone(&view_slot);
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();

    std::thread::Builder::new()
        .name("gait-pipeline".into())
        .spawn(move || {
            let result = run_pipeline(
                capture_cfg,
                live,
                model.as_deref(),
                detector.as_deref(),
                session_pipe,
                view_pipe,
                window_secs,
                stdout_json,
                preview_fps,
                preview_width,
                preview_quality,
                tx_pipe,
            );
            let _ = done_tx.send(result);
        })?;

    match done_rx.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            warn!("pipeline stopped: {e:#}");
            Err(e)
        }
        Err(_) => {
            warn!("pipeline thread ended unexpectedly");
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_pipeline(
    capture_cfg: CaptureConfig,
    live: bool,
    model: Option<&str>,
    detector: Option<&str>,
    session: Arc<Mutex<SessionConfig>>,
    view_slot: Arc<Mutex<CameraView>>,
    window_secs: f64,
    stdout_json: bool,
    preview_fps: u32,
    preview_width: u32,
    preview_quality: u8,
    tx: broadcast::Sender<String>,
) -> Result<()> {
    let fps = capture_cfg.fps;
    let mut source = open_capture(capture_cfg, live, Arc::clone(&view_slot))?;
    let mut estimator = open_estimator(model, detector, Arc::clone(&view_slot), fps)?;
    let mut tracker = GaitTracker::new(window_secs);

    let (initial_facing, initial_height) = {
        let s = session.lock().unwrap();
        (s.facing, s.height_cm)
    };
    let mut calibration = Calibration::new(initial_facing, initial_height);

    let pipeline_start = Instant::now();
    let preview_period = if preview_fps == 0 {
        None
    } else {
        Some(Duration::from_secs_f64(1.0 / preview_fps as f64))
    };
    let mut next_preview = Instant::now();
    let mut last_phase = demo::phase_at(0.0);

    info!(
        "pipeline started ({}x{} @ {} FPS, live={live}, view={:?}, facing={:?}, preview_fps={preview_fps})",
        source.config().width,
        source.config().height,
        source.config().fps,
        *view_slot.lock().unwrap(),
        initial_facing,
    );

    while let Some(frame) = source.next_frame()? {
        let (view, facing, preview_on) = {
            let s = session.lock().unwrap();
            let view = *s.view.lock().unwrap();
            (view, s.facing, s.preview_enabled)
        };
        if calibration.facing != facing {
            calibration.facing = facing;
        }

        let pose = estimator.estimate(&frame, pipeline_start)?;
        let phase = demo::phase_at(pose.timestamp_secs);
        if phase != last_phase {
            info!("demo phase → {}", phase.label());
            last_phase = phase;
        }
        calibration.update(&pose);
        let metrics = tracker.update(&pose, Some(&calibration), view);

        let include_preview = preview_on
            && preview_period.is_some_and(|p| {
                let now = Instant::now();
                if now >= next_preview {
                    next_preview = now + p;
                    true
                } else {
                    false
                }
            });

        let frame_b64 = if include_preview {
            match encode_preview_jpeg(&frame, preview_width, preview_quality) {
                Ok(b) => Some(b),
                Err(e) => {
                    warn!("preview jpeg failed: {e:#}");
                    None
                }
            }
        } else {
            None
        };

        let msg = MetricsMessage {
            kind: "gait_metrics",
            metrics,
            frame_width: frame.width,
            frame_height: frame.height,
            view,
            facing: calibration.resolved_facing,
            cm_per_px: calibration.cm_per_px,
            frame: frame_b64,
        };

        if stdout_json {
            let mut stdout_msg = msg.clone();
            stdout_msg.frame = None;
            println!("{}", serde_json::to_string(&stdout_msg)?);
        }

        let _ = tx.send(serde_json::to_string(&msg)?);
    }

    Ok(())
}

async fn run_ws_server(
    addr: SocketAddr,
    tx: broadcast::Sender<String>,
    session: Arc<Mutex<SessionConfig>>,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let tx = Arc::new(tx);

    loop {
        let (stream, peer) = listener.accept().await?;
        info!("websocket client connected: {peer}");
        let tx = Arc::clone(&tx);
        let session = Arc::clone(&session);
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, tx, session).await {
                warn!("websocket client {peer} error: {e:#}");
            }
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    tx: Arc<broadcast::Sender<String>>,
    session: Arc<Mutex<SessionConfig>>,
) -> Result<()> {
    let ws = accept_async(stream).await?;
    let (mut sink, mut incoming) = ws.split();
    let mut rx = tx.subscribe();

    // Tell the client the current session knobs on connect.
    let hello = {
        let s = session.lock().unwrap();
        serde_json::json!({
            "type": "session",
            "view": *s.view.lock().unwrap(),
            "facing": s.facing,
            "preview": s.preview_enabled,
        })
        .to_string()
    };
    sink.send(tokio_tungstenite::tungstenite::Message::Text(hello.into()))
        .await?;

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json) => {
                        sink.send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
                            .await?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            client = incoming.next() => {
                match client {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) | None => break,
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                        apply_client_command(&text, &session);
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(e.into()),
                }
            }
        }
    }
    Ok(())
}

fn apply_client_command(text: &str, session: &Mutex<SessionConfig>) {
    let Ok(cmd) = serde_json::from_str::<ClientCommand>(text) else {
        return;
    };
    if cmd.kind != "set_view" && cmd.kind != "set_config" && cmd.kind != "set_preview" {
        return;
    }
    let mut s = session.lock().unwrap();
    if let Some(v) = cmd.view.as_deref().and_then(CameraView::parse) {
        info!("session view → {}", v.as_str());
        *s.view.lock().unwrap() = v;
    }
    if let Some(f) = cmd.facing.as_deref().and_then(Facing::parse) {
        info!("session facing → {f:?}");
        s.facing = f;
    }
    if let Some(preview) = cmd.preview {
        info!("session preview → {preview}");
        s.preview_enabled = preview;
    }
}
