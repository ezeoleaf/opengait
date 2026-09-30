import { useGaitStream } from './hooks/useGaitStream'
import { VideoSkeletonPanel } from './components/VideoSkeletonPanel'
import { MetricCards } from './components/MetricCards'
import { MetricHistoryCharts } from './components/MetricHistoryCharts'
import { RecordingScrubber } from './components/RecordingScrubber'
import { ExportReport } from './components/ExportReport'
import { ViewSelector } from './components/ViewSelector'

export default function App() {
  const stream = useGaitStream({ url: 'ws://127.0.0.1:8080' })

  return (
    <div className="mx-auto flex min-h-full max-w-[1440px] flex-col gap-5 px-4 py-5 md:px-6">
      <header className="flex flex-wrap items-end justify-between gap-4 border-b border-line pb-4">
        <div>
          <p className="font-mono text-[11px] uppercase tracking-[0.28em] text-neon">Open Gait</p>
          <h1 className="font-display text-3xl font-extrabold tracking-tight text-ink md:text-4xl">
            Open Gait Dashboard
          </h1>
          <p className="mt-1 max-w-xl text-sm text-mute">
            Real-time running biomechanics — switch side, front, or back view for sagittal and
            frontal-plane form cues.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <ViewSelector
            value={stream.view}
            onChange={stream.setView}
            disabled={stream.status !== 'open'}
          />
          <StatusPill status={stream.status} error={stream.error} />
          {stream.status !== 'open' ? (
            <button
              type="button"
              onClick={stream.connect}
              className="rounded-md border border-neon/50 bg-neon/10 px-3 py-2 font-mono text-xs uppercase tracking-wider text-neon hover:bg-neon/20"
            >
              Reconnect
            </button>
          ) : (
            <button
              type="button"
              onClick={stream.disconnect}
              className="rounded-md border border-line px-3 py-2 font-mono text-xs uppercase tracking-wider text-mute hover:text-ink"
            >
              Disconnect
            </button>
          )}
        </div>
      </header>

      <div className="grid gap-4 lg:grid-cols-[minmax(0,1.7fr)_minmax(280px,0.7fr)]">
        <VideoSkeletonPanel
          metrics={stream.latest}
          frameDataUrl={stream.frameDataUrl}
          live={stream.status === 'open'}
          sourceWidth={stream.frameWidth}
          sourceHeight={stream.frameHeight}
        />
        <MetricCards
          metrics={stream.latest}
          cadenceHistory={stream.cadenceHistory}
          view={stream.view}
        />
      </div>

      <MetricHistoryCharts history={stream.metricHistory} view={stream.view} />

      <RecordingScrubber
        recording={stream.recording}
        recorded={stream.recorded}
        contactFrames={stream.contactFrames}
        onStart={stream.startRecording}
        onStop={stream.stopRecording}
        onClear={stream.clearRecording}
      />

      <ExportReport recorded={stream.recorded} contactFrames={stream.contactFrames} />

      <footer className="border-t border-line pt-3 pb-2 font-mono text-[11px] text-mute">
        open-gait · ws://127.0.0.1:8080 · view {stream.view} · facing {stream.facing ?? '—'}
        {stream.cmPerPx != null ? ` · ${stream.cmPerPx.toFixed(3)} cm/px` : ''}
        {stream.frameDataUrl ? ' · live preview' : ''}
        {stream.latest?.roi ? ' · ROI' : ''}
      </footer>
    </div>
  )
}

function StatusPill({
  status,
  error,
}: {
  status: string
  error: string | null
}) {
  const color =
    status === 'open'
      ? 'text-signal border-signal/40 bg-signal/10'
      : status === 'connecting'
        ? 'text-caution border-caution/40 bg-caution/10'
        : 'text-risk border-risk/40 bg-risk/10'

  return (
    <div className={`rounded-md border px-3 py-2 font-mono text-[11px] uppercase tracking-wider ${color}`}>
      {status}
      {error ? <span className="ml-2 normal-case tracking-normal opacity-80">· {error}</span> : null}
    </div>
  )
}
