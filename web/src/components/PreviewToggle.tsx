interface PreviewToggleProps {
  enabled: boolean
  onChange: (enabled: boolean) => void
  disabled?: boolean
}

/** Toggle JPEG camera preview over WebSocket (off = skeleton-only, no video bytes). */
export function PreviewToggle({ enabled, onChange, disabled }: PreviewToggleProps) {
  return (
    <button
      type="button"
      disabled={disabled}
      title={
        enabled
          ? 'Camera video is streaming to this dashboard — click to disable'
          : 'Camera video is not sent — skeleton overlay only'
      }
      onClick={() => onChange(!enabled)}
      className={
        enabled
          ? 'rounded-md border border-line px-3 py-2 font-mono text-[11px] uppercase tracking-wider text-mute hover:text-ink disabled:opacity-40'
          : 'rounded-md border border-caution/50 bg-caution/10 px-3 py-2 font-mono text-[11px] uppercase tracking-wider text-caution disabled:opacity-40'
      }
      aria-pressed={!enabled}
    >
      {enabled ? 'Video on' : 'Video off'}
    </button>
  )
}
