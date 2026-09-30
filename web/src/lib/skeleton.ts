import type { DetectorRoi, Keypoint } from '../types/gait'
import { Landmark, SKELETON_EDGES } from '../types/gait'

const CONF_MIN = 0.3

export interface SkeletonDrawOptions {
  showAngles?: boolean
  showOverstride?: boolean
  showRoi?: boolean
  roi?: DetectorRoi | null
  leftKneeDeg?: number | null
  rightKneeDeg?: number | null
  torsoLeanDeg?: number | null
  highlightSide?: 'left' | 'right' | 'both'
  overstrideDeltaX?: number | null
  /** Clear the canvas before drawing. Default false so video/stage can show through. */
  clear?: boolean
}

function visible(kp: Keypoint | undefined): kp is Keypoint {
  return !!kp && kp.confidence >= CONF_MIN
}

function get(landmarks: Keypoint[], id: number): Keypoint | undefined {
  return landmarks[id]
}

/** Map source image coords → canvas pixel space. */
export function scalePoint(
  kp: Keypoint,
  srcW: number,
  srcH: number,
  canvasW: number,
  canvasH: number,
): { x: number; y: number } {
  return {
    x: (kp.x / srcW) * canvasW,
    y: (kp.y / srcH) * canvasH,
  }
}

function drawBone(
  ctx: CanvasRenderingContext2D,
  a: { x: number; y: number },
  b: { x: number; y: number },
  color: string,
) {
  ctx.beginPath()
  ctx.moveTo(a.x, a.y)
  ctx.lineTo(b.x, b.y)
  ctx.strokeStyle = color
  ctx.lineWidth = 3
  ctx.lineCap = 'round'
  ctx.stroke()
}

function drawJoint(ctx: CanvasRenderingContext2D, p: { x: number; y: number }, color: string) {
  ctx.beginPath()
  ctx.arc(p.x, p.y, 4.5, 0, Math.PI * 2)
  ctx.fillStyle = color
  ctx.fill()
  ctx.strokeStyle = 'rgba(15, 23, 42, 0.8)'
  ctx.lineWidth = 1.5
  ctx.stroke()
}

/** Angle arc at `vertex` between points `a` and `c`, with degree callout. */
function drawAngleArc(
  ctx: CanvasRenderingContext2D,
  a: { x: number; y: number },
  vertex: { x: number; y: number },
  c: { x: number; y: number },
  degrees: number,
  color: string,
) {
  const v1 = { x: a.x - vertex.x, y: a.y - vertex.y }
  const v2 = { x: c.x - vertex.x, y: c.y - vertex.y }
  const ang1 = Math.atan2(v1.y, v1.x)
  const ang2 = Math.atan2(v2.y, v2.x)
  let delta = ang2 - ang1
  while (delta <= -Math.PI) delta += Math.PI * 2
  while (delta > Math.PI) delta -= Math.PI * 2

  const radius = 28
  ctx.beginPath()
  ctx.arc(vertex.x, vertex.y, radius, ang1, ang1 + delta, delta < 0)
  ctx.strokeStyle = color
  ctx.lineWidth = 2
  ctx.stroke()

  const mid = ang1 + delta / 2
  const labelX = vertex.x + Math.cos(mid) * (radius + 16)
  const labelY = vertex.y + Math.sin(mid) * (radius + 16)
  ctx.font = '600 12px "IBM Plex Mono", monospace'
  ctx.fillStyle = color
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillText(`${degrees.toFixed(0)}°`, labelX, labelY)
}

/**
 * Draw the BlazePose skeleton, angle arcs, and overstride guide onto `ctx`.
 * Designed to be called once per animation frame (~60 FPS).
 */
