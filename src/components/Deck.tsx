import { Camera, ChevronDown, Copy, Ellipsis, FileAudio, FileOutput, Pause, Play, Scissors, Square, Star } from "lucide-react";
import { useSessionAudio, type Player } from "../hooks/useSessionAudio";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { useElapsed } from "../hooks/useElapsed";
import { commands } from "../ipc/commands";
import type { SourceId, WindowInfo } from "../ipc/types";
import { addCut, copy, exportAs, markCurrentLine, newRecording, pauseOrResume, reportError, takeScreenshot, toggleRecording, transcribeFile } from "../lib/actions";
import { sessionClock, timerText } from "../lib/format";
import { animate, reducedMotion, stopAnimating, type Animated } from "../lib/frameLoop";
import { liveLine, mapLine, sessionShape, type Shape } from "../lib/line";
import { menuText, popupMenu, type MenuEntry } from "../lib/popupMenu";
import { copyEntries, sessionEntries } from "../lib/sessionMenu";
import { lineHistory } from "../store/line";
import { useRecording } from "../store/recording";
import { useSettings } from "../store/settings";
import { useSource } from "../store/source";
import { useTranscriptStore } from "../store/transcript";
import { useUi } from "../store/ui";
import { ModelNote } from "./ModelNote";
import { Progress, Spinner } from "./ui";

type DeckState = "start" | "recording" | "paused" | "finishing" | "review";

/**
 * How far below the expanded deck's top its line lies while recording and after. The transcript
 * runs on under the deck down to it, so only the tint under the line covers the text.
 */
export const LINE_Y = 30;

/**
 * In px: the deck's height while recording; the baseline then and, in the compact window, before
 * (the expanded window centres 開始 in the free space instead, with no line until it starts); how
 * far each side reaches; one step's width; where the pen starts (in line with the buttons under
 * it) and where it stops; where the compact 開始 sits on its empty line; the pen's radius; the
 * whole session's height. The line runs from edge to edge. Last, how many times a second the line
 * may be drawn: the compact window, which stays on screen through whole meetings, half as often.
 */
const SIZES = {
  expanded: { height: 124, base: LINE_Y, startBase: 30, centred: true, up: 20, down: 15, step: 1.6, startX: 14, endPad: 44, dotX: 14, pen: 4.5, mapAmp: 0.62, fps: 60 },
  compact: { height: 66, base: 12, startBase: 30, centred: false, up: 10, down: 8, step: 1.4, startX: 12, endPad: 34, dotX: 12, pen: 3.5, mapAmp: 0.75, fps: 30 },
} as const;
type Size = (typeof SIZES)[keyof typeof SIZES];

/**
 * Where the expanded 開始 rests, as a share of the deck's height above the model's line at the
 * bottom (`START_FOOT` px): a little above the middle, at the optical centre, because a shape at
 * the exact middle looks low. The CSS for 開始 and the hint under it uses the same numbers
 * (`--kk-rest`).
 */
const START_FOOT = 30;
const START_LIFT = 0.45;

const NS = "http://www.w3.org/2000/svg";
const r1 = (v: number) => Math.round(v * 10) / 10;
const easeInOut = (t: number) => (t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2);

type Els = {
  tint: SVGPathElement;
  restLead: SVGLineElement;
  rest: SVGLineElement;
  live: SVGGElement;
  fill: SVGPathElement;
  lead: SVGLineElement;
  ahead: SVGPathElement;
  gaps: SVGGElement;
  m: SVGPathElement;
  o: SVGPathElement;
  pending: SVGPathElement;
  ticks: SVGGElement;
  map: SVGGElement;
  mapFill: SVGPathElement;
  mapM: SVGPathElement;
  mapO: SVGPathElement;
  mapTicks: SVGGElement;
  pen: SVGCircleElement;
};

function build(svg: SVGSVGElement): Els {
  const el = <K extends keyof SVGElementTagNameMap>(tag: K, cls: string, parent: Element = svg) => {
    const e = document.createElementNS(NS, tag);
    if (cls) e.setAttribute("class", cls);
    if (tag === "path" || tag === "line") e.setAttribute("vector-effect", "non-scaling-stroke");
    parent.appendChild(e);
    return e as SVGElementTagNameMap[K];
  };
  const tint = el("path", "kk-line-tint");
  const restLead = el("line", "kk-line-ahead");
  const rest = el("line", "kk-line-ahead");
  const live = el("g", "");
  const map = el("g", "");
  const els = {
    tint,
    restLead,
    rest,
    live,
    fill: el("path", "kk-line-tint", live),
    lead: el("line", "kk-line-ahead", live),
    ahead: el("path", "kk-line-ahead", live),
    gaps: el("g", "", live),
    m: el("path", "kk-line-m", live),
    o: el("path", "kk-line-o", live),
    pending: el("path", "kk-line-pending", live),
    ticks: el("g", "", live),
    map,
    mapFill: el("path", "kk-line-tint", map),
    mapM: el("path", "kk-line-m", map),
    mapO: el("path", "kk-line-o", map),
    mapTicks: el("g", "", map),
    pen: el("circle", "kk-line-pen"),
  };
  return els;
}

