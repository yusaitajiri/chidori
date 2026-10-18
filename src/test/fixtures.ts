// Backend data for component tests, and a way back to a clean app between tests.

import type { AppInfo, AudioApp, DeepPartial, ModelEntry, Settings } from "../ipc/types";
import { useProjects } from "../store/projects";
import { useRecording } from "../store/recording";
import { useSettings } from "../store/settings";
import { useSource } from "../store/source";
import { useTranscript } from "../store/transcript";
import { useUi } from "../store/ui";

export function makeSettings(): Settings {
  return {
    locale: "ja",
    window: { alwaysOnTop: false, layout: "expanded" },
    updates: { check: true },
    source: { mode: "system", includeMic: true, micDeviceId: null, app: null },
    echoGuard: true,
    vad: { startThreshold: 0.5, hangoverMs: 800 },
    language: "ja",
    modelId: "turbo-q5",
    useGpu: true,
    accuracyFirst: false,
    partials: true,
    vocabulary: [],
    hallucinationFilter: true,
    audioCtxExperimental: false,
    screenshot: { target: "appWindow", excludeSelf: true, sound: true },
    hotkeys: { toggle: "Ctrl+Shift+R", screenshot: "Ctrl+Shift+S", mark: "", cut: "" },
    output: { root: "C:\\Users\\me\\Documents\\Chidori", titleTemplate: "{app} {date}" },
    export: { timestamps: true, labels: "auto", mergeParagraphs: true },
    copy: { screenshotMarkers: true },
    autoCopyOnStop: false,
    audio: { record: true },
  };
}

export function makeInfo(over: Partial<AppInfo> = {}): AppInfo {
  return {
    version: "0.3.0",
    osBuild: 26200,
    appLoopbackSupported: true,
    device: "gpu",
    deviceName: "Test GPU",
    engine: { state: "ready", gpu: true, deviceName: "Test GPU", modelId: "turbo-q5", error: null },
    gpuCompiled: true,
    hotkeyErrors: {},
    state: { state: "ready" },
    modelsDir: "C:\\models",
    logDir: null,
    recommendedModel: "turbo-q5",
    totalRamBytes: 16 * 2 ** 30,
    setupDone: true,
    gpuFailed: false,
    ...over,
  };
}

export function makeModel(id: string, over: Partial<ModelEntry> = {}): ModelEntry {
  return {
    id,
    nameJa: id,
    nameEn: id,
    descriptionJa: `${id} の説明`,
    descriptionEn: `About ${id}`,
    repo: "test/models",
    file: `ggml-${id}.bin`,
    revision: "main",
    sizeBytes: 100 * 2 ** 20,
    sha256: "0".repeat(64),
    license: "MIT",
    speedTier: "ok",
    hidden: false,
    installed: false,
    selected: false,
    recommended: false,
    partialBytes: 0,
    downloading: false,
    benchmark: null,
    ...over,
  };
}

export function makeApp(rootPid: number, name: string, over: Partial<AudioApp> = {}): AudioApp {
  return { rootPid, exe: `${name}.exe`, name, active: true, hasSession: true, ...over };
}

/** `partial` laid over `base`, as the backend merges `set_settings`. */
export function merge<T>(base: T, partial: DeepPartial<T>): T {
  const out = structuredClone(base) as Record<string, unknown>;
  for (const [k, v] of Object.entries(partial as Record<string, unknown>)) {
    const cur = out[k];
    out[k] = v && typeof v === "object" && !Array.isArray(v) && cur && typeof cur === "object" ? merge(cur, v) : v;
  }
  return out as T;
}

/** Every store as the app starts. */
export function resetStores() {
  for (const store of [useRecording, useSettings, useSource, useUi, useProjects, useTranscript]) {
    (store as { setState: (s: unknown, replace: true) => void }).setState(store.getInitialState(), true);
  }
}