export function drawSkeletonOverlay(
  ctx: CanvasRenderingContext2D,
  landmarks: Keypoint[],
  srcW: number,
  srcH: number,
  options: SkeletonDrawOptions = {},
  viewW?: number,
  viewH?: number,
): void {
  const canvasW = viewW ?? ctx.canvas.width
  const canvasH = viewH ?? ctx.canvas.height
  if (options.clear) {
    ctx.clearRect(0, 0, canvasW, canvasH)
  }

  if (options.showRoi !== false && options.roi) {
    drawDetectorRoi(ctx, options.roi, srcW, srcH, canvasW, canvasH)
  }

  if (!landmarks.length) return

  const pt = (id: number) => {
    const kp = get(landmarks, id)
    if (!visible(kp)) return null
    return scalePoint(kp, srcW, srcH, canvasW, canvasH)
  }

  // Bones
  for (const [i, j] of SKELETON_EDGES) {
    const a = pt(i)
    const b = pt(j)
    if (!a || !b) continue
    const isLeft = i >= Landmark.LeftHip && i <= Landmark.LeftFootIndex
    const color = isLeft ? '#4ade80' : '#22d3ee'
    drawBone(ctx, a, b, color)
  }

  // Joints
  const jointIds = [
    Landmark.LeftShoulder,
    Landmark.RightShoulder,
    Landmark.LeftHip,
    Landmark.RightHip,
    Landmark.LeftKnee,
    Landmark.RightKnee,
    Landmark.LeftAnkle,
    Landmark.RightAnkle,
    Landmark.LeftHeel,
    Landmark.RightHeel,
    Landmark.LeftFootIndex,
    Landmark.RightFootIndex,
  ]
  for (const id of jointIds) {
    const p = pt(id)
    if (!p) continue
    const isLeft = id === Landmark.LeftShoulder || (id >= Landmark.LeftHip && id <= Landmark.LeftFootIndex)
    drawJoint(ctx, p, isLeft ? '#4ade80' : '#22d3ee')
  }

  if (options.showAngles !== false) {
    const lHip = pt(Landmark.LeftHip)
    const lKnee = pt(Landmark.LeftKnee)
    const lAnkle = pt(Landmark.LeftAnkle)
    if (lHip && lKnee && lAnkle && options.leftKneeDeg != null) {
      drawAngleArc(ctx, lHip, lKnee, lAnkle, options.leftKneeDeg, '#4ade80')
    }

    const rHip = pt(Landmark.RightHip)
    const rKnee = pt(Landmark.RightKnee)
    const rAnkle = pt(Landmark.RightAnkle)
    if (rHip && rKnee && rAnkle && options.rightKneeDeg != null) {
      drawAngleArc(ctx, rHip, rKnee, rAnkle, options.rightKneeDeg, '#22d3ee')
    }

    // Torso lean: mid-hip → mid-shoulder vs vertical
    if (
      lHip &&
      rHip &&
      options.torsoLeanDeg != null &&
      pt(Landmark.LeftShoulder) &&
      pt(Landmark.RightShoulder)
    ) {
      const midHip = {
        x: (lHip.x + rHip.x) / 2,
        y: (lHip.y + rHip.y) / 2,
      }
      const ls = pt(Landmark.LeftShoulder)!
      const rs = pt(Landmark.RightShoulder)!
      const midShoulder = { x: (ls.x + rs.x) / 2, y: (ls.y + rs.y) / 2 }
      const vertical = { x: midHip.x, y: midHip.y - 80 }
      drawBone(ctx, midHip, midShoulder, 'rgba(251, 191, 36, 0.9)')
      ctx.setLineDash([4, 4])
      drawBone(ctx, midHip, vertical, 'rgba(148, 163, 184, 0.7)')
      ctx.setLineDash([])
      drawAngleArc(ctx, vertical, midHip, midShoulder, options.torsoLeanDeg, '#fbbf24')
    }
  }

  // Overstride: vertical plumbline from hip X + horizontal marker to ankle
  if (options.showOverstride !== false) {
    const side =
      options.highlightSide === 'right'
        ? 'right'
        : options.highlightSide === 'left'
          ? 'left'
          : options.overstrideDeltaX != null && options.overstrideDeltaX !== 0
            ? 'auto'
            : 'left'

    const hip =
      side === 'right' ? pt(Landmark.RightHip) : side === 'left' ? pt(Landmark.LeftHip) : pt(Landmark.LeftHip)
    const ankle =
      side === 'right'
        ? pt(Landmark.RightAnkle)
        : side === 'left'
          ? pt(Landmark.LeftAnkle)
          : pt(Landmark.LeftAnkle)

    // Prefer the side that has a fresh overstride sample
    const useRight =
      options.highlightSide === 'right' ||
      (options.highlightSide === 'both' && Math.abs(options.overstrideDeltaX ?? 0) > 0)
    const hipP = useRight ? pt(Landmark.RightHip) ?? hip : hip
    const ankleP = useRight ? pt(Landmark.RightAnkle) ?? ankle : ankle

    if (hipP && ankleP) {
      ctx.setLineDash([3, 5])
      ctx.beginPath()
      ctx.moveTo(hipP.x, 0)
      ctx.lineTo(hipP.x, canvasH)
      ctx.strokeStyle = 'rgba(248, 113, 113, 0.55)'
      ctx.lineWidth = 1.5
      ctx.stroke()
      ctx.setLineDash([])

      ctx.beginPath()
      ctx.moveTo(hipP.x, ankleP.y)
      ctx.lineTo(ankleP.x, ankleP.y)
      ctx.strokeStyle = '#f87171'
      ctx.lineWidth = 2
      ctx.stroke()

      const midX = (hipP.x + ankleP.x) / 2
      ctx.font = '600 11px "IBM Plex Mono", monospace'
      ctx.fillStyle = '#f87171'
      ctx.textAlign = 'center'
      const delta = options.overstrideDeltaX
      ctx.fillText(
        delta != null ? `overstride ${delta > 0 ? '+' : ''}${delta.toFixed(0)}px` : 'overstride',
        midX,
        ankleP.y - 10,
      )
    }
  }
}

