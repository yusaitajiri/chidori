// Small shared building blocks.

import { LoaderCircle } from "lucide-react";
import { useEffect, type ButtonHTMLAttributes, type ReactNode } from "react";

type Variant = "primary" | "danger" | "secondary" | "ghost" | "soft";
type Size = "sm" | "md" | "lg";

// primary is ui.css's `.button-primary`, the one accent-filled action of a view, with the kamon's
// inner line; a destructive confirmation takes the same shape in the brand's brick.
const variants: Record<Variant, string> = {
  primary: "button button-primary",
  danger: "button button-primary button-danger",
  secondary: "border border-line-strong bg-surface text-fg hover:bg-surface-2",
  ghost: "text-fg hover:bg-surface-2",
  soft: "bg-surface-2 text-fg hover:bg-surface-3",
};

const sizes: Record<Size, string> = {
  sm: "h-7 gap-1.5 px-2.5 text-[12px]",
  md: "h-8 gap-1.5 px-3 text-[13px]",
  lg: "h-10 gap-2 px-4 text-[14px]",
};

export function Button({
  variant = "secondary",
  size = "md",
  className = "",
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: Size }) {
  return (
    <button
      type="button"
      className={`inline-flex shrink-0 items-center justify-center rounded-md font-medium whitespace-nowrap transition-[background-color,border-color,color] duration-150 ease-out-soft disabled:pointer-events-none disabled:opacity-45 ${variants[variant]} ${sizes[size]} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}

export function IconButton({
  label,
  className = "",
  size = "md",
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string; size?: "sm" | "md" }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={`inline-flex shrink-0 items-center justify-center rounded-md text-muted transition-[background-color,color] duration-150 hover:bg-surface-2 hover:text-fg disabled:pointer-events-none disabled:opacity-40 ${size === "sm" ? "h-7 w-7" : "h-8 w-8"} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}

export function Spinner({ size = 14, className = "" }: { size?: number; className?: string }) {
  return <LoaderCircle size={size} className={`animate-spin ${className}`} aria-hidden />;
}

/** A switch: a real checkbox underneath, so it stays keyboard and screen-reader friendly. */
export function Switch({ checked, onChange, disabled, label }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; label?: string }) {
  return (
    <span className="relative inline-flex shrink-0">
      <input
        type="checkbox"
        role="switch"
        aria-label={label}
        className="peer absolute inset-0 z-10 cursor-pointer opacity-0 disabled:cursor-not-allowed"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span aria-hidden className="pointer-events-none absolute inset-0 rounded-sm peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-accent" />
      <SwitchMark on={checked} className="peer-disabled:opacity-50" />
    </span>
  );
}

/** How a switch looks, for a control that is a switch as a whole (a row that toggles). */
export function SwitchMark({ on, className = "" }: { on: boolean; className?: string }) {
  return (
    <span aria-hidden className={`relative inline-block h-5 w-9 shrink-0 rounded-sm transition-colors duration-150 ${on ? "bg-accent" : "bg-line-strong"} ${className}`}>
      <span className={`absolute top-0.5 left-0.5 size-4 rounded-[4px] bg-surface transition-transform duration-150 ease-out-soft ${on ? "translate-x-4" : ""}`} />
    </span>
  );
}

export function Toggle({ checked, onChange, label, disabled, help }: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean; help?: string }) {
  return (
    <label className={`flex items-center gap-3 py-2 ${disabled ? "opacity-50" : "cursor-pointer"}`}>
      <span className="min-w-0 flex-1">
        <span className="block text-[13px]">{label}</span>
        {help && <span className="mt-0.5 block text-xs text-muted">{help}</span>}
      </span>
      <Switch checked={checked} onChange={onChange} disabled={disabled} label={label} />
    </label>
  );
}

export function Field({ label, children, help }: { label: string; children: ReactNode; help?: string }) {
  return (
    <div className="py-2">
      <div className="mb-1.5 text-xs font-medium text-muted">{label}</div>
      {children}
      {help && <div className="mt-1.5 text-xs text-muted">{help}</div>}
    </div>
  );
}

const chevron = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23888' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E")`;

export const inputClass =
  "h-8 w-full rounded-md border border-line bg-surface-2 px-2.5 text-[13px] text-fg transition-colors hover:border-line-strong focus:border-accent focus:outline-none focus-visible:outline-none disabled:opacity-50";

export function Select<T extends string>({
  value,
  onChange,
  options,
  ariaLabel,
  disabled,
  className = "",
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
  ariaLabel: string;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <select
      aria-label={ariaLabel}
      className={`${inputClass} appearance-none bg-no-repeat pr-8 ${className}`}
      style={{ backgroundImage: chevron, backgroundPosition: "right 10px center" }}
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value as T)}
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function Progress({ value, max, className = "" }: { value: number; max: number; className?: string }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div className={`h-1.5 w-full overflow-hidden rounded-full bg-meter ${className}`} role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
      <div className="h-full rounded-full bg-accent transition-[width] duration-300 ease-out-soft" style={{ width: `${pct}%` }} />
    </div>
  );
}

/** A keyboard shortcut, e.g. `Ctrl+Alt+R`. */
export function Kbd({ keys }: { keys: string }) {
  return (
    <span className="inline-flex items-center gap-0.5">
      {keys.split("+").map((k) => (
        <kbd key={k} className="num rounded-sm border border-line bg-surface px-1.5 py-px text-[11px] text-muted">
          {k}
        </kbd>
      ))}
    </span>
  );
}

export function Dialog({ title, children, actions, onClose }: { title: string; children?: ReactNode; actions: ReactNode; onClose?: () => void }) {
  useEffect(() => {
    if (!onClose) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  // A sheet over the dimmed window, in the kamon's frame (the one container being worked in while
  // it is open): centred title, a hairline before the text, and the actions as full-width buttons
  // with the main one on top.
  return (
    <div className="fixed inset-0 z-50 flex animate-fade-in items-center justify-center bg-[color-mix(in_srgb,var(--ink)_30%,transparent)] p-5" role="dialog" aria-modal="true" aria-label={title}>
      <div className="frame w-full max-w-sm animate-pop rounded-lg bg-surface px-5 pt-5 pb-4 shadow-float">
        <h2 className="text-center text-[16px] leading-snug font-medium text-balance">{title}</h2>
        {children ? (
          <>
            <hr className="my-3 border-line" />
            <div className="mb-4 text-[13px] text-fg">{children}</div>
          </>
        ) : (
          <div className="h-4" />
        )}
        <div className="flex flex-col-reverse gap-1.5 [&>button]:h-10 [&>button]:w-full">{actions}</div>
      </div>
    </div>
  );
}

/** A titled group of settings on a card. */
export function Section({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-line bg-surface px-3.5 py-1.5">
      {title && <h3 className="pt-2 pb-0.5 text-[12px] font-medium text-muted">{title}</h3>}
      <div className="divide-y divide-line">{children}</div>
    </section>
  );
}
