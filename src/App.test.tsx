// The whole app against a stand-in backend: what a person does from 開始 to the saved session,
// and playing it back.

import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { useTranscript } from "./store/transcript";
import { useUi } from "./store/ui";
import { FakeBackend, FOLDER, SESSION_ID } from "./test/backend";
import { resetStores } from "./test/fixtures";
import { fakeAudio } from "./test/media";
import { answer, calls, dragDrop, emit, fails } from "./test/tauri";

vi.mock("@tanstack/react-virtual", () => import("./test/virtual"));

let backend: FakeBackend;

async function launch() {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("button", { name: "開始" });
  return user;
}

/** Records two lines from 相手 and one from 自分, with a screenshot, and waits for the saved session. */
async function recordAndSave(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "開始" }));
  await screen.findByText("話し声を待っています…");
  backend.hear("system", "今日の議題です", 0);
  backend.hear("mic", "はい", 3000);
  backend.screenshot("0001", 4000);
  backend.hear("system", "まず予算から", 5000);
  await user.click(screen.getByRole("button", { name: "停止" }));
  backend.save(10_000);
  await screen.findByRole("button", { name: "新しい録音" });
  // The saved session has been read back (its sound, its folder).
  await waitFor(() => expect(useTranscript.getState().folder).toBe(FOLDER));
}

const toast = () => screen.queryByRole("status");
/** The title bar, which names the recording's state too. */
const titleBar = () => screen.getByRole("banner");
const deck = () => document.querySelector<HTMLElement>("[data-deck]") as HTMLElement;
/** The row of the transcript that holds a line. */
const rowOf = (text: string) => screen.getByText(text).closest("[data-key]") as HTMLElement;

beforeEach(() => {
  resetStores();
  backend = new FakeBackend();
});

afterEach(() => {
  // Unmounted first, so leaving a session still pauses the stand-in audio.
  cleanup();
  vi.restoreAllMocks();
});

describe("a recording, from 開始 to the saved session", () => {
  it("transcribes as it hears, finishes, and opens the saved session", async () => {
    const user = await launch();
    await user.click(screen.getByRole("button", { name: "開始" }));
    expect(backend.startedWith).toMatchObject({ mode: "system", includeMic: true });

    // The recording's view: its title, and the line waiting for words.
    expect(await screen.findByText("システム全体 2026-10-18 19:00")).toBeInTheDocument();
    expect(screen.getByText("話し声を待っています…")).toBeInTheDocument();

    // Grey words first, then the final line in their place.
    backend.partial("system", "今日の", 0);
    expect(screen.getByText("今日の")).toHaveClass("text-partial");
    backend.hear("system", "今日の議題です", 0);
    expect(screen.queryByText("今日の")).not.toBeInTheDocument();
    backend.hear("mic", "はい", 3000);
    backend.screenshot("0001", 4000);
    const log = screen.getByRole("log");
    expect(within(log).getByText("相手")).toBeInTheDocument();
    expect(within(log).getByText("自分")).toBeInTheDocument();
    expect(within(log).getByRole("img")).toBeInTheDocument();

    // 停止: the queue finishes, then the session is saved.
    await user.click(screen.getByRole("button", { name: "停止" }));
    expect(await within(deck()).findByText("仕上げ中…")).toBeInTheDocument();
    expect(within(titleBar()).getByText("仕上げ中…")).toBeInTheDocument();
    backend.progress(1, 2);
    expect(within(deck()).getByText(/残り 1 件/)).toBeInTheDocument();
    backend.save(10_000);
    expect(await screen.findByRole("button", { name: "新しい録音" })).toBeInTheDocument();
    expect(toast()).toHaveTextContent("保存しました");
    expect(screen.queryByRole("button", { name: "停止" })).not.toBeInTheDocument();

    // The saved session: its numbers, its lines, and where it was saved.
    expect(await screen.findByText("行")).toHaveTextContent("2");
    expect(screen.getByText("枚のスクショ")).toHaveTextContent("1");
    expect(screen.getByText("今日の議題です")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "詳細" }));
    expect(screen.getByRole("button", { name: `Chidori › ${SESSION_ID}` })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "コピー" }));
    expect(calls("copy_transcript")).toEqual([{ sessionId: SESSION_ID, format: "plain" }]);
  });

  it("goes back to the start for a new recording", async () => {
    const user = await launch();
    await recordAndSave(user);
    await user.click(screen.getByRole("button", { name: "新しい録音" }));
    expect(await screen.findByRole("button", { name: "開始" })).toBeEnabled();
    expect(screen.queryByText("今日の議題です")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^ソースを選ぶ:/ })).toBeEnabled();
  });

  it("keeps events that are no longer live out of the saved session", async () => {
    const user = await launch();
    await recordAndSave(user);
    act(() => emit("transcript://segment", { id: "seg_late", source: "system", tStartMs: 9000, tEndMs: 9500, text: "遅れた行", edited: false }));
    expect(screen.queryByText("遅れた行")).not.toBeInTheDocument();
  });

  it("pauses and resumes", async () => {
    const user = await launch();
    await user.click(screen.getByRole("button", { name: "開始" }));
    await user.click(await screen.findByRole("button", { name: "一時停止" }));
    expect(within(deck()).getByText("一時停止中")).toBeInTheDocument();
    expect(within(titleBar()).getByText("一時停止中")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "スクショ" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "再開" }));
    expect(screen.queryByText("一時停止中")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "スクショ" })).toBeEnabled();
  });

  it("switches what is recorded without stopping", async () => {
    answer({ ...backend.commands(), switch_source: undefined });
    const user = await launch();
    await user.click(screen.getByRole("button", { name: "開始" }));
    // The recording first takes the source from the settings, then follows the picker.
    await waitFor(() => expect(calls("get_settings")).toHaveLength(2));
    await user.click(await screen.findByRole("button", { name: "ソースを選ぶ: システム全体" }));
    await user.click(within(screen.getByRole("listbox", { name: "ソースを切り替える" })).getByRole("option", { name: "マイクだけ" }));
    await waitFor(() => expect(calls("switch_source")).toEqual([{ source: { mode: "mic", app: undefined, includeMic: false, micDeviceId: undefined } }]));
    expect(await screen.findByText("ソースを切り替えました")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "停止" })).toBeInTheDocument();
  });

  it("shows why a recording could not start, with the way to fix it", async () => {
    answer({ ...backend.commands(), start_recording: fails("E_MIC_DENIED") });
    const user = await launch();
    await user.click(screen.getByRole("button", { name: "開始" }));
    const banner = await screen.findByRole("alert");
    expect(banner).toHaveTextContent("マイクへのアクセスがオフです");
    await user.click(within(banner).getByRole("button", { name: "設定を開く" }));
    expect(calls("open_mic_privacy")).toHaveLength(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "開始" })).toBeEnabled();
  });

  it("picks up a recording that was already running when the window opened", async () => {
    backend.recordingAtLaunch([
      ["system", "さっきの話ですが", 1000],
      ["mic", "はい", 4000],
    ]);
    render(<App />);
    expect(await screen.findByText("さっきの話ですが")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "停止" })).toBeInTheDocument();
    expect(screen.getByLabelText("経過時間")).toHaveTextContent("01:30");
    // New lines join the ones already there.
    backend.hear("system", "続けます", 8000);
    expect(screen.getByText("続けます")).toBeInTheDocument();
  });
});

