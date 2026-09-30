import { useEffect, useMemo, useState } from 'react'
import type { GaitMetrics, RecordedFrame } from '../types/gait'
import { VideoSkeletonPanel } from './VideoSkeletonPanel'

const SPEEDS = [0.1, 0.25, 0.5, 1] as const

interface RecordingScrubberProps {
  recording: boolean
  recorded: RecordedFrame[]
  contactFrames: number[]
  onStart: () => void
  onStop: () => void
  onClear: () => void
}

export function RecordingScrubber({
  recording,
  recorded,
  contactFrames,
  onStart,
  onStop,
  onClear,
}: RecordingScrubberProps) {
  const [index, setIndex] = useState(0)
  const [playing, setPlaying] = useState(false)
  const [speed, setSpeed] = useState<(typeof SPEEDS)[number]>(0.25)

  useEffect(() => {
    if (!recording && recorded.length) {
      setIndex(0)
      setPlaying(false)
    }
  }, [recording, recorded.length])

  useEffect(() => {
    if (!playing || recorded.length < 2) return
    const baseDt =
      (recorded[Math.min(index + 1, recorded.length - 1)].metrics.timestamp_secs -
        recorded[index].metrics.timestamp_secs) *
      1000
    const delay = Math.max(16, (baseDt || 16) / speed)
    const id = window.setTimeout(() => {
      setIndex((i) => {
        if (i >= recorded.length - 1) {
          setPlaying(false)
          return i
        }
        return i + 1
      })
    }, delay)
    return () => clearTimeout(id)
  }, [playing, index, recorded, speed])

  const current: GaitMetrics | null = recorded[index]?.metrics ?? null
  const frameDataUrl = recorded[index]?.frameDataUrl ?? null

  const contactSet = useMemo(() => new Set(contactFrames), [contactFrames])

  const jumpContact = (dir: -1 | 1) => {
    const sorted = [...contactFrames].sort((a, b) => a - b)
    if (!sorted.length) return
    if (dir > 0) {
      const next = sorted.find((i) => i > index)
      setIndex(next ?? sorted[sorted.length - 1])
    } else {
      const prev = [...sorted].reverse().find((i) => i < index)
      setIndex(prev ?? sorted[0])
    }
    setPlaying(false)
  }

  return (
    <section className="rounded-lg border border-line bg-panel/70 p-4">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="font-display text-lg font-semibold tracking-tight">Run recording</h2>
          <p className="text-sm text-mute">Capture 10–30s, scrub in slow motion, snap to foot contact.</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {!recording ? (
            <button
              type="button"
              onClick={onStart}
              className="rounded-md bg-risk px-3 py-2 font-mono text-xs font-semibold uppercase tracking-wider text-canvas hover:brightness-110"
            >
              ● Record Run
            </button>
          ) : (
            <button
              type="button"
              onClick={onStop}
              className="animate-pulse rounded-md border border-risk bg-risk/20 px-3 py-2 font-mono text-xs font-semibold uppercase tracking-wider text-risk"
            >
              ■ Stop ({recorded.length} frames)
            </button>
          )}
          <button
            type="button"
            onClick={onClear}
            disabled={!recorded.length && !recording}
            className="rounded-md border border-line px-3 py-2 font-mono text-xs uppercase tracking-wider text-mute hover:border-mute disabled:opacity-40"
          >
            Clear
          </button>
        </div>
      </div>

      {recorded.length > 0 && !recording ? (
        <div className="grid gap-4 lg:grid-cols-[1.2fr_1fr]">
          <VideoSkeletonPanel metrics={current} frameDataUrl={frameDataUrl} live={false} />

          <div className="flex flex-col gap-3">
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                className="rounded border border-line px-2 py-1.5 font-mono text-xs text-ink hover:border-neon"
                onClick={() => {
                  setPlaying(false)
                  setIndex((i) => Math.max(0, i - 1))
                }}
              >
                ‹ Step Back
              </button>
              <button
                type="button"
                className="rounded border border-line px-2 py-1.5 font-mono text-xs text-ink hover:border-neon"
                onClick={() => setPlaying((p) => !p)}
              >
                {playing ? 'Pause' : 'Play'}
              </button>
              <button
                type="button"
                className="rounded border border-line px-2 py-1.5 font-mono text-xs text-ink hover:border-neon"
                onClick={() => {
                  setPlaying(false)
                  setIndex((i) => Math.min(recorded.length - 1, i + 1))
                }}
              >
                Step Forward ›
              </button>
            </div>

            <div className="flex flex-wrap gap-1.5">
              {SPEEDS.map((s) => (
                <button
                  key={s}
                  type="button"
                  onClick={() => setSpeed(s)}
                  className={`rounded px-2 py-1 font-mono text-[11px] ${
                    speed === s
                      ? 'bg-neon/20 text-neon border border-neon/50'
                      : 'border border-line text-mute hover:text-ink'
                  }`}
                >
                  {s}x
                </button>
              ))}
            </div>

            <label className="block">
              <span className="mb-1 block font-mono text-[10px] uppercase tracking-wider text-mute">
                Scrub · frame {index + 1}/{recorded.length}
                {contactSet.has(index) ? ' · CONTACT' : ''}
              </span>
              <input
                type="range"
                min={0}
                max={Math.max(0, recorded.length - 1)}
                value={index}
                onChange={(e) => {
                  setPlaying(false)
                  setIndex(Number(e.target.value))
                }}
                className="w-full accent-neon"
              />
            </label>

            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => jumpContact(-1)}
                disabled={!contactFrames.length}
                className="rounded-md border border-signal/40 bg-signal/10 px-3 py-2 font-mono text-[11px] uppercase tracking-wider text-signal disabled:opacity-40"
              >
                ‹ Prev foot contact
              </button>
              <button
                type="button"
                onClick={() => jumpContact(1)}
                disabled={!contactFrames.length}
                className="rounded-md border border-signal/40 bg-signal/10 px-3 py-2 font-mono text-[11px] uppercase tracking-wider text-signal disabled:opacity-40"
              >
                Next foot contact ›
              </button>
            </div>

            <p className="font-mono text-[11px] text-mute">
              {contactFrames.length} contact frames tagged for review
            </p>
          </div>
        </div>
      ) : (
        <p className="font-mono text-sm text-mute">
          {recording
            ? 'Recording live stream… stop within 30 seconds to scrub.'
            : 'No clip yet. Hit Record Run while the live stream is open.'}
        </p>
      )}
    </section>
  )
}
