import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ScreenshotPayload, Segment, SourceId } from "../ipc/types";
import { createTranscriptStore, TranscriptContext, type TranscriptStore } from "../store/transcript";
import { useRecording } from "../store/recording";
import { useUi } from "../store/ui";
import { resetStores } from "../test/fixtures";
import { answer, calls, fails } from "../test/tauri";
import { scrollToIndex } from "../test/virtual";
import { Transcript, TranscriptTail } from "./LiveTranscript";

vi.mock("@tanstack/react-virtual", () => import("../test/virtual"));

const seg = (id: string, tStartMs: number, text: string, source: SourceId = "app", over: Partial<Segment> = {}): Segment => ({
  id,
  utteranceId: `u-${id}`,
  source,
  tStartMs,
  tEndMs: tStartMs + 2000,
  text,
  edited: false,
  ...over,
});

const shot = (id: string, tMs: number): ScreenshotPayload => ({ id, tMs, thumbDataUrl: "data:image/png;base64,", width: 10, height: 10, file: `images/${id}.png` });

let store: TranscriptStore;

function show(ui = <Transcript editable={false} live />) {
  return { user: userEvent.setup(), ...render(<TranscriptContext.Provider value={store}>{ui}</TranscriptContext.Provider>) };
}

/** The text of each line and picture, top to bottom. */
const lines = () => screen.getAllByText((_, el) => el?.tagName === "P" || el?.tagName === "IMG").map((el) => (el.tagName === "IMG" ? "[img]" : el.textContent));

