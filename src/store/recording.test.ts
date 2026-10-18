import { beforeEach, describe, expect, it } from "vitest";
import { resetStores } from "../test/fixtures";
import { isBusy, useRecording } from "./recording";

const sources = { ids: ["app", "mic"] as ("app" | "mic")[], appName: "Zoom" };

describe("recording store", () => {
  beforeEach(resetStores);

  it("keeps the session and its sources while finishing, and lets go once ready", () => {
    const st = useRecording.getState();
    st.applyState({ state: "recording", sessionId: "s1", sources, folder: "C:\\s1", elapsedMs: 5000 });
    st.applyState({ state: "finishing" });
    let r = useRecording.getState();
    expect(r.sessionId).toBe("s1");
    expect(r.sources).toEqual(sources);
    expect(r.elapsedMs).toBe(5000);
    st.applyState({ state: "ready" });
    r = useRecording.getState();
    expect(r.sessionId).toBeUndefined();
    expect(r.sources).toBeUndefined();
    expect(r.elapsedMs).toBe(0);
    // The folder stays, for 「フォルダを開く」 after the recording.
    expect(r.folder).toBe("C:\\s1");
  });

  it("keeps the elapsed time through a pause that reports none", () => {
    const st = useRecording.getState();
    st.applyState({ state: "recording", sessionId: "s1", elapsedMs: 12_000 });
    st.applyState({ state: "paused", sessionId: "s1" });
    expect(useRecording.getState().elapsedMs).toBe(12_000);
  });

  it("forgets the lag once the recording ends, and the progress once finished", () => {
    const st = useRecording.getState();
    st.applyState({ state: "recording", sessionId: "s1" });
    st.setLag({ lagMs: 20_000, queued: 3, device: "cpu" });
    st.applyState({ state: "paused", sessionId: "s1" });
    expect(useRecording.getState().lag?.lagMs).toBe(20_000);
    st.applyState({ state: "finishing" });
    expect(useRecording.getState().lag).toBeNull();
    st.setFinishing({ done: 1, total: 4, doneMs: 1000, totalMs: 4000 });
    st.applyState({ state: "ready" });
    expect(useRecording.getState().finishing).toBeNull();
  });

  it("deferred transcription is only on while the backend says so", () => {
    const st = useRecording.getState();
    st.applyState({ state: "recording", sessionId: "s1", deferred: true });
    expect(useRecording.getState().deferred).toBe(true);
    st.applyState({ state: "recording", sessionId: "s1" });
    expect(useRecording.getState().deferred).toBe(false);
  });

  it("counts recording, paused and finishing as busy", () => {
    expect(["needs_model", "ready", "recording", "paused", "finishing"].filter((s) => isBusy(s as never))).toEqual(["recording", "paused", "finishing"]);
  });
});
