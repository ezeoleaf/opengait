import type { ReactNode } from 'react'
import { Area, AreaChart, ResponsiveContainer, YAxis } from 'recharts'
import type { CameraView, GaitMetrics } from '../types/gait'
import { StatusBadge } from './StatusBadge'
import {
  cadenceStatus,
  crossoverStatus,
  flexionFromInterior,
  hipDropStatus,
  kneeFlexionStatus,
  kneeValgusStatus,
  lateralLeanStatus,
  overstrideStatus,
  torsoLeanStatus,
} from '../lib/thresholds'

interface MetricCardsProps {
  metrics: GaitMetrics | null
  cadenceHistory: Array<{ t: number; spm: number }>
  view: CameraView
}

function Card({
  label,
  value,
  unit,
  detail,
  status,
  children,
}: {
  label: string
  value: string
  unit?: string
  detail?: string
  status: ReturnType<typeof cadenceStatus>
  children?: ReactNode
}) {
  return (
    <div className="rounded-lg border border-line bg-panel/80 p-4 backdrop-blur">
      <div className="mb-2 flex items-start justify-between gap-2">
        <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-mute">{label}</p>
        <StatusBadge status={status} />
      </div>
      <div className="flex items-baseline gap-2">
        <span className="font-mono text-3xl font-semibold tracking-tight text-ink">{value}</span>
        {unit ? <span className="font-mono text-sm text-neon">{unit}</span> : null}
      </div>
      {detail ? <p className="mt-1 text-sm text-mute">{detail}</p> : null}
      {children}
    </div>
  )
}

function fmtDeg(v: number | null | undefined, signed = true): string {
  if (v == null) return '—'
  const s = signed && v > 0 ? '+' : ''
  return `${s}${v.toFixed(1)}`
}

export function MetricCards({ metrics, cadenceHistory, view }: MetricCardsProps) {
  const frontal = view === 'front' || view === 'back'
  const cadence = metrics?.cadence_spm ?? null

  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-1">
      <Card
        label="Cadence"
        value={cadence != null ? cadence.toFixed(0) : '—'}
        unit="SPM"
        status={cadenceStatus(cadence)}
        detail="Target band 170–190"
      >
        <div className="mt-3 h-12">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={cadenceHistory}>
              <YAxis domain={['dataMin - 5', 'dataMax + 5']} hide />
              <Area
                type="monotone"
                dataKey="spm"
                stroke="#22d3ee"
                fill="rgba(34, 211, 238, 0.18)"
                strokeWidth={1.5}
                isAnimationActive={false}
                dot={false}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </Card>

      {frontal ? <FrontalCards metrics={metrics} /> : <SagittalCards metrics={metrics} />}
    </div>
  )
}

function SagittalCards({ metrics }: { metrics: GaitMetrics | null }) {
  const over = metrics?.left_overstride ?? metrics?.right_overstride ?? null
  const kneeInterior = metrics?.left_foot_strike
    ? metrics.left_knee_flexion_deg
    : metrics?.right_foot_strike
      ? metrics.right_knee_flexion_deg
      : (metrics?.left_knee_flexion_deg ?? metrics?.right_knee_flexion_deg)
  const kneeFlex = flexionFromInterior(kneeInterior)
  const lean = metrics?.torso_lean_deg ?? null

  return (
    <>
      <Card
        label="Overstride"
        value={
          over
            ? over.ahead_cm != null
              ? `${over.ahead_cm > 0 ? '+' : ''}${over.ahead_cm.toFixed(1)}`
              : `${(over.ahead_px ?? over.ankle_hip_delta_x) > 0 ? '+' : ''}${(over.ahead_px ?? over.ankle_hip_delta_x).toFixed(1)}`
            : '—'
        }
        unit={over?.ahead_cm != null ? 'cm' : 'px'}
        status={overstrideStatus(over?.overstride_ratio)}
        detail={
          over
            ? `${(over.ahead_px ?? over.ankle_hip_delta_x) >= 0 ? 'ahead' : 'behind'} of COM · ratio ${over.overstride_ratio.toFixed(3)}`
            : 'Awaiting foot contact'
        }
      />

      <Card
        label="Knee Flexion @ IC"
        value={kneeFlex != null ? kneeFlex.toFixed(1) : '—'}
        unit="°"
        status={kneeFlexionStatus(kneeInterior)}
        detail="Flexion from straight at initial contact"
      />

      <Card
        label="Torso Forward Lean"
        value={lean != null ? Math.abs(lean).toFixed(1) : '—'}
        unit="°"
        status={torsoLeanStatus(lean)}
        detail={
          lean == null
            ? 'vs vertical'
            : lean < 0
              ? 'leaning back (vs run direction)'
              : 'into run direction'
        }
      />
    </>
  )
}

function FrontalCards({ metrics }: { metrics: GaitMetrics | null }) {
  const hipDrop = metrics?.hip_drop_deg ?? null
  const lateral = metrics?.trunk_lateral_lean_deg ?? null
  const valgusL = metrics?.left_knee_valgus_deg ?? null
  const valgusR = metrics?.right_knee_valgus_deg ?? null
  const valgus =
    valgusL != null && valgusR != null
      ? Math.abs(valgusL) > Math.abs(valgusR)
        ? valgusL
        : valgusR
      : (valgusL ?? valgusR)
  const crossL = metrics?.left_crossover_px ?? null
  const crossR = metrics?.right_crossover_px ?? null
  const crossover =
    crossL != null && crossR != null ? Math.max(crossL, crossR) : (crossL ?? crossR)

  return (
    <>
      <Card
        label="Hip Drop"
        value={fmtDeg(hipDrop)}
        unit="°"
        status={hipDropStatus(hipDrop)}
        detail={
          hipDrop == null
            ? 'Pelvic obliquity'
            : hipDrop > 0
              ? 'right hip lower'
              : 'left hip lower'
        }
      />

      <Card
        label="Trunk Lateral Lean"
        value={fmtDeg(lateral)}
        unit="°"
        status={lateralLeanStatus(lateral)}
        detail="Frontal lean vs vertical"
      />

      <Card
        label="Knee Valgus"
        value={fmtDeg(valgus)}
        unit="°"
        status={kneeValgusStatus(valgus)}
        detail={
          valgusL != null || valgusR != null
            ? `L ${fmtDeg(valgusL)} · R ${fmtDeg(valgusR)}`
            : 'Medial knee collapse'
        }
      />

      <Card
        label="Crossover"
        value={crossover != null ? crossover.toFixed(0) : '—'}
        unit="px"
        status={crossoverStatus(crossover)}
        detail={
          crossL != null || crossR != null
            ? `L ${crossL?.toFixed(0) ?? '—'} · R ${crossR?.toFixed(0) ?? '—'} past midline`
            : 'Ankle vs mid-pelvis'
        }
      />
    </>
  )
}