describe("Transcript", () => {
  beforeEach(() => {
    resetStores();
    scrollToIndex.mockClear();
    store = createTranscriptStore();
    store.setState({ sessionId: "s1", sources: [{ id: "app", label: "Zoom" }, { id: "mic", label: "自分" }] });
  });

  it("waits for speech while live, and says a finished session has none", () => {
    const { unmount } = show();
    expect(screen.getByText("話し声を待っています…")).toBeInTheDocument();
    unmount();
    show(<Transcript editable />);
    expect(screen.getByText("文字起こしはありません")).toBeInTheDocument();
  });

  it("says the words are being written when speech waits for transcription, unless grey words show it", () => {
    show();
    const lag = (lagMs: number) => act(() => useRecording.setState({ state: "recording", lag: { lagMs, queued: 1, device: "cpu" } }));
    // A line a fast GPU writes in time shows nothing.
    lag(800);
    expect(screen.getByText("話し声を待っています…")).toBeInTheDocument();
    lag(4000);
    expect(screen.getByRole("status")).toHaveTextContent("文字起こし中…");
    act(() => store.getState().addSegment(seg("a", 1000, "一行目")));
    expect(screen.getByRole("status")).toHaveTextContent("文字起こし中…");
    act(() => store.getState().setPartial({ source: "app", utteranceId: "u-next", text: "えっと", tStartMs: 4000 }));
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    // Waiting for Stop on purpose is not being written.
    act(() => {
      store.getState().setPartial({ source: "app", utteranceId: "u-next", text: "", tStartMs: 4000 });
      useRecording.setState({ deferred: true });
    });
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("puts screenshots and late lines where they belong in time, with the words still coming last", () => {
    show();
    act(() => {
      const st = store.getState();
      st.addSegment(seg("b", 6000, "二行目"));
      st.addScreenshot(shot("img1", 3000));
      st.addSegment(seg("a", 1000, "一行目"));
      st.setPartial({ source: "mic", utteranceId: "u-next", text: "えっと", tStartMs: 9000 });
    });
    expect(lines()).toEqual(["一行目", "[img]", "二行目", "えっと"]);
    expect(screen.getByText("えっと")).toHaveClass("text-partial");
  });

  it("darkens the words in place when their final line arrives", () => {
    show();
    act(() => store.getState().setPartial({ source: "app", utteranceId: "u-a", text: "こんにち", tStartMs: 0 }));
    expect(screen.getByText("こんにち")).toHaveClass("text-partial");
    act(() => store.getState().addSegment(seg("a", 0, "こんにちは")));
    expect(screen.queryByText("こんにち")).not.toBeInTheDocument();
    expect(screen.getByText("こんにちは")).not.toHaveClass("text-partial");
  });

  it("names the speaker at each turn, only when both sides are recorded", () => {
    show();
    act(() => {
      const st = store.getState();
      st.addSegment(seg("a", 0, "どうも"));
      st.addSegment(seg("b", 3000, "よろしく"));
      st.addSegment(seg("c", 6000, "はい", "mic"));
      // A minute on, the same speaker starts a new turn.
      st.addSegment(seg("d", 70_000, "では", "mic"));
    });
    expect(screen.getAllByText("相手")).toHaveLength(1);
    expect(screen.getAllByText("自分")).toHaveLength(2);
    act(() => store.setState({ sources: [{ id: "app", label: "Zoom" }] }));
    expect(screen.queryByText("相手")).not.toBeInTheDocument();
  });

  it("shows markers as rules with what happened", () => {
    store.setState({
      items: [
        { kind: "marker", id: "m1", tMs: 1000, type: "cut" },
        { kind: "marker", id: "m2", tMs: 2000, type: "unprocessed", detail: "12" },
      ],
    });
    show(<Transcript editable />);
    expect(screen.getByText("区切り")).toBeInTheDocument();
    expect(screen.getByText("以降、未処理の音声 12 秒")).toBeInTheDocument();
  });

  it("edits a line and keeps what was saved", async () => {
    answer({ update_segment: ({ text }: { text: string }) => seg("a", 0, text, "app", { edited: true }) });
    store.getState().addSegment(seg("a", 0, "こんにちわ"));
    const { user } = show(<Transcript editable />);
    await user.click(screen.getByRole("button", { name: "行を編集" }));
    const box = screen.getByRole("textbox", { name: "行を編集" });
    await user.clear(box);
    await user.type(box, "こんにちは");
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(calls("update_segment")).toEqual([{ sessionId: "s1", segmentId: "a", text: "こんにちは" }]);
    expect(screen.getByText("こんにちは")).toBeInTheDocument();
    expect(screen.getByText("編集済み")).toBeInTheDocument();
  });

  it("cancels an edit with Escape", async () => {
    store.getState().addSegment(seg("a", 0, "そのまま"));
    const { user } = show(<Transcript editable />);
    await user.click(screen.getByRole("button", { name: "行を編集" }));
    await user.type(screen.getByRole("textbox"), "追加{Escape}");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.getByText("そのまま")).toBeInTheDocument();
    expect(calls("update_segment")).toEqual([]);
  });

  it("says when a change could not be saved", async () => {
    answer({ delete_segment: fails("E_INTERNAL", "locked") });
    store.getState().addSegment(seg("a", 0, "消せない"));
    const { user } = show(<Transcript editable />);
    await user.click(screen.getByRole("button", { name: "行を削除" }));
    expect(screen.getByText("消せない")).toBeInTheDocument();
    expect(useUi.getState().toasts.map((t) => t.text)).toEqual(["保存に失敗しました"]);
  });

  it("deletes a line", async () => {
    store.getState().addSegment(seg("a", 0, "消す行"));
    const { user } = show(<Transcript editable />);
    await user.click(screen.getByRole("button", { name: "行を削除" }));
    expect(calls("delete_segment")).toEqual([{ sessionId: "s1", segmentId: "a" }]);
    expect(screen.queryByText("消す行")).not.toBeInTheDocument();
  });

  it("marks a line important, even live, but edits only a finished session", async () => {
    answer({ mark_segment: ({ important }: { important: boolean }) => seg("a", 0, "大事", "app", { important }) });
    store.getState().addSegment(seg("a", 0, "大事"));
    const { user } = show(<Transcript editable={false} live />);
    expect(screen.queryByRole("button", { name: "行を編集" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "重要の印を付ける" }));
    expect(calls("mark_segment")).toEqual([{ sessionId: "s1", segmentId: "a", important: true }]);
    expect(screen.getByRole("button", { name: "重要の印を外す" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("大事").closest("p")).toHaveTextContent("重要: 大事");
  });

  it("captions a screenshot", async () => {
    store.getState().addScreenshot(shot("img1", 1000));
    const { user } = show(<Transcript editable />);
    await user.click(screen.getByRole("button", { name: "キャプションを追加" }));
    await user.type(screen.getByRole("textbox", { name: "キャプション" }), "ホワイトボード {Enter}");
    expect(calls("set_caption")).toEqual([{ sessionId: "s1", id: "img1", caption: "ホワイトボード " }]);
    expect(screen.getByText("ホワイトボード")).toBeInTheDocument();
    expect(screen.getByRole("img")).toHaveAccessibleName("ホワイトボード");
  });

  it("plays from a line only in a finished session that kept its sound", async () => {
    store.setState({ audio: [{ file: "audio/1.ogg", startMs: 0 }] });
    store.getState().addSegment(seg("a", 4000, "ここから"));
    const { user, unmount } = show(<Transcript editable live />);
    expect(screen.queryByRole("button", { name: "ここから再生" })).not.toBeInTheDocument();
    unmount();
    show(<Transcript editable />);
    await user.click(screen.getByRole("button", { name: "ここから再生" }));
    expect(store.getState().play?.ms).toBe(4000);
  });

  it("brings the moment pointed at on the session's line into view", () => {
    store.setState({ items: [0, 10_000, 20_000, 30_000].map((t, i) => ({ kind: "segment" as const, ...seg(`s${i}`, t, `行${i}`) })) });
    show(<Transcript editable header={<h1>見出し</h1>} />);
    act(() => store.getState().seekTo(15_000));
    // Row 0 is the header; 20 s is the first line at or after 15 s.
    expect(scrollToIndex).toHaveBeenLastCalledWith(3, { align: "center" });
  });
});

describe("TranscriptTail", () => {
  beforeEach(() => {
    resetStores();
    store = createTranscriptStore();
    store.setState({ sessionId: "s1", sources: [{ id: "app", label: "Zoom" }, { id: "mic", label: "自分" }] });
  });

  it("shows the last two lines, the words still coming included, markers left out", () => {
    show(<TranscriptTail />);
    expect(screen.getByText("話し声を待っています…")).toBeInTheDocument();
    act(() => {
      const st = store.getState();
      st.addSegment(seg("a", 0, "一つ目"));
      st.addSegment(seg("b", 2000, "二つ目", "mic"));
      st.addMarker({ kind: "marker", id: "m", tMs: 3000, type: "cut" });
      st.setPartial({ source: "app", utteranceId: "u-c", text: "三つ", tStartMs: 4000 });
    });
    const tail = screen.getByText("二つ目").closest("[aria-live]") as HTMLElement;
    expect(within(tail).queryByText("一つ目")).not.toBeInTheDocument();
    expect(within(tail).getByText("三つ")).toBeInTheDocument();
    expect(within(tail).getByText("自分")).toBeInTheDocument();
  });
});
