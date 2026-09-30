import type { ReactNode } from 'react'
import { Area, AreaChart, ResponsiveContainer, YAxis } from 'recharts'
import type { GaitMetrics } from '../types/gait'
import { StatusBadge } from './StatusBadge'
import {
  cadenceStatus,
  flexionFromInterior,
  kneeFlexionStatus,
  overstrideStatus,
  torsoLeanStatus,
} from '../lib/thresholds'

interface MetricCardsProps {
  metrics: GaitMetrics | null
  cadenceHistory: Array<{ t: number; spm: number }>
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

export function MetricCards({ metrics, cadenceHistory }: MetricCardsProps) {
  const cadence = metrics?.cadence_spm ?? null
  const over = metrics?.left_overstride ?? metrics?.right_overstride ?? null
  const kneeInterior =
    metrics?.left_foot_strike
      ? metrics.left_knee_flexion_deg
      : metrics?.right_foot_strike
        ? metrics.right_knee_flexion_deg
        : (metrics?.left_knee_flexion_deg ?? metrics?.right_knee_flexion_deg)
  const kneeFlex = flexionFromInterior(kneeInterior)
  const lean = metrics?.torso_lean_deg ?? null

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
    </div>
  )
}
