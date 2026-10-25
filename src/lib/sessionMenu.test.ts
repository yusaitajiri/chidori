import { beforeEach, describe, expect, it } from "vitest";
import { useRecording } from "../store/recording";
import { useTranscript } from "../store/transcript";
import { useUi } from "../store/ui";
import { resetStores } from "../test/fixtures";
import { answer, calls } from "../test/tauri";
import { deleteSessions } from "./actions";
import type { MenuEntry } from "./popupMenu";
import { copyEntries, sessionEntries } from "./sessionMenu";

const texts = (entries: MenuEntry[]) => entries.map((e) => (e === "separator" ? "─" : e.text));
const pick = (entries: MenuEntry[], text: string) => {
  const e = entries.find((x) => x !== "separator" && x.text === text);
  if (!e || e === "separator" || !("action" in e)) throw new Error(`no ${text}`);
  e.action();
};

const session = { id: "s1", title: "Zoom", folder: "C:\\Chidori\\s1" };

describe("the session menu", () => {
  beforeEach(() => {
    resetStores();
    useRecording.setState({ state: "ready" });
  });

  it("holds the same actions in an open session and a row, the row adding what a deck and header hold", () => {
    expect(texts(sessionEntries(session, false))).toEqual(["続きを録音", "─", "フォルダを開く", "─", "削除"]);
    expect(texts(sessionEntries(session, true))).toEqual(["続きを録音", "─", "コピー", "書き出し…", "─", "名前を変更", "プロジェクト", "フォルダを開く", "─", "削除"]);
  });

  it("offers 復元 first for a session that ended in a crash, and nothing to record onto it", () => {
    const entries = sessionEntries({ ...session, recoverable: true }, true);
    expect(texts(entries).slice(0, 2)).toEqual(["復元", "─"]);
    expect(entries[2]).toMatchObject({ text: "続きを録音", enabled: false });
  });

  it("asks before deleting, naming the session", () => {
    pick(sessionEntries(session, false), "削除");
    expect(useUi.getState().deleting).toMatchObject({ ids: ["s1"], title: "Zoom" });
  });

  it("copies each format", async () => {
    answer({ copy_transcript: { chars: 10 } });
    expect(texts(copyEntries("s1", false))).toEqual(["Agent用にコピー", "Markdownでコピー"]);
    for (const text of texts(copyEntries("s1", true))) pick(copyEntries("s1", true), text);
    await Promise.resolve();
    expect(calls("copy_transcript").map((c) => c.format)).toEqual(["plain", "agent", "markdown"]);
  });
});

describe("deleting sessions", () => {
  beforeEach(() => {
    resetStores();
    useRecording.setState({ state: "ready" });
  });

  it("takes the deleted session off the main view", async () => {
    useTranscript.setState({ sessionId: "s1" });
    await deleteSessions(["s1"]);
    expect(calls("delete_session")).toEqual([{ sessionId: "s1" }]);
    expect(useTranscript.getState().sessionId).toBeUndefined();
  });

  it("goes back to 履歴 from the past session it deleted", async () => {
    answer({ delete_sessions: { deleted: 2 } });
    useUi.getState().go("session", { sessionId: "s2" });
    await deleteSessions(["s1", "s2"]);
    expect(useUi.getState().view).toBe("history");
    expect(useUi.getState().toasts.map((t) => t.text)).toEqual(["2 件をごみ箱に移動しました"]);
  });
});
