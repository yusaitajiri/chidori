import { beforeEach, describe, expect, it } from "vitest";
import { answer, fails } from "../test/tauri";
import { makeApp, makeSettings, resetStores } from "../test/fixtures";
import { useSource } from "./source";

describe("source store", () => {
  beforeEach(resetStores);

  it("starts from the saved source, with the last app not bound to a process yet", () => {
    const s = makeSettings();
    s.source = { mode: "app", includeMic: false, micDeviceId: "usb", app: { exe: "Zoom.exe", name: "Zoom" } };
    useSource.getState().initFrom(s);
    const src = useSource.getState();
    expect(src.mode).toBe("app");
    expect(src.includeMic).toBe(false);
    expect(src.micDeviceId).toBe("usb");
    expect(src.app).toEqual({ exe: "Zoom.exe", name: "Zoom", rootPid: 0 });
  });

  it("binds the last-used app to its running process, whatever the case of its name", async () => {
    answer({ list_audio_apps: [makeApp(4242, "Zoom", { exe: "zoom.EXE", iconDataUrl: "data:icon" })] });
    useSource.setState({ mode: "app", app: { exe: "Zoom.exe", name: "Zoom", rootPid: 0 } });
    await useSource.getState().refreshApps();
    expect(useSource.getState().app).toEqual({ exe: "zoom.EXE", name: "Zoom", rootPid: 4242, iconDataUrl: "data:icon" });
  });

  it("keeps the chosen app when it is not running", async () => {
    answer({ list_audio_apps: [makeApp(1, "Teams")] });
    useSource.setState({ mode: "app", app: { exe: "Zoom.exe", name: "Zoom", rootPid: 0 } });
    await useSource.getState().refreshApps();
    expect(useSource.getState().app?.exe).toBe("Zoom.exe");
    expect(useSource.getState().apps).toHaveLength(1);
  });

  it("stops showing it is loading when the list fails", async () => {
    answer({ list_audio_apps: fails("E_INTERNAL") });
    await expect(useSource.getState().refreshApps()).rejects.toMatchObject({ code: "E_INTERNAL" });
    expect(useSource.getState().loadingApps).toBe(false);
  });

  it("asks for the mic as a second source only when 相手 is not the mic itself", () => {
    const st = useSource.getState();
    st.setMode("mic");
    expect(useSource.getState().config()).toEqual({ mode: "mic", app: undefined, includeMic: false, micDeviceId: undefined });
    st.setMode("system");
    expect(useSource.getState().config().includeMic).toBe(true);
  });

  it("names the app only when recording an app", () => {
    const st = useSource.getState();
    st.setApp(makeApp(7, "Zoom"));
    st.setMicDevice("usb");
    expect(useSource.getState().config()).toEqual({ mode: "system", app: undefined, includeMic: true, micDeviceId: "usb" });
    st.setMode("app");
    expect(useSource.getState().config().app).toEqual({ exe: "Zoom.exe", rootPid: 7, name: "Zoom" });
  });
});
