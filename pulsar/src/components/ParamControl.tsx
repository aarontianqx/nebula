import type { ParamSpec, ParamValue } from "../types/tool";

interface Props {
  spec: ParamSpec;
  value: ParamValue;
  onChange: (value: ParamValue) => void;
}

export function ParamControl({ spec, value, onChange }: Props) {
  if (spec.kind === "bool") {
    return (
      <label className="flex cursor-pointer items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={Boolean(value)}
          onChange={(e) => onChange(e.target.checked)}
          className="h-4 w-4 accent-[var(--color-accent)]"
        />
        <span>{spec.label}</span>
      </label>
    );
  }

  if (spec.kind === "enum") {
    return (
      <label className="flex items-center gap-2 text-sm">
        <span className="text-[var(--color-text-muted)]">{spec.label}</span>
        <select
          value={String(value)}
          onChange={(e) => onChange(e.target.value)}
          className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-sm outline-none"
        >
          {spec.options.map((opt) => (
            <option key={opt} value={opt}>
              {opt}
            </option>
          ))}
        </select>
      </label>
    );
  }

  if (spec.kind === "int") {
    return (
      <label className="flex items-center gap-2 text-sm">
        <span className="text-[var(--color-text-muted)]">{spec.label}</span>
        <input
          type="number"
          value={Number(value)}
          onChange={(e) => onChange(Number(e.target.value))}
          className="w-20 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-sm outline-none"
        />
      </label>
    );
  }

  return (
    <label className="flex items-center gap-2 text-sm">
      <span className="text-[var(--color-text-muted)]">{spec.label}</span>
      <input
        type="text"
        value={String(value)}
        onChange={(e) => onChange(e.target.value)}
        className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-sm outline-none"
      />
    </label>
  );
}
