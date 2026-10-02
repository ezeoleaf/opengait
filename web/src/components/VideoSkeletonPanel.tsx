import { useEffect, useRef } from 'react'
import type { GaitMetrics } from '../types/gait'
import { drawSkeletonOverlay } from '../lib/skeleton'

interface VideoSkeletonPanelProps {
  metrics: GaitMetrics | null
  frameDataUrl: string | null
  live: boolean
  /** When false, camera JPEG is not shown (skeleton on dark stage only). */
  previewEnabled?: boolean
  sourceWidth?: number
  sourceHeight?: number
}

/**
 * Responsive video stage with a transparent canvas overlay redrawn at ~60 FPS
 * via requestAnimationFrame, driven by the latest WebSocket metrics.
 */
export function VideoSkeletonPanel({
  metrics,
  frameDataUrl,
  live,
  previewEnabled = true,
  sourceWidth = 1280,
  sourceHeight = 720,
}: VideoSkeletonPanelProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const metricsRef = useRef(metrics)
  const frameRef = useRef(frameDataUrl)
  const srcRef = useRef({ w: sourceWidth, h: sourceHeight })
  const sizeRef = useRef({ w: 1, h: 1, dpr: 1 })

  metricsRef.current = metrics
  frameRef.current = frameDataUrl
  srcRef.current = { w: sourceWidth, h: sourceHeight }

  useEffect(() => {
    const canvas = canvasRef.current
    const container = containerRef.current
    if (!canvas || !container) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    let raf = 0
    let running = true

    const resize = () => {
      const rect = container.getBoundingClientRect()
      const dpr = Math.min(window.devicePixelRatio || 1, 2)
      const w = Math.max(1, Math.floor(rect.width))
      const h = Math.max(1, Math.floor(rect.height))
      sizeRef.current = { w, h, dpr }
      canvas.width = Math.floor(w * dpr)
      canvas.height = Math.floor(h * dpr)
      canvas.style.width = `${w}px`
      canvas.style.height = `${h}px`
    }

    const ro = new ResizeObserver(resize)
    ro.observe(container)
    resize()

    const tick = () => {
      if (!running) return
      const { w, h, dpr } = sizeRef.current
      const m = metricsRef.current
      const src = srcRef.current

      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)

      if (!frameRef.current) {
        ctx.fillStyle = '#0b1220'
        ctx.fillRect(0, 0, w, h)
        ctx.strokeStyle = 'rgba(51, 65, 85, 0.35)'
        ctx.lineWidth = 1
        for (let x = 0; x < w; x += 40) {
          ctx.beginPath()
          ctx.moveTo(x, 0)
          ctx.lineTo(x, h)
          ctx.stroke()
        }
        for (let y = 0; y < h; y += 40) {
          ctx.beginPath()
          ctx.moveTo(0, y)
          ctx.lineTo(w, y)
          ctx.stroke()
        }
      } else {
        ctx.clearRect(0, 0, w, h)
      }

      if (m?.landmarks?.length || m?.roi) {
        const over =
          m.left_overstride?.ankle_hip_delta_x ?? m.right_overstride?.ankle_hip_delta_x ?? null
        drawSkeletonOverlay(
          ctx,
          m.landmarks ?? [],
          src.w,
          src.h,
          {
            leftKneeDeg: m.left_knee_flexion_deg,
            rightKneeDeg: m.right_knee_flexion_deg,
            torsoLeanDeg: m.torso_lean_deg,
            overstrideDeltaX: over,
            highlightSide: m.right_foot_strike
              ? 'right'
              : m.left_foot_strike
                ? 'left'
                : 'both',
            roi: m.roi,
            showRoi: true,
          },
          w,
          h,
        )
      }

      raf = requestAnimationFrame(tick)
    }

    raf = requestAnimationFrame(tick)
    return () => {
      running = false
      cancelAnimationFrame(raf)
      ro.disconnect()
    }
  }, [])

  const visibleJoints =
    metrics?.landmarks?.filter((k) => k.confidence >= 0.3 && (k.x !== 0 || k.y !== 0)).length ?? 0

  return (
    <div
      ref={containerRef}
      className="relative aspect-video w-full overflow-hidden rounded-lg border border-line bg-panel shadow-[inset_0_0_80px_rgba(15,23,42,0.8)]"
    >
      {frameDataUrl ? (
        <img
          src={frameDataUrl}
          alt="Live gait camera"
          className="absolute inset-0 h-full w-full object-contain"
          draggable={false}
        />
      ) : null}

      <canvas ref={canvasRef} className="absolute inset-0 h-full w-full" />

      <div className="pointer-events-none absolute left-3 top-3 flex items-center gap-2">
        <span
          className={`h-2 w-2 rounded-full ${live ? 'animate-pulse bg-signal' : 'bg-mute'}`}
        />
        <span className="font-mono text-[11px] uppercase tracking-widest text-mute">
          {live
            ? previewEnabled
              ? 'Live · video + overlay'
              : 'Live · skeleton only'
            : 'Offline'}
        </span>
      </div>

      {metrics && (
        <div className="pointer-events-none absolute bottom-3 left-3 font-mono text-[11px] text-mute">
          frame {metrics.frame_index} · t={metrics.timestamp_secs.toFixed(2)}s · joints{' '}
          {visibleJoints}
          {(metrics.left_foot_strike || metrics.right_foot_strike) && (
            <span className="ml-2 text-risk">● FOOT CONTACT</span>
          )}
        </div>
      )}
    </div>
  )
}