function setLine(e: SVGLineElement, x1: number, x2: number, y1: number, y2 = y1) {
  e.setAttribute("x1", String(r1(x1)));
  e.setAttribute("x2", String(r1(x2)));
  e.setAttribute("y1", String(r1(y1)));
  e.setAttribute("y2", String(r1(y2)));
}

/** A short line: x1, x2, y1, y2. */
type Seg = [number, number, number, number];

/**
 * Replaces a group's short lines (pauses, and screenshots: a line of no length with round ends,
 * which stays a round dot while the zoom stretches the group); there are only ever a handful.
 */
function setLines(g: SVGGElement, cls: string, lines: Seg[]) {
  while (g.childElementCount > lines.length) g.lastElementChild?.remove();
  lines.forEach(([x1, x2, y1, y2], i) => {
    let e = g.children[i] as SVGLineElement | undefined;
    if (!e) {
      e = document.createElementNS(NS, "line");
      e.setAttribute("vector-effect", "non-scaling-stroke");
      g.appendChild(e);
    }
    e.setAttribute("class", cls);
    setLine(e, x1, x2, y1, y2);
  });
}

type Tween = { t: number; ms: number; set: (p: number) => void; done?: () => void };

/** Draws the line: empty before a recording, the pen drawing the levels, or a whole session. */
class DeckLine implements Animated {
  mode: "dot" | "live" | "map" = "dot";
  /** The pen's radius as it comes in with the line (0 while 開始 waits). */
  dotR = 0;
  penR: number;
  /** 0 the line as it waits before a recording … 1 where it is while recording. */
  move = 0;
  /** 0 the live line … 1 the whole session: the zoom out when a session is saved. */
  k = 0;
  /** The whole session's heights: 1 at rest, 0 flat (on its way back to the dot). */
  amp = 1;
  pending = 0;
  pendingFrom = 0;
  shape: Shape | null = null;
  two = true;
  recording = false;
  readonly fps: number;
  private tweens: Tween[] = [];
  private readonly svg: SVGSVGElement;
  private readonly els: Els;
  private readonly size: Size;

  constructor(svg: SVGSVGElement, els: Els, size: Size) {
    this.svg = svg;
    this.els = els;
    this.size = size;
    this.penR = size.pen;
    this.fps = size.fps;
  }

  /** Runs `set` from 0 to 1 (eased) over `ms`; with Windows' animation effects off it jumps to the end. */
  tween(ms: number, set: (p: number) => void, done?: () => void) {
    if (reducedMotion()) {
      set(1);
      done?.();
      this.draw();
      return;
    }
    this.tweens.push({ t: 0, ms, set, done });
    animate(this);
  }

  stop() {
    this.tweens = [];
    stopAnimating(this);
  }

  frame(dt: number): boolean {
    for (const tw of [...this.tweens]) {
      tw.t += dt * 1000;
      const p = Math.min(1, tw.t / tw.ms);
      tw.set(easeInOut(p));
      if (p >= 1) {
        this.tweens.splice(this.tweens.indexOf(tw), 1);
        tw.done?.();
      }
    }
    this.draw();
    return this.tweens.length > 0 || (this.mode === "live" && this.recording);
  }

