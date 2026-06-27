import type { ParamSpec, ParamValue } from "../types/tool";

interface Props {
  spec: ParamSpec;
  value: ParamValue;
  onChange: (value: ParamValue) => void;
  /** 堆叠布局（表单纵向排列，标签在上）。默认 inline（标签在左）。 */
  stacked?: boolean;
}

export function ParamControl({ spec, value, onChange, stacked = false }: Props) {
  if (spec.kind === "bool") {
    return (
      <label className="flex cursor-pointer select-none items-center gap-2 text-sm">
        <Switch checked={Boolean(value)} onChange={(c) => onChange(c)} />
        <span className="text-[var(--color-text)]">{spec.label}</span>
      </label>
    );
  }

  if (spec.kind === "enum") {
    // 选项不多时用 segmented control，比下拉更直观可扫读
    const useSegmented = spec.options.length > 0 && spec.options.length <= 4;
    return (
      <Field label={spec.label} stacked={stacked}>
        {useSegmented ? (
          <Segmented
            options={spec.options}
            value={String(value)}
            onChange={(v) => onChange(v)}
          />
        ) : (
          <select
            value={String(value)}
            onChange={(e) => onChange(e.target.value)}
            className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
          >
            {spec.options.map((opt) => (
              <option key={opt} value={opt}>
                {opt}
              </option>
            ))}
          </select>
        )}
      </Field>
    );
  }

  if (spec.kind === "int") {
    return (
      <Field label={spec.label} stacked={stacked}>
        <input
          type="number"
          value={Number(value)}
          onChange={(e) => onChange(Number(e.target.value))}
          className="w-24 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
        />
      </Field>
    );
  }

  return (
    <Field label={spec.label} stacked={stacked}>
      <input
        type="text"
        value={String(value)}
        onChange={(e) => onChange(e.target.value)}
        className={[
          "font-mono rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]",
          stacked ? "w-full" : "w-56",
        ].join(" ")}
      />
    </Field>
  );
}

function Field({
  label,
  stacked,
  children,
}: {
  label: string;
  stacked: boolean;
  children: React.ReactNode;
}) {
  if (stacked) {
    return (
      <label className="flex flex-col gap-1.5">
        <span className="text-xs font-medium text-[var(--color-text-muted)]">{label}</span>
        {children}
      </label>
    );
  }
  return (
    <label className="flex items-center gap-2 text-sm">
      <span className="text-[var(--color-text-muted)]">{label}</span>
      {children}
    </label>
  );
}

/** 开关（替代原生 checkbox，视觉更现代）。 */
function Switch({ checked, onChange }: { checked: boolean; onChange: (c: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={[
        "relative h-5 w-9 shrink-0 rounded-full transition-colors",
        checked ? "bg-[var(--color-accent)]" : "bg-[var(--color-border-strong)]",
      ].join(" ")}
    >
      {/* 滑块：显式 left + 垂直居中锚定，靠 translate-x 平移，避免无锚点导致的错位 */}
      <span
        className={[
          "absolute left-0.5 top-1/2 h-4 w-4 -translate-y-1/2 rounded-full bg-white shadow-sm transition-transform",
          checked ? "translate-x-4" : "translate-x-0",
        ].join(" ")}
      />
    </button>
  );
}

/** 分段选择器（少量枚举值）。 */
function Segmented({
  options,
  value,
  onChange,
}: {
  options: string[];
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="inline-flex rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] p-0.5">
      {options.map((opt) => (
        <button
          key={opt}
          type="button"
          onClick={() => onChange(opt)}
          className={[
            "rounded px-2.5 py-1 text-xs font-medium transition-colors",
            value === opt
              ? "bg-[var(--color-accent)] text-white"
              : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]",
          ].join(" ")}
        >
          {opt}
        </button>
      ))}
    </div>
  );
}
