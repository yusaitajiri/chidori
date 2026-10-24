// A stand-in for the Rust side, for tests of the whole app: commands change its state and it
// reports a recording with the same events the backend sends; the test says what is heard.

import { act } from "@testing-library/react";
import type { AppInfo, AudioFile, Segment, SessionView, Settings, SourceConfig, SourceId, StatePayload } from "../ipc/types";
import { makeInfo, makeModel, makeSettings, merge } from "./fixtures";
import { answer, emit } from "./tauri";

export const SESSION_ID = "2026-10-18_1900_system";
export const FOLDER = `C:\\Users\\me\\Documents\\Chidori\\${SESSION_ID}`;

export class FakeBackend {
  settings: Settings = makeSettings();
  info: AppInfo = makeInfo();
  session: SessionView | null = null;
  private source: SourceConfig | null = null;
  private seq = 0;

  constructor() {
    answer(this.commands());
  }

  /** Answers every command the app calls on the way, so a test can add or replace some. */
  commands(): Record<string, unknown> {
    return {
      get_settings: () => this.settings,
      get_app_info: () => this.info,
      set_settings: ({ partial }: { partial: object }) => (this.settings = merge(this.settings, partial)),
      models_list: [makeModel("turbo-q5", { nameJa: "標準", installed: true, selected: true, recommended: true })],
      list_audio_apps: [],
      list_mic_devices: [],
      list_recoverable: [],
      list_projects: [],
      start_recording: ({ source }: { source: SourceConfig }) => this.start(source),
      pause_recording: () => this.report({ state: "paused" }),
      resume_recording: () => this.report({ state: "recording" }),
      stop_recording: () => this.report({ state: "finishing" }),
      transcribe_file: ({ path }: { path?: string }) => this.transcribeFile(path ?? this.picked),
      get_session: ({ sessionId }: { sessionId: string }) => {
        if (this.session?.id !== sessionId) return Promise.reject({ code: "E_INTERNAL", message: "no such session" });
        return structuredClone(this.session);
      },
      copy_transcript: ({ sessionId }: { sessionId: string }) => ({ chars: this.session?.id === sessionId ? 10 : 0 }),
    };
  }

  /** The app opens while a recording runs (started by the hotkey, or the window reloaded). */
  recordingAtLaunch(lines: [SourceId, string, number][]) {
    this.start({ mode: "system", includeMic: true }, false);
    for (const [source, text, t] of lines) {
      this.seq += 1;
      this.session?.items.push({ kind: "segment", id: `seg_${this.seq}`, utteranceId: `utt_${this.seq}`, source, tStartMs: t, tEndMs: t + 2000, text, edited: false });
    }
    this.info = { ...this.info, state: { ...this.info.state, elapsedMs: 90_000 } };
  }

  private start(source: SourceConfig, announce = true) {
    this.source = source;
    const ids: SourceId[] = source.mode === "mic" ? ["mic"] : [source.mode === "app" ? "app" : "system", ...(source.includeMic ? (["mic"] as SourceId[]) : [])];
    this.session = {
      v: 1,
      id: SESSION_ID,
      title: "システム全体 2026-10-18 19:00",
      startedAt: "2026-10-18T19:00:00+09:00",
      durationMs: 0,
      sources: ids.map((id) => ({ id, label: id })),
      model: { id: "turbo-q5", sha256: "0".repeat(64) },
      language: "ja",
      gpu: true,
      items: [],
      folder: FOLDER,
    };
    this.report({ state: "recording", elapsedMs: 0 }, announce);
    return { sessionId: SESSION_ID, folder: FOLDER };
  }

  /** The file the open dialog returns; `null` cancels it. */
  picked: string | null = "C:\\Users\\me\\Videos\\lecture.mp4";

  /** A file transcribed into a new session (FR-18): it finishes from the start, named by the file. */
  private transcribeFile(path: string | null) {
    if (!path) return null;
    const name = path.split("\\").pop() ?? path;
    this.session = {
      v: 1,
      id: SESSION_ID,
      title: name.replace(/\.[^.]+$/, ""),
      startedAt: "2026-10-18T19:00:00+09:00",
      durationMs: 0,
      sources: [{ id: "file", label: "相手", name }],
      model: { id: "turbo-q5", sha256: "0".repeat(64) },
      language: "ja",
      gpu: true,
      items: [],
      folder: FOLDER,
    };
    this.report({ state: "finishing", sources: { ids: ["file"], appName: name } });
    return { sessionId: SESSION_ID, folder: FOLDER };
  }

  /** The recording's state as the backend reports it (`recording://state`). */
  private report(p: StatePayload, announce = true) {
    const busy = p.state === "recording" || p.state === "paused" || p.state === "finishing";
    const ids = this.session?.sources.map((s) => s.id) ?? [];
    const full: StatePayload = busy && this.session ? { sessionId: this.session.id, folder: FOLDER, sources: { ids }, ...p } : p;
    this.info = { ...this.info, state: full };
    if (announce) emit("recording://state", full);
    return undefined;
  }

  /** Words still being decoded, in grey. */
  partial(source: SourceId, text: string, tStartMs: number) {
    act(() => emit("transcript://partial", { source, utteranceId: `utt_${this.seq + 1}`, text, tStartMs }));
  }

  /** A final line, which replaces the grey words of the same utterance. */
  hear(source: SourceId, text: string, tStartMs: number, tEndMs = tStartMs + 2000): Segment {
    this.seq += 1;
    const seg: Segment = { id: `seg_${this.seq}`, utteranceId: `utt_${this.seq}`, source, tStartMs, tEndMs, text, edited: false };
    this.session?.items.push({ kind: "segment", ...seg });
    act(() => emit("transcript://segment", seg));
    return seg;
  }

  screenshot(id: string, tMs: number) {
    const shot = { id, tMs, file: `images/${id}.png`, width: 16, height: 9 };
    this.session?.items.push({ kind: "screenshot", ...shot });
    act(() => emit("transcript://screenshot", { ...shot, thumbDataUrl: "data:image/png;base64," }));
  }

  progress(done: number, total: number) {
    act(() => emit("finishing://progress", { done, total, doneMs: done * 1000, totalMs: total * 1000 }));
  }

  /**
   * The queue has drained: the session is written, with its sound when the settings keep it (one
   * file, or `audio` for a session recorded in several parts).
   */
  save(durationMs: number, audio: AudioFile[] = [{ file: "audio/0001.ogg", startMs: 0 }]) {
    const s = this.session;
    if (!s) throw new Error("nothing recorded");
    s.endedAt = "2026-10-18T19:10:00+09:00";
    s.durationMs = durationMs;
    if (this.settings.audio.record) s.audio = audio;
    act(() => {
      this.report({ state: "ready" });
      emit("session://saved", { sessionId: s.id, folder: FOLDER, transcriptPath: `${FOLDER}\\transcript.md` });
    });
  }

  /** The source the last recording was started with. */
  get startedWith() {
    return this.source;
  }
}
