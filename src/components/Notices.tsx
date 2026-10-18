import { CircleAlert, CircleCheckBig, Info, TriangleAlert, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { runNoticeAction } from "../lib/actions";
import { useUi } from "../store/ui";

/**
 * Banners at the top with one action button (section 5): errors on the brand's brick tint and
 * warnings on its ochre, each with its icon and words, so none relies on colour alone.
 */
export function Banners() {
  const { t } = useTranslation();
  const { banners, dismissBanner } = useUi();
  if (banners.length === 0) return null;
  return (
    <div className="shrink-0 space-y-1.5 px-3 pb-1.5" role="alert">
      {banners.map((b) => {
        const Icon = b.level === "error" ? CircleAlert : b.level === "warn" ? TriangleAlert : Info;
        // The brand's brick and ochre on their tints, always with words: the accent is near both.
        const tone = b.level === "error" ? "bg-danger-tint" : b.level === "warn" ? "bg-warning-tint" : "bg-surface-2";
        const iconTone = b.level === "error" ? "text-danger" : b.level === "warn" ? "text-warning" : "text-muted";
        return (
          <div key={b.id} className={`flex animate-slide-down items-center gap-2.5 rounded-lg px-3 py-2 text-[12px] text-fg ${tone}`}>
            <Icon size={15} className={`shrink-0 ${iconTone}`} />
            <span className="min-w-0 flex-1 leading-snug">{t(b.message, b.params ?? {})}</span>
            {b.action && (
              <button
                type="button"
                className="shrink-0 rounded-md border border-line-strong bg-surface px-2.5 py-1 text-[12px] font-medium text-fg transition-colors hover:bg-surface-2"
                onClick={() => {
                  void runNoticeAction(b.action!.command);
                  dismissBanner(b.id);
                }}
              >
                {t(b.action.label)}
              </button>
            )}
            <button type="button" aria-label={t("close")} className="shrink-0 rounded-sm p-0.5 opacity-70 transition-opacity hover:opacity-100" onClick={() => dismissBanner(b.id)}>
              <X size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}

/** In-app toasts, shown for 2 s under the title bar: a small sheet; its icon tells the level. */
export function Toasts() {
  const toasts = useUi((s) => s.toasts);
  if (toasts.length === 0) return null;
  return (
    <div className="pointer-events-none fixed inset-x-0 top-11 z-40 flex flex-col items-center gap-1.5 px-4" role="status" aria-live="polite">
      {toasts.map((toast) => {
        const Icon = toast.level === "error" ? CircleAlert : toast.level === "warn" ? TriangleAlert : CircleCheckBig;
        return (
          <div key={toast.id} className="flex max-w-full animate-pop items-center gap-2 rounded-lg border border-line bg-surface px-3.5 py-2 text-[12px] font-medium text-fg shadow-float">
            <Icon size={14} className="shrink-0" />
            <span className="truncate">{toast.text}</span>
          </div>
        );
      })}
    </div>
  );
}
