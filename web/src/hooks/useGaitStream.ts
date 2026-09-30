import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  ConnectionStatus,
  GaitMetrics,
  MetricHistoryPoint,
  MetricsMessage,
  RecordedFrame,
} from '../types/gait'
import { metricHistoryFromGait } from '../components/MetricHistoryCharts'

const DEFAULT_URL = 'ws://127.0.0.1:8080'
const MAX_HISTORY = 300
const MAX_RECORD_SECS = 30

export interface GaitStreamOptions {
  url?: string
  /** When true, keep a rolling buffer for sparkline / report charts. */
  historyLimit?: number
  autoConnect?: boolean
}

export interface GaitStreamState {
  status: ConnectionStatus
  latest: GaitMetrics | null
  frameDataUrl: string | null
  frameWidth: number
  frameHeight: number
  facing: 'left' | 'right' | 'auto' | null
  cmPerPx: number | null
  history: GaitMetrics[]
  cadenceHistory: Array<{ t: number; spm: number }>
  metricHistory: MetricHistoryPoint[]
  recording: boolean
  recorded: RecordedFrame[]
  contactFrames: number[]
  error: string | null
  connect: () => void
  disconnect: () => void
  startRecording: () => void
  stopRecording: () => void
  clearRecording: () => void
}

export function useGaitStream(options: GaitStreamOptions = {}): GaitStreamState {
  const url = options.url ?? DEFAULT_URL
  const historyLimit = options.historyLimit ?? MAX_HISTORY
  const autoConnect = options.autoConnect ?? true

  const [status, setStatus] = useState<ConnectionStatus>('closed')
  const [latest, setLatest] = useState<GaitMetrics | null>(null)
  const [frameDataUrl, setFrameDataUrl] = useState<string | null>(null)
  const [frameWidth, setFrameWidth] = useState(1280)
  const [frameHeight, setFrameHeight] = useState(720)
  const [facing, setFacing] = useState<'left' | 'right' | 'auto' | null>(null)
  const [cmPerPx, setCmPerPx] = useState<number | null>(null)
  const [history, setHistory] = useState<GaitMetrics[]>([])
  const [cadenceHistory, setCadenceHistory] = useState<Array<{ t: number; spm: number }>>([])
  const [metricHistory, setMetricHistory] = useState<MetricHistoryPoint[]>([])
  const [recording, setRecording] = useState(false)
  const [recorded, setRecorded] = useState<RecordedFrame[]>([])
  const [contactFrames, setContactFrames] = useState<number[]>([])
  const [error, setError] = useState<string | null>(null)

  const wsRef = useRef<WebSocket | null>(null)
  const recordingRef = useRef(false)
  const recordStartRef = useRef<number | null>(null)
  const rafPending = useRef(false)
  const pendingMsg = useRef<MetricsMessage | null>(null)

  const applyMessage = useCallback(
    (msg: MetricsMessage) => {
      const m = msg.metrics
      setLatest(m)
      if (msg.frame_width && msg.frame_height) {
        setFrameWidth(msg.frame_width)
        setFrameHeight(msg.frame_height)
      }
      if (msg.facing) setFacing(msg.facing)
      if (msg.cm_per_px != null) setCmPerPx(msg.cm_per_px)
      if (msg.frame) {
        const dataUrl = msg.frame.startsWith('data:')
          ? msg.frame
          : `data:image/jpeg;base64,${msg.frame}`
        setFrameDataUrl(dataUrl)
      }

      setHistory((prev) => {
        const next = [...prev, m]
        return next.length > historyLimit ? next.slice(next.length - historyLimit) : next
      })

      if (m.cadence_spm != null) {
        setCadenceHistory((prev) => {
          const next = [...prev, { t: m.timestamp_secs, spm: m.cadence_spm! }]
          return next.length > historyLimit ? next.slice(next.length - historyLimit) : next
        })
      }

      setMetricHistory((prev) => {
        const next = [...prev, metricHistoryFromGait(m)]
        return next.length > historyLimit ? next.slice(next.length - historyLimit) : next
      })

      if (recordingRef.current) {
        const start = recordStartRef.current ?? m.timestamp_secs
        recordStartRef.current = start
        if (m.timestamp_secs - start >= MAX_RECORD_SECS) {
          recordingRef.current = false
          setRecording(false)
        } else {
          setRecorded((prev) => {
            const frame: RecordedFrame = {
              metrics: m,
              frameDataUrl: msg.frame
                ? msg.frame.startsWith('data:')
                  ? msg.frame
                  : `data:image/jpeg;base64,${msg.frame}`
                : undefined,
            }
            const next = [...prev, frame]
            if (m.left_foot_strike || m.right_foot_strike) {
              setContactFrames((c) => [...c, next.length - 1])
            }
            return next
          })
        }
      }
    },
    [historyLimit],
  )

  /** Coalesce inbound WS messages to one React update per animation frame (~60 Hz). */
  const scheduleApply = useCallback(
    (msg: MetricsMessage) => {
      pendingMsg.current = msg
      if (rafPending.current) return
      rafPending.current = true
      requestAnimationFrame(() => {
        rafPending.current = false
        if (pendingMsg.current) {
          applyMessage(pendingMsg.current)
          pendingMsg.current = null
        }
      })
    },
    [applyMessage],
  )

  const disconnect = useCallback(() => {
    wsRef.current?.close()
    wsRef.current = null
    setStatus('closed')
  }, [])

  const connect = useCallback(() => {
    disconnect()
    setError(null)
    setStatus('connecting')

    try {
      const ws = new WebSocket(url)
      wsRef.current = ws

      ws.onopen = () => setStatus('open')
      ws.onerror = () => {
        setStatus('error')
        setError(`Failed to connect to ${url}`)
      }
      ws.onclose = () => {
        setStatus('closed')
        wsRef.current = null
      }
      ws.onmessage = (ev) => {
        try {
          const raw = typeof ev.data === 'string' ? ev.data : null
          if (!raw) return
          const msg = JSON.parse(raw) as MetricsMessage
          if (msg.type !== 'gait_metrics' || !msg.metrics) return
          scheduleApply(msg)
        } catch {
          // ignore malformed frames
        }
      }
    } catch (e) {
      setStatus('error')
      setError(e instanceof Error ? e.message : 'WebSocket error')
    }
  }, [disconnect, scheduleApply, url])

  useEffect(() => {
    if (autoConnect) connect()
    return () => disconnect()
  }, [autoConnect, connect, disconnect])

  const startRecording = useCallback(() => {
    recordingRef.current = true
    recordStartRef.current = null
    setRecorded([])
    setContactFrames([])
    setRecording(true)
  }, [])

  const stopRecording = useCallback(() => {
    recordingRef.current = false
    setRecording(false)
  }, [])

  const clearRecording = useCallback(() => {
    recordingRef.current = false
    recordStartRef.current = null
    setRecording(false)
    setRecorded([])
    setContactFrames([])
  }, [])

  return {
    status,
    latest,
    frameDataUrl,
    frameWidth,
    frameHeight,
    facing,
    cmPerPx,
    history,
    cadenceHistory,
    metricHistory,
    recording,
    recorded,
    contactFrames,
    error,
    connect,
    disconnect,
    startRecording,
    stopRecording,
    clearRecording,
  }
}
