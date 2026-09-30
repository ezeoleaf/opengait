import type { CameraView } from '../types/gait'

const VIEWS: Array<{ id: CameraView; label: string; hint: string }> = [
  { id: 'side', label: 'Side', hint: 'Sagittal' },
  { id: 'front', label: 'Front', hint: 'Anterior' },
  { id: 'back', label: 'Back', hint: 'Posterior' },
]

interface ViewSelectorProps {
  value: CameraView
  onChange: (view: CameraView) => void
  disabled?: boolean
}

export function ViewSelector({ value, onChange, disabled }: ViewSelectorProps) {
  return (
    <div
      className="inline-flex rounded-md border border-line bg-panel/80 p-0.5"
      role="group"
      aria-label="Camera view"
    >
      {VIEWS.map((v) => {
        const active = value === v.id
        return (
          <button
            key={v.id}
            type="button"
            disabled={disabled}
            title={v.hint}
            onClick={() => onChange(v.id)}
            className={
              active
                ? 'rounded px-3 py-1.5 font-mono text-[11px] uppercase tracking-wider text-ink bg-neon/20 border border-neon/40'
                : 'rounded px-3 py-1.5 font-mono text-[11px] uppercase tracking-wider text-mute hover:text-ink border border-transparent disabled:opacity-40'
            }
          >
            {v.label}
          </button>
        )
      })}
    </div>
  )
}
