//! open-gait — real-time running gait analysis.
//!
//! Captures a side-view stream, runs pose estimation, computes biomechanics,
//! and streams JSON metrics (+ optional JPEG preview) to a local WebSocket dashboard.

mod biomechanics;
mod calibration;
mod camera;
mod pose;
mod preview;

use crate::biomechanics::GaitTracker;
use crate::calibration::{Calibration, Facing};
use crate::camera::{open_capture, CaptureConfig};
use crate::pose::open_estimator;
use crate::preview::encode_preview_jpeg;
use anyhow::Result;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
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

    /// Optional path to a BlazePose / MoveNet `.onnx` landmark model (`--features onnx`).
    #[arg(long)]
    model: Option<String>,

    /// Optional BlazePose person detector ONNX. Defaults to `pose_detection.onnx`
    /// next to `--model` when present.
    #[arg(long)]
    detector: Option<String>,

    /// Runner facing direction in the image: left | right | auto.
    #[arg(long, default_value = "auto")]
    facing: String,

    /// Known runner height in centimetres (enables overstride in cm).
    #[arg(long)]
    height_cm: Option<f32>,

    /// JPEG preview stream rate over WebSocket (0 = off). Metrics still run at capture FPS.
    #[arg(long, default_value_t = 12)]
    preview_fps: u32,

    /// Max width of JPEG preview frames (keeps WS payload small).
    #[arg(long, default_value_t = 640)]
    preview_width: u32,

    /// JPEG quality 1–100.
    #[arg(long, default_value_t = 60)]
    preview_quality: u8,

    /// Cadence / strike rolling window in seconds.
    #[arg(long, default_value_t = 5.0)]
    window_secs: f64,

    /// Also emit one JSON object per line on stdout (never includes JPEG).
    #[arg(long, default_value_t = true)]
    stdout_json: bool,

    /// Bind address for the metrics WebSocket server.
    #[arg(long, default_value = "127.0.0.1:8080")]
    ws_addr: String,

    /// Disable the WebSocket server (stdout only).
    #[arg(long)]
    no_ws: bool,
}

/// Envelope sent to dashboards over WebSocket / stdout.
#[derive(Debug, Clone, Serialize)]
struct MetricsMessage {
    #[serde(rename = "type")]
    kind: &'static str,
    metrics: biomechanics::GaitMetrics,
    frame_width: u32,
    frame_height: u32,
    facing: Facing,
    cm_per_px: Option<f32>,
    /// Base64 JPEG (no data-URL prefix). Omitted on stdout and when throttled.
    #[serde(skip_serializing_if = "Option::is_none")]
    frame: Option<String>,
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
    let facing = Facing::parse(&args.facing).ok_or_else(|| {
        anyhow::anyhow!("invalid --facing {:?} (use left|right|auto)", args.facing)
    })?;
    let calibration = Calibration::new(facing, args.height_cm);

    let (tx, _) = broadcast::channel::<String>(256);

    if !args.no_ws {
        let addr: SocketAddr = args.ws_addr.parse()?;
        let tx_ws = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = run_ws_server(addr, tx_ws).await {
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
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();

    std::thread::Builder::new()
        .name("gait-pipeline".into())
        .spawn(move || {
            let result = run_pipeline(
                capture_cfg,
                live,
                model.as_deref(),
                detector.as_deref(),
                calibration,
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
    mut calibration: Calibration,
    window_secs: f64,
    stdout_json: bool,
    preview_fps: u32,
    preview_width: u32,
    preview_quality: u8,
    tx: broadcast::Sender<String>,
) -> Result<()> {
    let mut source = open_capture(capture_cfg, live)?;
    let mut estimator = open_estimator(model, detector)?;
    let mut tracker = GaitTracker::new(window_secs);
    let pipeline_start = Instant::now();
    let preview_period = if preview_fps == 0 {
        None
    } else {
        Some(Duration::from_secs_f64(1.0 / preview_fps as f64))
    };
    let mut next_preview = Instant::now();

    info!(
        "pipeline started ({}x{} @ {} FPS, live={live}, facing={:?}, height_cm={:?}, preview_fps={preview_fps})",
        source.config().width,
        source.config().height,
        source.config().fps,
        calibration.facing,
        calibration.height_cm
    );

    while let Some(frame) = source.next_frame()? {
        let pose = estimator.estimate(&frame, pipeline_start)?;
        calibration.update(&pose);
        let metrics = tracker.update(&pose, Some(&calibration));

        let include_preview = preview_period.is_some_and(|p| {
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
            facing: calibration.resolved_facing,
            cm_per_px: calibration.cm_per_px,
            frame: frame_b64.clone(),
        };

        if stdout_json {
            let mut stdout_msg = msg.clone();
            stdout_msg.frame = None; // keep stdout lean
            println!("{}", serde_json::to_string(&stdout_msg)?);
        }

        let _ = tx.send(serde_json::to_string(&msg)?);
    }

    Ok(())
}

async fn run_ws_server(addr: SocketAddr, tx: broadcast::Sender<String>) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let tx = Arc::new(tx);

    loop {
        let (stream, peer) = listener.accept().await?;
        info!("websocket client connected: {peer}");
        let tx = Arc::clone(&tx);
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, tx).await {
                warn!("websocket client {peer} error: {e:#}");
            }
        });
    }
}

async fn handle_client(stream: TcpStream, tx: Arc<broadcast::Sender<String>>) -> Result<()> {
    let ws = accept_async(stream).await?;
    let (mut sink, mut incoming) = ws.split();
    let mut rx = tx.subscribe();

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
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(e.into()),
                }
            }
        }
    }
    Ok(())
}
