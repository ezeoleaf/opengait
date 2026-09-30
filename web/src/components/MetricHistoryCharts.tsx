import { Area, AreaChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts'
import type { MetricHistoryPoint } from '../types/gait'
import { flexionFromInterior } from '../lib/thresholds'

interface MetricHistoryChartsProps {
  history: MetricHistoryPoint[]
}

function ChartCard({
  title,
  color,
  dataKey,
  unit,
  data,
}: {
  title: string
  color: string
  dataKey: keyof MetricHistoryPoint
  unit: string
  data: MetricHistoryPoint[]
}) {
  const series = data.filter((d) => d[dataKey] != null)
  const latest = series.length ? (series[series.length - 1][dataKey] as number) : null

  return (
    <div className="rounded-lg border border-line bg-panel/70 p-3">
      <div className="mb-2 flex items-baseline justify-between gap-2">
        <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-mute">{title}</p>
        <p className="font-mono text-sm text-ink">
          {latest != null ? latest.toFixed(1) : '—'}
          <span className="ml-1 text-mute">{unit}</span>
        </p>
      </div>
      <div className="h-24">
        {series.length > 1 ? (
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={series}>
              <XAxis dataKey="t" hide />
              <YAxis domain={['auto', 'auto']} hide />
              <Tooltip
                contentStyle={{
                  background: '#1e293b',
                  border: '1px solid #334155',
                  fontSize: 11,
                  fontFamily: 'IBM Plex Mono, monospace',
                }}
                labelFormatter={(t) => `t=${Number(t).toFixed(2)}s`}
                formatter={(value) => [
                  `${typeof value === 'number' ? value.toFixed(1) : value}${unit}`,
                  title,
                ]}
              />
              <Area
                type="monotone"
                dataKey={dataKey}
                stroke={color}
                fill={`${color}33`}
                strokeWidth={1.5}
                isAnimationActive={false}
                dot={false}
                connectNulls
              />
            </AreaChart>
          </ResponsiveContainer>
        ) : (
          <p className="flex h-full items-center font-mono text-xs text-mute">Collecting samples…</p>
        )}
      </div>
    </div>
  )
}

export function MetricHistoryCharts({ history }: MetricHistoryChartsProps) {
  return (
    <section className="rounded-lg border border-line bg-panel/50 p-4">
      <div className="mb-3">
        <h2 className="font-display text-lg font-semibold tracking-tight">Session trends</h2>
        <p className="text-sm text-mute">
          Rolling knee flexion, torso lean, and overstride over the live stream.
        </p>
      </div>
      <div className="grid gap-3 md:grid-cols-3">
        <ChartCard
          title="Knee flexion"
          color="#4ade80"
          dataKey="knee"
          unit="°"
          data={history}
        />
        <ChartCard title="Torso lean" color="#fbbf24" dataKey="lean" unit="°" data={history} />
        <ChartCard
          title="Overstride"
          color="#f87171"
          dataKey="overstride"
          unit="px"
          data={history}
        />
      </div>
    </section>
  )
}

/** Build a history point from the latest metrics sample. */
export function metricHistoryFromGait(m: {
  timestamp_secs: number
  left_knee_flexion_deg: number | null
  right_knee_flexion_deg: number | null
  left_foot_strike: boolean
  right_foot_strike: boolean
  torso_lean_deg: number | null
  left_overstride: { ahead_px?: number; ankle_hip_delta_x: number } | null
  right_overstride: { ahead_px?: number; ankle_hip_delta_x: number } | null
}): MetricHistoryPoint {
  const kneeInterior = m.left_foot_strike
    ? m.left_knee_flexion_deg
    : m.right_foot_strike
      ? m.right_knee_flexion_deg
      : (m.left_knee_flexion_deg ?? m.right_knee_flexion_deg)
  const over = m.left_overstride ?? m.right_overstride
  return {
    t: m.timestamp_secs,
    knee: flexionFromInterior(kneeInterior),
    lean: m.torso_lean_deg,
    overstride: over ? (over.ahead_px ?? over.ankle_hip_delta_x) : null,
  }
}
