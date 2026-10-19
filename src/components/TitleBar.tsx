import { ArrowLeft, History, Maximize2, Minimize2, Minus, Pin, Settings, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { commands } from "../ipc/commands";
import { sourcesText } from "../lib/format";
import { backOf, PLACES, placeOf } from "../lib/nav";
import { sourceMenu } from "../lib/sourceMenu";
import { isBusy, useRecording } from "../store/recording";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";
import { Logo } from "@yusai/kamon-react";
import { IconButton } from "./ui";

/** The name, with the app's kamon as a window's icon sits beside its title. */
export function Brand() {
  return (
    <span data-tauri-drag-region className="pointer-events-none flex shrink-0 items-center gap-2 font-medium">
      <Logo app="chidori" size={18} />
      Chidori
    </span>
  );
}

/**
 * What is recording, in place of the name while a recording runs: an accent mark (grey while paused
 * or finishing), the state and the source. Away from the main view it leads back to the recording;
 * in the compact window's main view, which has no room for the recording's header, it opens a
 * native menu to switch the source (FR-17).
 */
function LiveTag({ compact }: { compact: boolean }) {
  const { t } = useTranslation();
  const state = useRecording((s) => s.state);
  const recorded = useRecording((s) => s.sources);
  const sources = sourcesText(recorded, t);
  const view = useUi((s) => s.view);
  const go = useUi((s) => s.go);
  const quiet = state === "paused" || state === "finishing";
  const label = state === "paused" ? t("paused") : state === "finishing" ? t("finishing") : t("recording");
  const body = (
    <>
      <span className={`size-2 shrink-0 rounded-[2px] transition-colors duration-150 ${quiet ? "bg-line-strong" : "bg-rec"}`} aria-hidden />
      <span className="shrink-0 text-[12px] font-medium">{label}</span>
      {sources && <span className="min-w-0 truncate text-[12px] text-muted">{sources}</span>}
    </>
  );
  if (view === "main" && compact && state !== "finishing") {
    return (
      <button type="button" onClick={() => void sourceMenu()} aria-haspopup="menu" title={t("switchSource")} className="-mx-1 flex min-w-0 items-center gap-2 rounded-md px-1 py-0.5 transition-colors hover:bg-surface-2">
        {body}
      </button>
    );
  }
  if (view === "main") {
    // The title bar may be too narrow for the source (English, a long app name): it shows on hover.
    return (
      <span data-tauri-drag-region className="flex min-w-0 items-center gap-2" aria-live="off" title={sources || undefined}>
        {body}
      </span>
    );
  }
  return (
    <button type="button" onClick={() => go("main")} title={t("backToLive")} className="-mx-1 flex min-w-0 items-center gap-2 rounded-md px-1 py-0.5 transition-colors hover:bg-surface-2">
      {body}
    </button>
  );
}

export function WindowButtons({ layoutToggle = true }: { layoutToggle?: boolean }) {
  const { t } = useTranslation();
  const view = useUi((s) => s.view);
  const go = useUi((s) => s.go);
  const layout = useSettings((s) => s.settings?.window.layout ?? "expanded");
  const onTop = useSettings((s) => s.settings?.window.alwaysOnTop ?? false);
  const update = useSettings((s) => s.update);
  // Only 録音 has a compact form; everywhere else the window is expanded, and shrinking it goes to
  // 録音. App resizes the window when the layout it shows changes.
  const compact = view === "main" && layout === "compact";
  const toggleLayout = async () => {
    await update({ window: { layout: compact ? "expanded" : "compact" } });
    if (view !== "main") go("main");
  };
  return (
    <>
      {/* Always on top: off until pinned. */}
      <IconButton size="sm" label={t("alwaysOnTop")} aria-pressed={onTop} onClick={() => void update({ window: { alwaysOnTop: !onTop } })} className={onTop ? "!text-fg" : ""}>
        <Pin size={14} fill={onTop ? "currentColor" : "none"} className={onTop ? "" : "rotate-45"} />
      </IconButton>
      {layoutToggle && (
        <IconButton size="sm" label={compact ? t("expand") : t("shrink")} onClick={toggleLayout}>
          {compact ? <Maximize2 size={14} /> : <Minimize2 size={14} />}
        </IconButton>
      )}
      <IconButton size="sm" label={t("minimize")} onClick={() => commands.windowAction("minimize")}>
        <Minus size={15} />
      </IconButton>
      <IconButton size="sm" label={t("close")} onClick={() => commands.windowAction("close")} className="hover:!bg-danger hover:!text-surface">
        <X size={15} />
      </IconButton>
    </>
  );
}

/** 録音 · 履歴 · 設定 as words; the current place is in the accent's tint, and 録音 carries a mark while recording. */
function Places() {
  const { t } = useTranslation();
  const view = useUi((s) => s.view);
  const go = useUi((s) => s.go);
  const busy = isBusy(useRecording((s) => s.state));
  const current = placeOf(view);
  const label = { main: t("tabRecord"), history: t("history"), settings: t("settings") };
  return (
    <nav aria-label={t("navLabel")} className="flex shrink-0 items-center">
      {PLACES.map((place) => {
        const here = place === current;
        return (
          <button
            key={place}
            type="button"
            aria-current={here ? "page" : undefined}
            onClick={() => go(place)}
            title={place === "main" && busy && !here ? t("backToLive") : undefined}
            className={`relative flex items-center gap-1.5 rounded-sm px-[7px] py-[3px] text-[13px] font-medium whitespace-nowrap transition-colors ${here ? "bg-accent-soft text-accent-text" : "text-muted hover:text-fg"}`}
          >
            {place === "main" && busy && <span className="size-1.5 rounded-[2px] bg-rec" aria-hidden />}
            {label[place]}
          </button>
        );
      })}
    </nav>
  );
}

/**
 * The title bar over every screen, kept while the views under it change: back (a past session, a
 * section of 設定), the name or what is recording, the three places as words (expanded) or as icons
 * (compact), and the window buttons. Draggable. No rule under it: the views start right below, and
 * only the setup wizard draws one, under its title.
 */
export function TitleBar({
  compact = false,
  setup = false,
}: {
  compact?: boolean;
  /** The setup wizard: no places, no layout toggle. */
  setup?: boolean;
}) {
  const { t } = useTranslation();
  const busy = isBusy(useRecording((s) => s.state));
  const go = useUi((s) => s.go);
  const back = useUi((s) => (setup ? undefined : backOf(s.view, s.settingsTab)));
  const places = !setup;
  return (
    <header data-tauri-drag-region className="relative z-20 flex h-10 shrink-0 items-center gap-1 pr-1.5 pl-3">
      {back && (
        <IconButton size="sm" label={t("back")} onClick={() => go(back)} className="-ml-1.5">
          <ArrowLeft size={16} />
        </IconButton>
      )}
      <span data-tauri-drag-region className="flex min-w-0 flex-1 items-center">
        {busy ? <LiveTag compact={compact} /> : <Brand />}
      </span>
      {places && !compact && <Places />}
      {places && compact && (
        <>
          <IconButton size="sm" label={t("history")} onClick={() => go("history")}>
            <History size={15} />
          </IconButton>
          <IconButton size="sm" label={t("settings")} onClick={() => go("settings")}>
            <Settings size={15} />
          </IconButton>
        </>
      )}
      {places && <span className="mx-1 h-4 w-px shrink-0 bg-line" aria-hidden />}
      <WindowButtons layoutToggle={!setup} />
    </header>
  );
}
