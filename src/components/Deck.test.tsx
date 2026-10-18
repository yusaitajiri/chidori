import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { useRecording } from "../store/recording";
import { useSettings } from "../store/settings";
import { useSource } from "../store/source";
import { useTranscript } from "../store/transcript";
import { useUi } from "../store/ui";
import { makeInfo, makeSettings, resetStores } from "../test/fixtures";
import { answer, calls, fails } from "../test/tauri";
import { Deck } from "./Deck";

function ready() {
  useSettings.setState({ settings: makeSettings(), info: makeInfo() });
  useRecording.setState({ state: "ready" });
}

function recording(over: Partial<ReturnType<typeof useRecording.getState>> = {}) {
  ready();
  useRecording.setState({ state: "recording", sessionId: "s1", elapsedMs: 65_000, elapsedAt: Date.now(), ...over });
}

const toasts = () => useUi.getState().toasts.map((t) => t.text);

describe("Deck before a recording", () => {
  beforeEach(resetStores);

  it("starts recording what the picker chose", async () => {
    ready();
    render(<Deck phase="start" />);
    await userEvent.click(screen.getByRole("button", { name: "開始" }));
    expect(calls("start_recording")).toEqual([{ source: { mode: "system", includeMic: true, app: undefined, micDeviceId: undefined }, title: undefined }]);
  });

  it("waits for the app to load before 開始 can be pressed", () => {
    useRecording.setState({ state: "ready" });
    render(<Deck phase="start" />);
    expect(screen.getByRole("button", { name: "開始" })).toBeDisabled();
  });

  it("asks for an app instead of starting without one", async () => {
    ready();
    useSource.setState({ mode: "app", app: null });
    render(<Deck phase="start" />);
    await userEvent.click(screen.getByRole("button", { name: "開始" }));
    expect(calls("start_recording")).toEqual([]);
    expect(toasts()).toEqual(["アプリを選んでください"]);
  });

  it("shows why a recording could not start", async () => {
    ready();
    answer({ start_recording: fails("E_MIC_DENIED") });
    render(<Deck phase="start" />);
    await userEvent.click(screen.getByRole("button", { name: "開始" }));
    expect(useUi.getState().banners).toMatchObject([{ code: "E_MIC_DENIED", message: "micDenied" }]);
    expect(screen.getByRole("button", { name: "開始" })).toBeEnabled();
  });

  it("names the start shortcut, unless another app holds it", () => {
    ready();
    const { unmount } = render(<Deck phase="start" />);
    expect(screen.getByText("Ctrl + Shift + R でも開始できます")).toBeInTheDocument();
    unmount();
    useSettings.setState({ info: makeInfo({ hotkeyErrors: { toggle: "taken" } }) });
    render(<Deck phase="start" />);
    expect(screen.queryByText(/でも開始できます/)).not.toBeInTheDocument();
  });

  it("shows the model it will use, which opens the model settings", async () => {
    ready();
    render(<Deck phase="start" />);
    await userEvent.click(screen.getByRole("button", { name: "モデルを変更: turbo-q5" }));
    expect(useUi.getState()).toMatchObject({ view: "settings", settingsTab: "models" });
  });
});

