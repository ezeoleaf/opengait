import type { MetricStatus } from '../types/gait'

/** Threshold bands for live status badges. */
export function cadenceStatus(spm: number | null | undefined): MetricStatus {
  if (spm == null) return 'idle'
  if (spm >= 170 && spm <= 190) return 'optimal'
  if (spm >= 160 && spm < 170) return 'caution'
  if (spm > 190 && spm <= 200) return 'caution'
  return 'risk'
}

export function overstrideStatus(ratio: number | null | undefined): MetricStatus {
  if (ratio == null) return 'idle'
  if (ratio < 0.08) return 'optimal'
  if (ratio < 0.15) return 'caution'
  return 'risk'
}

/** Knee flexion at IC: interior Hip-Knee-Ankle angle. Lower = more flexed. */
export function kneeFlexionStatus(interiorDeg: number | null | undefined): MetricStatus {
  if (interiorDeg == null) return 'idle'
  // Convert interior angle ≈180 (straight) to flexion from straight.
  const flexion = 180 - interiorDeg
  if (flexion >= 12 && flexion <= 25) return 'optimal'
  if (flexion >= 8 && flexion < 12) return 'caution'
  if (flexion > 25 && flexion <= 35) return 'caution'
  return 'risk'
}

export function torsoLeanStatus(deg: number | null | undefined): MetricStatus {
  if (deg == null) return 'idle'
  const abs = Math.abs(deg)
  if (abs >= 4 && abs <= 12) return 'optimal'
  if (abs < 4 || (abs > 12 && abs <= 18)) return 'caution'
  return 'risk'
}

export function flexionFromInterior(interiorDeg: number | null | undefined): number | null {
  if (interiorDeg == null) return null
  return Math.max(0, 180 - interiorDeg)
}

export const STATUS_LABEL: Record<MetricStatus, string> = {
  optimal: 'Optimal',
  caution: 'Caution',
  risk: 'High Risk',
  idle: 'Waiting',
}

export const STATUS_CLASS: Record<MetricStatus, string> = {
  optimal: 'bg-signal/15 text-signal border-signal/40',
  caution: 'bg-caution/15 text-caution border-caution/40',
  risk: 'bg-risk/15 text-risk border-risk/40',
  idle: 'bg-line/40 text-mute border-line',
}