/** Draw the oriented detector ROI as a dashed rotated square. */
export function drawDetectorRoi(
  ctx: CanvasRenderingContext2D,
  roi: DetectorRoi,
  srcW: number,
  srcH: number,
  canvasW: number,
  canvasH: number,
): void {
  const sx = canvasW / srcW
  const sy = canvasH / srcH
  const cx = roi.cx * sx
  const cy = roi.cy * sy
  const half = (roi.size * Math.min(sx, sy)) / 2
  const cos = Math.cos(roi.angle_rad)
  const sin = Math.sin(roi.angle_rad)

  const corner = (lx: number, ly: number) => ({
    x: cx + lx * cos - ly * sin,
    y: cy + lx * sin + ly * cos,
  })
  const corners = [
    corner(-half, -half),
    corner(half, -half),
    corner(half, half),
    corner(-half, half),
  ]

  ctx.save()
  ctx.setLineDash([6, 4])
  ctx.strokeStyle = 'rgba(251, 191, 36, 0.85)'
  ctx.lineWidth = 1.5
  ctx.beginPath()
  ctx.moveTo(corners[0].x, corners[0].y)
  for (let i = 1; i < corners.length; i++) {
    ctx.lineTo(corners[i].x, corners[i].y)
  }
  ctx.closePath()
  ctx.stroke()
  ctx.setLineDash([])

  // Center crosshair
  ctx.strokeStyle = 'rgba(251, 191, 36, 0.6)'
  ctx.beginPath()
  ctx.moveTo(cx - 6, cy)
  ctx.lineTo(cx + 6, cy)
  ctx.moveTo(cx, cy - 6)
  ctx.lineTo(cx, cy + 6)
  ctx.stroke()

  ctx.font = '600 10px "IBM Plex Mono", monospace'
  ctx.fillStyle = 'rgba(251, 191, 36, 0.9)'
  ctx.textAlign = 'left'
  ctx.fillText('ROI', corners[0].x + 4, corners[0].y - 4)
  ctx.restore()
}

/** Clear and paint a dark stage when no video frame is present. */
export function drawEmptyStage(ctx: CanvasRenderingContext2D): void {
  const { width, height } = ctx.canvas
  ctx.fillStyle = '#0b1220'
  ctx.fillRect(0, 0, width, height)
  // subtle grid
  ctx.strokeStyle = 'rgba(51, 65, 85, 0.35)'
  ctx.lineWidth = 1
  const step = 40
  for (let x = 0; x < width; x += step) {
    ctx.beginPath()
    ctx.moveTo(x, 0)
    ctx.lineTo(x, height)
    ctx.stroke()
  }
  for (let y = 0; y < height; y += step) {
    ctx.beginPath()
    ctx.moveTo(0, y)
    ctx.lineTo(width, y)
    ctx.stroke()
  }
}
