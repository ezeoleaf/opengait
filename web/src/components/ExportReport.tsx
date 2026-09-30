import {
  buildSessionReport,
  downloadJsonReport,
  downloadPdfReport,
} from '../lib/report'
import type { RecordedFrame } from '../types/gait'

interface ExportReportProps {
  recorded: RecordedFrame[]
  contactFrames: number[]
}

export function ExportReport({ recorded, contactFrames }: ExportReportProps) {
  const disabled = recorded.length === 0

  const onGenerate = (format: 'pdf' | 'json') => {
    const report = buildSessionReport(recorded, contactFrames)
    if (format === 'pdf') downloadPdfReport(report)
    else downloadJsonReport(report)
  }

  return (
    <section className="rounded-lg border border-line bg-panel/70 p-4">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h2 className="font-display text-lg font-semibold tracking-tight">Debrief report</h2>
          <p className="max-w-xl text-sm text-mute">
            Compile peak metrics, average cadence, and ground-contact snapshots into a downloadable
            PDF or JSON report.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            disabled={disabled}
            onClick={() => onGenerate('pdf')}
            className="rounded-md bg-neon px-3 py-2 font-mono text-xs font-semibold uppercase tracking-wider text-canvas hover:brightness-110 disabled:opacity-40"
          >
            Generate Gait Report
          </button>
          <button
            type="button"
            disabled={disabled}
            onClick={() => onGenerate('json')}
            className="rounded-md border border-line px-3 py-2 font-mono text-xs uppercase tracking-wider text-ink hover:border-neon disabled:opacity-40"
          >
            JSON
          </button>
        </div>
      </div>
    </section>
  )
}
