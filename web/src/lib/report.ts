import { jsPDF } from 'jspdf'
import type { RecordedFrame } from '../types/gait'
import { flexionFromInterior } from './thresholds'

export interface SessionReport {
  generatedAt: string
  durationSecs: number
  frameCount: number
  contactCount: number
  averageCadenceSpm: number | null
  peakCadenceSpm: number | null
  averageTorsoLeanDeg: number | null
  averageKneeFlexionIcDeg: number | null
  averageOverstrideRatio: number | null
  contactSnapshots: Array<{
    index: number
    timestamp_secs: number
    side: 'left' | 'right' | 'both'
    knee_flexion_deg: number | null
    overstride_ratio: number | null
    torso_lean_deg: number | null
  }>
}

export function buildSessionReport(
  recorded: RecordedFrame[],
  contactFrames: number[],
): SessionReport {
  const cadences = recorded
    .map((f) => f.metrics.cadence_spm)
    .filter((v): v is number => v != null)
  const leans = recorded
    .map((f) => f.metrics.torso_lean_deg)
    .filter((v): v is number => v != null)
  const overs: number[] = []
  const flexions: number[] = []

  for (const idx of contactFrames) {
    const f = recorded[idx]
    if (!f) continue
    const m = f.metrics
    const o = m.left_overstride ?? m.right_overstride
    if (o) overs.push(o.overstride_ratio)
    const knee = m.left_foot_strike
      ? m.left_knee_flexion_deg
      : m.right_knee_flexion_deg
    const flex = flexionFromInterior(knee)
    if (flex != null) flexions.push(flex)
  }

  const avg = (arr: number[]) =>
    arr.length ? arr.reduce((a, b) => a + b, 0) / arr.length : null

  const t0 = recorded[0]?.metrics.timestamp_secs ?? 0
  const t1 = recorded[recorded.length - 1]?.metrics.timestamp_secs ?? 0

  return {
    generatedAt: new Date().toISOString(),
    durationSecs: Math.max(0, t1 - t0),
    frameCount: recorded.length,
    contactCount: contactFrames.length,
    averageCadenceSpm: avg(cadences),
    peakCadenceSpm: cadences.length ? Math.max(...cadences) : null,
    averageTorsoLeanDeg: avg(leans),
    averageKneeFlexionIcDeg: avg(flexions),
    averageOverstrideRatio: avg(overs),
    contactSnapshots: contactFrames.map((index) => {
      const m = recorded[index]?.metrics
      const side =
        m?.left_foot_strike && m?.right_foot_strike
          ? 'both'
          : m?.right_foot_strike
            ? 'right'
            : 'left'
      const knee =
        side === 'right' ? m?.right_knee_flexion_deg : m?.left_knee_flexion_deg
      const o = m?.left_overstride ?? m?.right_overstride
      return {
        index,
        timestamp_secs: m?.timestamp_secs ?? 0,
        side,
        knee_flexion_deg: flexionFromInterior(knee ?? null),
        overstride_ratio: o?.overstride_ratio ?? null,
        torso_lean_deg: m?.torso_lean_deg ?? null,
      }
    }),
  }
}

export function downloadJsonReport(report: SessionReport, filename = 'open-gait-report.json') {
  const blob = new Blob([JSON.stringify(report, null, 2)], { type: 'application/json' })
  triggerDownload(blob, filename)
}

export function downloadPdfReport(report: SessionReport, filename = 'open-gait-report.pdf') {
  const doc = new jsPDF()
  let y = 20
  const line = (text: string, size = 11) => {
    doc.setFontSize(size)
    doc.text(text, 14, y)
    y += size * 0.55 + 4
  }

  line('Open Gait Dashboard — Session Report', 18)
  line(`Generated ${report.generatedAt}`, 10)
  y += 4
  line(`Duration: ${report.durationSecs.toFixed(1)} s`)
  line(`Frames: ${report.frameCount}  ·  Foot contacts: ${report.contactCount}`)
  line(
    `Avg cadence: ${report.averageCadenceSpm?.toFixed(0) ?? '—'} SPM  ·  Peak: ${report.peakCadenceSpm?.toFixed(0) ?? '—'}`,
  )
  line(`Avg knee flexion @ IC: ${report.averageKneeFlexionIcDeg?.toFixed(1) ?? '—'}°`)
  line(`Avg torso lean: ${report.averageTorsoLeanDeg?.toFixed(1) ?? '—'}°`)
  line(`Avg overstride ratio: ${report.averageOverstrideRatio?.toFixed(3) ?? '—'}`)
  y += 6
  line('Ground-contact snapshots', 13)
  for (const snap of report.contactSnapshots.slice(0, 20)) {
    line(
      `#${snap.index}  t=${snap.timestamp_secs.toFixed(2)}s  ${snap.side}  knee ${snap.knee_flexion_deg?.toFixed(1) ?? '—'}°  overstride ${snap.overstride_ratio?.toFixed(3) ?? '—'}`,
      9,
    )
    if (y > 270) {
      doc.addPage()
      y = 20
    }
  }
  y += 8
  doc.save(filename)
}

function triggerDownload(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}