describe("Deck while recording", () => {
  beforeEach(resetStores);

  it("shows the time and stops", async () => {
    recording();
    render(<Deck phase="live" />);
    expect(screen.getByLabelText("経過時間")).toHaveTextContent("01:05");
    await userEvent.click(screen.getByRole("button", { name: "停止" }));
    expect(calls("stop_recording")).toHaveLength(1);
  });

  it("pauses, and while paused says so, resumes, and takes no screenshots", async () => {
    recording();
    const { rerender } = render(<Deck phase="live" />);
    await userEvent.click(screen.getByRole("button", { name: "一時停止" }));
    expect(calls("pause_recording")).toHaveLength(1);
    act(() => useRecording.getState().applyState({ state: "paused", sessionId: "s1" }));
    rerender(<Deck phase="live" />);
    expect(screen.getByText("一時停止中")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "スクショ" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "再開" }));
    expect(calls("resume_recording")).toHaveLength(1);
  });

  it("takes a screenshot", async () => {
    recording();
    render(<Deck phase="live" />);
    await userEvent.click(screen.getByRole("button", { name: "スクショ" }));
    expect(calls("take_screenshot")).toHaveLength(1);
  });

  it("says how far transcription is behind, from two seconds on", () => {
    recording({ lag: { lagMs: 1400, queued: 1, device: "cpu" } });
    render(<Deck phase="live" />);
    expect(screen.queryByText(/遅れ/)).not.toBeInTheDocument();
    act(() => useRecording.getState().setLag({ lagMs: 20_000, queued: 4, device: "cpu" }));
    expect(screen.getByText("遅れ 20秒")).toBeInTheDocument();
  });

  it("while transcription waits for 停止, offers to resume it now", async () => {
    recording({ deferred: true, lag: { lagMs: 60_000, queued: 9, device: "cpu" } });
    render(<Deck phase="live" />);
    expect(screen.getByText(/停止後に文字起こし/)).toBeInTheDocument();
    expect(screen.queryByText(/遅れ/)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "今すぐ再開" }));
    expect(calls("defer_transcription")).toEqual([{ deferred: false }]);
  });

  it("marks the line being said, or says there is none yet", async () => {
    recording();
    answer({ mark_current_line: false });
    render(<Deck phase="live" />);
    await userEvent.click(screen.getByRole("button", { name: "話している行に重要の印" }));
    expect(toasts()).toEqual(["印を付ける行がまだありません"]);
    answer({ mark_current_line: true });
    await userEvent.click(screen.getByRole("button", { name: "話している行に重要の印" }));
    expect(toasts()).toContain("重要の印を付けました");
  });

  it("starts a new part with a cut", async () => {
    recording();
    render(<Deck phase="live" />);
    await userEvent.click(screen.getByRole("button", { name: "区切りを入れる" }));
    expect(calls("add_cut")).toHaveLength(1);
    expect(toasts()).toEqual(["区切りを入れました"]);
  });
});

describe("Deck after a recording", () => {
  beforeEach(resetStores);

  it("while finishing, counts the lines left and can stop", async () => {
    recording();
    useRecording.getState().applyState({ state: "finishing" });
    useRecording.getState().setFinishing({ done: 3, total: 5, doneMs: 3000, totalMs: 5000 });
    render(<Deck phase="live" />);
    expect(screen.getByText("仕上げ中…")).toBeInTheDocument();
    expect(screen.getByText(/残り 2 件/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "処理を中止" }));
    expect(calls("cancel_finishing")).toHaveLength(1);
  });

  it("copies the session and opens the export dialog", async () => {
    ready();
    answer({ copy_transcript: { chars: 120 } });
    render(<Deck phase="review" sessionId="s1" />);
    await userEvent.click(screen.getByRole("button", { name: "コピー" }));
    expect(calls("copy_transcript")).toEqual([{ sessionId: "s1", format: "plain" }]);
    expect(toasts()).toEqual(["コピーしました"]);
    await userEvent.click(screen.getByRole("button", { name: "書き出し" }));
    expect(useUi.getState().exportFor).toBe("s1");
  });

  it("clears the finished session for a new recording", async () => {
    ready();
    useTranscript.setState({ sessionId: "s1" });
    render(<Deck phase="review" sessionId="s1" onNew />);
    await userEvent.click(screen.getByRole("button", { name: "新しい録音" }));
    expect(useTranscript.getState().sessionId).toBeUndefined();
  });

  it("offers playback only when the session kept its sound", () => {
    ready();
    const { unmount } = render(<Deck phase="review" sessionId="s1" />);
    expect(screen.queryByRole("button", { name: "再生" })).not.toBeInTheDocument();
    unmount();
    useTranscript.setState({ sessionId: "s1", folder: "C:\\s1", audio: [{ file: "audio/1.ogg", startMs: 0 }] });
    render(<Deck phase="review" sessionId="s1" />);
    expect(screen.getByRole("button", { name: "再生" })).toBeInTheDocument();
  });
});
