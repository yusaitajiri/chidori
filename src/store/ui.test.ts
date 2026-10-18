import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetStores } from "../test/fixtures";
import { useUi } from "./ui";

const notice = (code: string, message = code) => ({ level: "warn" as const, code, message });

describe("ui store", () => {
  beforeEach(resetStores);
  afterEach(() => vi.useRealTimers());

  it("opens 設定 at its list unless a section is asked for", () => {
    const ui = useUi.getState();
    ui.go("settings", { tab: "hotkeys" });
    expect(useUi.getState().settingsTab).toBe("hotkeys");
    ui.go("main");
    expect(useUi.getState().settingsTab).toBe("hotkeys");
    ui.go("settings");
    expect(useUi.getState().settingsTab).toBe("index");
  });

  it("remembers the session being viewed across other pages", () => {
    const ui = useUi.getState();
    ui.go("session", { sessionId: "s1" });
    ui.go("history");
    expect(useUi.getState().viewingSessionId).toBe("s1");
  });

  it("shows one banner per code, the newest replacing an older one, three at most", () => {
    const ui = useUi.getState();
    ui.pushBanner(notice("a", "first"));
    ui.pushBanner(notice("b"));
    ui.pushBanner(notice("a", "second"));
    expect(useUi.getState().banners.map((b) => b.message)).toEqual(["b", "second"]);
    ui.pushBanner(notice("c"));
    ui.pushBanner(notice("d"));
    expect(useUi.getState().banners.map((b) => b.code)).toEqual(["a", "c", "d"]);
    ui.dismissBanner(useUi.getState().banners[0].id);
    expect(useUi.getState().banners.map((b) => b.code)).toEqual(["c", "d"]);
  });

  it("shows three toasts at most, each for two seconds", () => {
    vi.useFakeTimers();
    const ui = useUi.getState();
    for (const text of ["1", "2", "3", "4"]) ui.toast(text);
    expect(useUi.getState().toasts.map((t) => t.text)).toEqual(["2", "3", "4"]);
    vi.advanceTimersByTime(2000);
    expect(useUi.getState().toasts).toEqual([]);
  });

  it("opens the new-project dialog with the sessions to put in it, and closes it", () => {
    const ui = useUi.getState();
    ui.askNewProject(["s1", "s2"]);
    expect(useUi.getState().newProject?.sessionIds).toEqual(["s1", "s2"]);
    ui.askNewProject();
    expect(useUi.getState().newProject).toBeUndefined();
  });
});