  draw() {
    const w = this.svg.clientWidth;
    const h = this.svg.clientHeight;
    if (!w) return;
    const s = this.size;
    const e = this.els;
    // Before a recording the expanded window holds 開始 in the middle of the free space, with no
    // line yet; recording, the line lies near the top of the deck.
    const restBase = s.centred ? (h - START_FOOT) * START_LIFT : s.startBase;
    const base = this.mode === "dot" ? restBase + (s.base - restBase) * this.move : s.base;
    e.tint.setAttribute("d", `M-2 ${r1(base)}H${w + 2}V${h + 2}H-2Z`);
    if (this.mode === "dot") {
      const restX = s.centred ? w / 2 : s.dotX;
      const x = restX + (s.startX - restX) * this.move;
      // The line appears as 開始 fades and the pen comes down to where it starts; the compact one
      // waits under 開始 on its line.
      setLine(e.rest, x, w, base);
      e.rest.setAttribute("opacity", s.centred ? this.move.toFixed(3) : "1");
      setLine(e.restLead, 0, x, base);
      e.restLead.setAttribute("opacity", this.move.toFixed(3));
      e.pen.setAttribute("cx", String(r1(x)));
      e.pen.setAttribute("cy", String(r1(base)));
      e.pen.setAttribute("r", String(r1(Math.max(0, this.dotR))));
      e.live.setAttribute("opacity", "0");
      e.map.setAttribute("opacity", "0");
      return;
    }
    e.rest.setAttribute("opacity", "0");
    e.restLead.setAttribute("opacity", "0");
    let ax: number = s.startX;
    if (this.mode === "live") {
      const frac = this.recording && !reducedMotion() ? Math.min(1, Math.max(0, (performance.now() - lineHistory.at) / 100)) : 0;
      const g = { base, up: s.up, down: s.down, step: s.step, startX: s.startX, endPad: s.endPad };
      const l = liveLine(lineHistory.samples, frac, this.pending, w, g, this.two);
      e.fill.setAttribute("d", l.fill);
      e.o.setAttribute("d", l.o);
      e.m.setAttribute("d", l.m);
      e.pending.setAttribute("d", l.pending);
      setLines(e.gaps, "kk-line-gap", l.gaps.map(([a, b]): Seg => [a, b, base, base]));
      setLines(e.ticks, "kk-line-shot", l.ticks.map((x): Seg => [x, x, base, base]));
      // Grey where nothing is written: before the pen started, and ahead of the line.
      setLine(e.lead, 0, l.x0, base);
      e.ahead.setAttribute("d", l.ahead);
      e.pen.setAttribute("cx", String(r1(l.pen[0])));
      e.pen.setAttribute("cy", String(r1(l.pen[1])));
      e.pen.setAttribute("r", String(r1(Math.max(0, this.penR))));
      ax = l.pen[0];
    } else {
      e.pen.setAttribute("r", "0");
    }
    if (this.shape && (this.k > 0 || this.mode === "map")) {
      const a = s.mapAmp * this.amp;
      const l = mapLine(this.shape, w, 0, base, s.up * a, s.down * a, this.two);
      e.mapFill.setAttribute("d", l.fill);
      e.mapO.setAttribute("d", l.o);
      e.mapM.setAttribute("d", l.m);
      setLines(e.mapTicks, "kk-line-shot", l.ticks.map((x): Seg => [x, x, base, base]));
    }
    // The zoom out: the live line shrinks towards the pen while the whole session shrinks into place.
    const k = this.mode === "map" ? 1 : this.k;
    const sl = 1 - 0.92 * k;
    const sm = 5 - 4 * k;
    e.live.setAttribute("transform", `translate(${r1(ax * (1 - sl))} 0) scale(${sl.toFixed(4)} 1)`);
    e.live.setAttribute("opacity", (1 - k).toFixed(3));
    e.map.setAttribute("transform", `translate(${r1(ax * (1 - sm))} 0) scale(${sm.toFixed(4)} 1)`);
    e.map.setAttribute("opacity", k.toFixed(3));
  }
}

/**
 * The bottom of the main window and of a past session: one line across the window. Before a
 * recording 開始 waits in the middle of the free space (in the compact window, at the start of an
 * empty line). Pressing it brings the line down to the bottom with a pen tip that draws it as it
 * moves: 相手 above the line in the accent, 自分 below it in ink, over the accent's tint. When the
 * session is saved the pen lifts and the line zooms out to the whole session, which you can point
 * at for the time and click to jump there; the finished session's buttons sit under it.
 */
