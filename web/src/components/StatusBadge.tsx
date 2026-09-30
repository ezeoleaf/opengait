import type { MetricStatus } from '../types/gait'
import { STATUS_CLASS, STATUS_LABEL } from '../lib/thresholds'

interface StatusBadgeProps {
  status: MetricStatus
}

export function StatusBadge({ status }: StatusBadgeProps) {
  return (
    <span
      className={`inline-flex items-center rounded border px-2 py-0.5 font-mono text-[10px] font-medium uppercase tracking-wider ${STATUS_CLASS[status]}`}
    >
      {STATUS_LABEL[status]}
    </span>
  )
}