describe("transcribing a file", () => {
  it("transcribes a file picked under 開始, with how far into it the text has got", async () => {
    const user = await launch();
    await user.click(screen.getByRole("button", { name: "ファイルから文字起こし" }));
    expect(calls("transcribe_file")).toEqual([{ path: undefined }]);

    // It finishes from the start: no recording's controls, the place in the file instead.
    expect(await within(deck()).findByText("文字起こし中")).toBeInTheDocument();
    expect(within(titleBar()).getByText("lecture.mp4")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "停止" })).not.toBeInTheDocument();
    act(() => emit("finishing://progress", { done: 0, total: 0, doneMs: 754_000, totalMs: 3_600_000 }));
    expect(within(deck()).getByText(/12:34 \/ 1:00:00/)).toBeInTheDocument();
    expect(within(deck()).getByRole("progressbar")).toBeInTheDocument();

    // Its times are places in the file, not clock times.
    backend.hear("file", "では本題に入ります", 754_000);
    expect(within(rowOf("では本題に入ります")).getByText(/00:12:34/)).toBeInTheDocument();
    backend.save(3_600_000);
    expect(await screen.findByRole("button", { name: "新しい録音" })).toBeInTheDocument();
  });

  it("does nothing when the dialog is cancelled", async () => {
    backend.picked = null;
    const user = await launch();
    await user.click(screen.getByRole("button", { name: "ファイルから文字起こし" }));
    expect(calls("transcribe_file")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "開始" })).toBeEnabled();
  });

  it("transcribes a file dropped on the window", async () => {
    await launch();
    act(() => dragDrop({ type: "enter", paths: ["C:\\rec\\memo.m4a"] }));
    expect(screen.getByText("ここにドロップして文字起こし")).toBeInTheDocument();
    act(() => dragDrop({ type: "drop", paths: ["C:\\rec\\memo.m4a"] }));
    expect(screen.queryByText("ここにドロップして文字起こし")).not.toBeInTheDocument();
    await waitFor(() => expect(calls("transcribe_file")).toEqual([{ path: "C:\\rec\\memo.m4a" }]));
    expect(await within(titleBar()).findByText("memo.m4a")).toBeInTheDocument();
  });

  it("says why a file can't be transcribed", async () => {
    answer({ ...backend.commands(), transcribe_file: fails("E_FILE_UNREADABLE", "noAudio") });
    const user = await launch();
    await user.click(screen.getByRole("button", { name: "ファイルから文字起こし" }));
    expect(await screen.findByText("このファイルには音声がありません")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "開始" })).toBeEnabled();
  });
});