export function Deck({
  compact = false,
  phase,
  sessionId,
  folder,
  onNew = false,
}: {
  compact?: boolean;
  phase: "start" | "live" | "review";
  sessionId?: string;
  folder?: string;
  /** Shows 「新しい録音」 (the main window only). */
  onNew?: boolean;
}) {
  const recState = useRecording((s) => s.state);
  const finishing = useRecording((s) => s.finishing);
  const state: DeckState =
    phase === "start" ? "start" : phase === "review" ? "review" : recState === "paused" ? "paused" : recState === "finishing" ? "finishing" : "recording";
  const store = useTranscriptStore();
  const items = store((s) => s.items);
  const durationMs = store((s) => s.durationMs);
  const startedAt = store((s) => s.startedAt);
  const continued = store((s) => s.continued);
  const sessionSources = store((s) => s.sources);
  const fromFile = sessionSources.some((s) => s.id === "file");
  const clock = useMemo(() => (startedAt ? sessionClock(startedAt, continued, fromFile) : undefined), [startedAt, continued, fromFile]);
  const recorded = useRecording((s) => s.sources?.ids);
  const sound = store((s) => s.sound);
  const mode = useSource((s) => s.mode);
  const includeMic = useSource((s) => s.includeMic);
  // Until the session is loaded, the sources come from the recording, or before it from the picker.
  const ids: SourceId[] = sessionSources.length
    ? sessionSources.map((s) => s.id)
    : recorded
      ? recorded
      : mode === "mic"
      ? ["mic"]
      : [mode === "app" ? "app" : "system", ...(includeMic ? (["mic"] as SourceId[]) : [])];
  const two = ids.includes("mic") && ids.some((id) => id !== "mic");
  const size = compact ? SIZES.compact : SIZES.expanded;
  // The expanded deck fills the free space before a recording, so its dot sits in the middle; once
  // there is a transcript, it reaches up under the deck to the line.
  const grows = state === "start" && size.centred;
  const under = !compact && state !== "start";
  let total = durationMs ?? 0;
  for (const i of items) if (i.kind === "segment") total = Math.max(total, i.tEndMs);

  const rootRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const lineRef = useRef<DeckLine | null>(null);
  const prev = useRef<DeckState | null>(null);
  const lastHeight = useRef(0);
  const latest = useRef({ items, durationMs, two, sound });
  latest.current = { items, durationMs, two, sound };
  const [scrub, setScrub] = useState<{ x: number; ms: number } | null>(null);
  const player = useSessionAudio(state === "review");
  const playingMs = store((s) => s.playingMs);

  useLayoutEffect(() => {
    const root = rootRef.current;
    const svg = svgRef.current;
    if (!root || !svg) return;
    const els = build(svg);
    const line = new DeckLine(svg, els, size);
    const st = prev.current ?? state;
    line.two = latest.current.two;
    line.recording = st === "recording";
    if (st !== "start") {
      line.move = 1;
      line.mode = st === "review" ? "map" : "live";
      if (st === "review") {
        line.k = 1;
        line.shape = sessionShape(latest.current.items, latest.current.durationMs, latest.current.sound);
      }
    }
    lineRef.current = line;
    lastHeight.current = root.getBoundingClientRect().height;
    line.draw();
    if (line.recording) animate(line);
    const ro = new ResizeObserver(() => {
      lastHeight.current = root.getBoundingClientRect().height;
      line.draw();
    });
    ro.observe(root);
    return () => {
      ro.disconnect();
      line.stop();
      lineRef.current = null;
      svg.replaceChildren();
    };
    // One line per size; it reads the state through `prev` and `latest`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [compact]);

  // Moving between states: 開始 gives way to the pen, the line comes to rest, and back. The deck
  // has already been laid out at its new height (taller before a recording); it is held at the old
  // one and eased to the new one together with the line, before anything is painted.
  useLayoutEffect(() => {
    const line = lineRef.current;
    const root = rootRef.current;
    const from = prev.current;
    prev.current = state;
    if (!line || !root || from === state) return;
    line.recording = state === "recording";
    if (from === null) {
      line.draw();
      return;
    }
    const fromH = lastHeight.current;
    const toH = root.getBoundingClientRect().height;
    const resizes = Math.abs(toH - fromH) > 0.5;
    // While it moves, 開始 and what waits under it stay hidden (`data-moving`; React never sets it).
    const hold = () => {
      if (!resizes) return;
      root.dataset.moving = "";
      root.style.flex = "none";
      root.style.height = `${fromH}px`;
    };
    const ease = (p: number) => {
      if (resizes) root.style.height = `${fromH + (toH - fromH) * p}px`;
    };
    const release = () => {
      delete root.dataset.moving;
      root.style.flex = "";
      root.style.height = "";
    };
    if (state === "start") {
      setScrub(null);
      if (from === "review") {
        // 「新しい録音」: the session's line flattens and rises to the middle, the pen fades and 開始 comes back.
        hold();
        line.tween(300, (p) => (line.amp = 1 - p), () => {
          line.mode = "dot";
          line.k = 0;
          line.amp = 1;
          line.shape = null;
          line.move = 1;
          line.dotR = size.pen;
          line.tween(
            480,
            (p) => {
              ease(p);
              line.move = 1 - p;
              line.dotR = size.pen * (1 - p);
            },
            release,
          );
        });
      } else {
        line.stop();
        release();
        line.mode = "dot";
        line.k = 0;
        line.move = 0;
        line.dotR = 0;
        line.draw();
      }
      return;
    }
    if (from === "start") {
      // 開始: the line sinks to the bottom while the pen comes in at its start.
      line.stop();
      hold();
      line.mode = "dot";
      line.tween(
        480,
        (p) => {
          ease(p);
          line.move = p;
          line.dotR = size.pen * p;
        },
        () => {
          release();
          line.mode = "live";
          line.move = 1;
          line.penR = size.pen;
          line.k = 0;
          animate(line);
        },
      );
      return;
    }
    if (state === "finishing") {
      line.pendingFrom = Math.min(40, lineHistory.samples.length);
      line.pending = line.pendingFrom;
      line.draw();
      return;
    }
    if (state === "review") {
      line.pending = 0;
      line.shape = sessionShape(latest.current.items, latest.current.durationMs, latest.current.sound);
      if (line.mode !== "live") {
        line.mode = "map";
        line.k = 1;
        line.draw();
        return;
      }
      line.tween(260, (p) => (line.penR = size.pen * (1 - p)), () =>
        line.tween(800, (p) => (line.k = p), () => {
          line.mode = "map";
          line.draw();
        }),
      );
      return;
    }
    // Paused or resumed.
    line.draw();
    if (state === "recording") animate(line);
  }, [state, size]);

  // While finishing, the unwritten tail fills in as the queue empties.
  useEffect(() => {
    const line = lineRef.current;
    if (!line || state !== "finishing") return;
    const left = !finishing ? 1 : finishing.total > 0 ? Math.max(0, (finishing.total - finishing.done) / finishing.total) : 0;
    line.pending = Math.round(line.pendingFrom * left);
    line.draw();
  }, [finishing, state]);

  // A finished session's line follows the session: its sound, or its transcript, and its screenshots.
  useEffect(() => {
    const line = lineRef.current;
    if (!line) return;
    line.two = two;
    if (state === "review") line.shape = sessionShape(items, durationMs, sound);
    if (line.mode !== "live" || !line.recording) line.draw();
  }, [items, durationMs, sound, two, state]);

  const scrubAt = (clientX: number, clientY: number) => {
    const svg = svgRef.current;
    if (!svg || state !== "review" || total <= 0) return null;
    const b = svg.getBoundingClientRect();
    if (clientY - b.top > size.base + 18) return null;
    const u = Math.min(1, Math.max(0, (clientX - b.left) / b.width));
    return { x: u * b.width, ms: u * total };
  };

  const style = {
    "--kk-h": `${size.height}px`,
    "--kk-dx": `${size.dotX}px`,
    "--kk-sb": `${size.startBase}px`,
    "--kk-b": `${size.base}px`,
    "--kk-rest": `calc((100% - ${START_FOOT}px) * ${START_LIFT})`,
  } as CSSProperties;
  return (
    <div
      ref={rootRef}
      data-deck={state}
      className={`kk-deck relative ${compact ? "kk-deck-compact" : ""} ${grows ? "kk-deck-grow" : ""} ${under ? "kk-deck-under" : ""}`}
      style={style}
      onPointerMove={(e) => setScrub(scrubAt(e.clientX, e.clientY))}
      onPointerLeave={() => setScrub(null)}
      onClick={(e) => {
        const at = scrubAt(e.clientX, e.clientY);
        if (at && !(e.target as HTMLElement).closest("button")) store.getState().seekTo(at.ms);
      }}
    >
      <svg ref={svgRef} className="kk-line pointer-events-none absolute inset-0 h-full w-full overflow-visible" aria-hidden />
      {/* Where the finished session's line takes the pointer; above it, the pointer reaches the text. */}
      {under && state === "review" && <div className="kk-scrub-zone" aria-hidden />}
      {state === "start" && <StartRow compact={compact} />}
      {(state === "recording" || state === "paused") && <LiveRow compact={compact} paused={state === "paused"} />}
      {state === "finishing" && <FinishingRow />}
      {state === "review" && sessionId && <ReviewRow compact={compact} sessionId={sessionId} folder={folder} onNew={onNew} player={player} />}
      {/* Where the sound being played is, on the session's line (FR-09). */}
      {state === "review" && playingMs !== null && total > 0 && <div className="kk-playhead" style={{ left: `${Math.min(100, (playingMs / total) * 100)}%` }} aria-hidden />}
      {state === "review" && !compact && clock && total > 0 && (
        <div className="kk-line-ends num" aria-hidden>
          {/* A file's ends are its start and its length (FR-18); hours and minutes would read as minutes and seconds. */}
          <span>{fromFile ? timerText(0) : clock(0).slice(0, 5)}</span>
          <span>{fromFile ? timerText(total) : clock(total).slice(0, 5)}</span>
        </div>
      )}
      {scrub && (
        <div className="kk-scrub" style={{ left: scrub.x }} aria-hidden>
          <span className="num">{clock ? clock(scrub.ms) : timerText(scrub.ms)}</span>
        </div>
      )}
    </div>
  );
}

function HotkeyHint({ className }: { className: string }) {
  const { t } = useTranslation();
  const key = useSettings((s) => s.settings?.hotkeys.toggle);
  const taken = useSettings((s) => !!s.info?.hotkeyErrors.toggle);
  if (!key || taken) return null;
  return <span className={className}>{t("startHint", { key: key.replaceAll("+", " + ") })}</span>;
}

/**
 * 開始, the one primary action before a recording, a round button with the record dot. In the
 * expanded window a hotkey (when one is
 * set) is under it and the model sits at the bottom, so 開始 alone is centred; in the compact one
 * it sits on the empty line where the pen will start.
 */
function StartRow({ compact }: { compact: boolean }) {
  const { t } = useTranslation();
  const state = useRecording((s) => s.state);
  const info = useSettings((s) => s.info);
  const [starting, setStarting] = useState(false);
  const disabled = state !== "ready" || !info || starting;
  // The other way in (FR-18): a recording made elsewhere, transcribed as fast as the PC can.
  const fileLink = (
    <button type="button" onClick={() => void transcribeFile()} disabled={disabled} className="kk-link inline-flex items-center gap-1.5 disabled:opacity-45">
      <FileAudio size={13} aria-hidden />
      {t("transcribeFile")}
    </button>
  );
  const start = async () => {
    setStarting(true);
    try {
      await toggleRecording();
    } finally {
      setStarting(false);
    }
  };
  return (
    <div className="absolute inset-0 animate-fade-in">
      <button
        type="button"
        className="kk-go button button-primary"
        onClick={start}
        disabled={disabled}
        data-starting={starting || undefined}
        aria-label={t("start")}
        title={t("start")}
      >
        <span className="kk-go-dot" aria-hidden />
      </button>
      {compact ? (
        <HotkeyHint className="kk-go-hint" />
      ) : (
        <>
          <div className="kk-go-under">
            <HotkeyHint className="" />
            {fileLink}
          </div>
          <div className="kk-go-foot">
            <ModelNote />
          </div>
        </>
      )}
    </div>
  );
}

/**
 * What the camera takes (FR-34): the recorded app's window, the cursor's screen or every screen
 * (the setting, remembered), or any open window for the rest of this recording.
 */
async function shotMenu(t: (k: string, p?: Record<string, unknown>) => string) {
  const rec = useRecording.getState();
  const settings = useSettings.getState();
  const target = settings.settings?.screenshot.target ?? "appWindow";
  const picked = rec.shotWindow;
  const app = rec.sources?.ids.includes("app") ? rec.sources.appName : undefined;
  const windows = await commands.listWindows().catch((): WindowInfo[] => []);
  if (picked && !windows.some((w) => w.id === picked.id)) windows.unshift(picked);
  const choose = (next: "appWindow" | "cursorMonitor" | "allMonitors") => async () => {
    try {
      if (picked) await commands.setShotWindow(null);
      if (next !== target) await settings.update({ screenshot: { target: next } });
    } catch (e) {
      reportError(e);
    }
  };
  // Without an app recorded, the app's window means the cursor's screen.
  const cursor = target === "cursorMonitor" || (target === "appWindow" && !app);
  const entries: MenuEntry[] = [
    ...(app ? [{ text: menuText(t("shotWindowOf", { app })), checked: !picked && target === "appWindow", action: choose("appWindow") }] : []),
    { text: t("targetCursor"), checked: !picked && cursor, action: choose("cursorMonitor") },
    { text: t("targetAll"), checked: !picked && target === "allMonitors", action: choose("allMonitors") },
    "separator",
    {
      text: t("shotPickWindow"),
      items: windows.length
        ? windows.map((w) => ({
            text: menuText(w.title.length > 60 ? `${w.title.slice(0, 59)}…` : w.title),
            checked: picked?.id === w.id,
            action: () => void commands.setShotWindow(w).catch(reportError),
          }))
        : [{ text: t("noWindows"), enabled: false, action: () => {} }],
    },
  ];
  await popupMenu(entries).catch(reportError);
}

/** The time in big numerals, then pause or resume, stop, screenshot (with what it takes), mark and cut. */
function LiveRow({ compact, paused }: { compact: boolean; paused: boolean }) {
  const { t } = useTranslation();
  const elapsed = useElapsed();
  const lag = useRecording((s) => (s.lag ? Math.round(s.lag.lagMs / 1000) : 0));
  const shotWindow = useRecording((s) => s.shotWindow);
  const deferred = useRecording((s) => s.deferred);
  // Waiting for Stop, the lag only grows and says nothing (FR-25).
  const note = paused ? t("paused") : deferred ? t("deferredNote") : lag >= 2 ? t("lag", { n: lag }) : "";
  // Small and grey while it may catch up, bold from 15 s, yellow from 30 s (FR-25).
  const noteClass = paused || deferred || lag < 15 ? "text-muted" : lag < 30 ? "font-medium text-fg" : "font-medium text-lag";
  const ctl = "kk-ctl grid shrink-0 place-items-center rounded-md border border-line-strong bg-surface text-fg transition-colors duration-150 hover:bg-surface-2 disabled:pointer-events-none disabled:opacity-40";
  return (
    <>
      {/* The row has no room left beside the timer, so the note sits on the line. */}
      {note && (
        <span className={`kk-line-note truncate text-[11.5px] ${noteClass}`} aria-live="polite">
          {note}
          {deferred && !paused && (
            <>
              {" · "}
              <button type="button" onClick={() => void commands.deferTranscription(false).catch(reportError)} className="pointer-events-auto font-medium text-accent-text underline-offset-2 hover:underline">
                {t("resumeTranscription")}
              </button>
            </>
          )}
        </span>
      )}
      <div className="kk-row flex items-center justify-between gap-3 animate-fade-in [animation-delay:300ms]">
        <span className="flex min-w-0 items-baseline gap-2.5">
          <span className={`num kk-timer ${paused ? "text-muted" : ""}`} aria-label={t("elapsed")}>
            {timerText(elapsed)}
          </span>
        </span>
        <span className="flex shrink-0 items-center gap-2">
          <button
            type="button"
            onClick={pauseOrResume}
            aria-pressed={paused}
            aria-label={paused ? t("resume") : t("pause")}
            title={paused ? t("resume") : t("pause")}
            className={`${ctl} ${paused ? "!border-accent-soft !bg-accent-soft !text-accent-text" : ""}`}
          >
            {paused ? <Play size={16} fill="currentColor" /> : <Pause size={16} fill="currentColor" />}
          </button>
          <button
            type="button"
            onClick={toggleRecording}
            aria-label={t("stop")}
            className={`button button-primary shrink-0 !py-0 ${compact ? "h-9 !px-4 !text-[13px]" : "h-[42px] !px-[18px] !text-[14px]"}`}
          >
            <Square size={10} fill="currentColor" />
            {t("stop")}
          </button>
          <span className="flex shrink-0">
            <button type="button" onClick={takeScreenshot} disabled={paused} aria-label={t("screenshot")} title={shotWindow ? `${t("screenshot")}: ${shotWindow.title}` : t("screenshot")} className={`${ctl} kk-split-l`}>
              <Camera size={17} />
            </button>
            <button type="button" onClick={() => shotMenu(t)} aria-haspopup="menu" aria-label={t("shotTarget")} title={t("shotTarget")} className={`${ctl} kk-split-r`}>
              <ChevronDown size={12} />
            </button>
          </span>
          <button type="button" onClick={markCurrentLine} aria-label={t("markLine")} title={t("markLine")} className={ctl}>
            <Star size={16} />
          </button>
          <button type="button" onClick={addCut} aria-label={t("cut")} title={t("cut")} className={ctl}>
            <Scissors size={16} />
          </button>
        </span>
      </div>
    </>
  );
}

/** Finishing this long shows a progress bar; a short one is over before a bar would mean anything. */
const FINISHING_BAR_MS = 5_000;

function FinishingRow() {
  const { t } = useTranslation();
  const finishing = useRecording((s) => s.finishing);
  // A file (FR-18) shows how far into it the text has got, from the start.
  const fromFile = useRecording((s) => !!s.sources?.ids.includes("file"));
  const left = finishing && finishing.total > 0 ? finishing.total - finishing.done : null;
  const [long, setLong] = useState(false);
  useEffect(() => {
    const id = setTimeout(() => setLong(true), FINISHING_BAR_MS);
    return () => clearTimeout(id);
  }, []);
  // By audio rather than by line: the lines left differ in length, merged ones most.
  const bar = (long || fromFile) && finishing && finishing.totalMs > 0;
  return (
    <div className="kk-row flex items-center justify-between gap-3 text-[12.5px] animate-fade-in" title={fromFile ? undefined : t("finishingDetail")}>
      <span className="flex min-w-0 flex-1 flex-col gap-1.5">
        {/* The spinner says it is still working while one long stretch keeps the count still. */}
        <span className="flex min-w-0 items-center gap-1.5">
          <Spinner size={12} className="shrink-0 text-muted" />
          <span className="min-w-0 truncate">
            <b className="font-medium">{fromFile ? t("transcribingFile") : t("finishing")}</b>
            {fromFile && finishing && finishing.totalMs > 0 && (
              <span className="num text-muted">
                {" "}
                · {timerText(finishing.doneMs)} / {timerText(finishing.totalMs)}
              </span>
            )}
            {left !== null && <span className="text-muted"> · {t("finishingLeft", { n: left })}</span>}
          </span>
        </span>
        {bar && <Progress value={finishing.doneMs} max={finishing.totalMs} className="max-w-[260px] animate-fade-in" />}
      </span>
      <button type="button" onClick={() => commands.cancelFinishing()} className="kk-link shrink-0">
        {t("stopFinishing")}
      </button>
    </div>
  );
}

/** Play (when the session kept its sound), copy, 書き出し, more, and 「新しい録音」. */
function ReviewRow({ compact, sessionId, folder, onNew, player }: { compact: boolean; sessionId: string; folder?: string; onNew: boolean; player: Player }) {
  const { t } = useTranslation();
  const setExportFor = useUi((s) => s.setExportFor);
  const title = useTranscriptStore()((s) => s.title);
  // The session's menu, as 履歴's rows have it, less what this row and the header already hold.
  const more = () => void popupMenu(sessionEntries({ id: sessionId, title: title ?? "", folder }, false)).catch(reportError);
  const copyMenu = () => void popupMenu(copyEntries(sessionId, false)).catch(reportError);
  // 書き出し picks a format in a dialog; the compact window is too small for one, so a menu there.
  const exportMenu = async () => {
    const entries: MenuEntry[] = [
      { text: t("exportMarkdown"), action: () => void exportAs("markdown", sessionId) },
      { text: t("exportMarkdownZip"), action: () => void exportAs("markdown", sessionId, true) },
      { text: t("exportPdf"), action: () => void exportAs("pdf", sessionId) },
      { text: t("exportTypst"), action: () => void exportAs("typst", sessionId) },
    ];
    await popupMenu(entries).catch(reportError);
  };
  const act = `kk-act inline-flex shrink-0 items-center gap-2 rounded-md border border-line-strong bg-surface font-medium whitespace-nowrap transition-colors duration-150 hover:bg-surface-2 ${compact ? "h-9 px-3 text-[12.5px]" : "h-10 px-3 text-[13px]"}`;
  return (
    <div className="kk-row flex items-center gap-2 animate-fade-in [animation-delay:500ms]">
      {player.available && (
        <button
          type="button"
          className={`${act} justify-center !px-0 ${compact ? "!w-9" : "!w-10"} ${player.playing ? "!border-accent-soft !bg-accent-soft !text-accent-text" : ""}`}
          onClick={player.toggle}
          aria-pressed={player.playing}
          aria-label={player.playing ? t("pausePlayback") : t("play")}
          title={player.playing ? t("pausePlayback") : t("play")}
        >
          {player.playing ? <Pause size={15} fill="currentColor" /> : <Play size={15} fill="currentColor" />}
        </button>
      )}
      {/* コピー copies plain text; its chevron picks the other formats, joined like the camera. */}
      <span className="flex shrink-0">
        <button type="button" className={`${act} kk-split-l`} onClick={() => copy("plain", sessionId)}>
          <Copy size={15} />
          {t("copy")}
        </button>
        <button type="button" className={`${act} kk-split-r`} onClick={copyMenu} aria-haspopup="menu" aria-label={t("copyAs")} title={t("copyAs")}>
          <ChevronDown size={14} />
        </button>
      </span>
      {compact ? (
        <button type="button" className={`${act} !w-9 justify-center !px-0`} onClick={exportMenu} aria-haspopup="menu" aria-label={t("export")} title={t("export")}>
          <FileOutput size={16} />
        </button>
      ) : (
        <button type="button" className={act} onClick={() => setExportFor(sessionId)} aria-haspopup="dialog">
          <FileOutput size={15} />
          {t("export")}
        </button>
      )}
      <button type="button" className={`${act} !w-10 justify-center !px-0 ${compact ? "!w-9" : ""}`} onClick={more} aria-haspopup="menu" aria-label={t("moreActions")} title={t("moreActions")}>
        <Ellipsis size={17} />
      </button>
      <span className="min-w-1 flex-1" />
      {onNew && (
        <button type="button" className={act} onClick={newRecording}>
          {t("newRecording")}
        </button>
      )}
    </div>
  );
}