describe("playing a saved session", () => {
  it("plays its sound, follows it in the transcript, and pauses", async () => {
    const audio = fakeAudio();
    const user = await launch();
    await recordAndSave(user);
    await user.click(screen.getByRole("button", { name: "再生" }));
    expect(audio.src).toBe(`asset://localhost/${encodeURIComponent(`${FOLDER}\\audio\\0001.ogg`)}`);
    audio.loaded();
    expect(audio.playing).toBe(true);
    const pause = screen.getByRole("button", { name: "再生を一時停止" });
    expect(pause).toHaveAttribute("aria-pressed", "true");

    // Six seconds in, the line that started at five is the one being played.
    audio.at(6);
    expect(rowOf("まず予算から")).toHaveClass("bg-surface-2");
    expect(rowOf("今日の議題です")).not.toHaveClass("bg-surface-2");
    expect(document.querySelector(".kk-playhead")).toHaveStyle({ left: "60%" });

    await user.click(pause);
    expect(audio.playing).toBe(false);
    expect(screen.getByRole("button", { name: "再生" })).toHaveAttribute("aria-pressed", "false");
  });

  it("plays from the line chosen", async () => {
    const audio = fakeAudio();
    const user = await launch();
    await recordAndSave(user);
    await user.click(within(rowOf("まず予算から")).getByRole("button", { name: "ここから再生" }));
    audio.loaded();
    expect(audio.seconds).toBe(5);
    expect(audio.playing).toBe(true);
  });

  it("runs on from one recording into the next in a session recorded in parts", async () => {
    const audio = fakeAudio();
    const user = await launch();
    await user.click(screen.getByRole("button", { name: "開始" }));
    backend.hear("system", "一回目", 0);
    backend.hear("system", "二回目", 61_000);
    await user.click(await screen.findByRole("button", { name: "停止" }));
    backend.save(120_000, [
      { file: "audio/0001.ogg", startMs: 0 },
      { file: "audio/0002.ogg", startMs: 60_000 },
    ]);
    await waitFor(() => expect(useTranscript.getState().audio).toHaveLength(2));
    await user.click(screen.getByRole("button", { name: "再生" }));
    audio.loaded();
    audio.end();
    expect(audio.src).toContain(encodeURIComponent("audio\\0002.ogg"));
    audio.loaded();
    audio.at(2);
    expect(rowOf("二回目")).toHaveClass("bg-surface-2");
  });

  it("stops when the session is left", async () => {
    const audio = fakeAudio();
    const user = await launch();
    await recordAndSave(user);
    await user.click(screen.getByRole("button", { name: "再生" }));
    audio.loaded();
    await user.click(screen.getByRole("button", { name: "新しい録音" }));
    expect(audio.playing).toBe(false);
    expect(useTranscript.getState().playingMs).toBeNull();
  });

  it("says when the sound file cannot be opened", async () => {
    const audio = fakeAudio();
    const user = await launch();
    await recordAndSave(user);
    await user.click(screen.getByRole("button", { name: "再生" }));
    audio.fail();
    expect(await screen.findByText("音声ファイルを開けません")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "再生" })).toBeInTheDocument();
    expect(document.querySelector(".kk-playhead")).toBeNull();
  });

  it("tries the file again, and says so again, on every press", async () => {
    const audio = fakeAudio();
    const user = await launch();
    await recordAndSave(user);
    await user.click(screen.getByRole("button", { name: "再生" }));
    audio.fail();
    await screen.findByText("音声ファイルを開けません");
    act(() => useUi.setState({ toasts: [] }));
    await user.click(screen.getByRole("button", { name: "再生" }));
    // It waits for the file to load again rather than playing the element that failed.
    expect(audio.playing).toBe(false);
    audio.fail();
    expect(await screen.findByText("音声ファイルを開けません")).toBeInTheDocument();
  });

  it("has nothing to play when the sound was not kept", async () => {
    backend.settings.audio.record = false;
    const user = await launch();
    await recordAndSave(user);
    expect(screen.queryByRole("button", { name: "再生" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "ここから再生" })).not.toBeInTheDocument();
  });
});
