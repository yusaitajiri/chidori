import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { useRecording } from "../store/recording";
import { useSettings } from "../store/settings";
import { useSource } from "../store/source";
import { makeApp, makeInfo, makeSettings, merge, resetStores } from "../test/fixtures";
import { answer, calls } from "../test/tauri";
import { SourcePicker } from "./SourcePicker";

const zoom = makeApp(100, "Zoom");
const notepad = makeApp(200, "メモ帳", { exe: "notepad.exe", hasSession: false, active: false });
const usb = { id: "usb", name: "USB Mic", isDefault: false };

function setup() {
  let current = makeSettings();
  answer({
    list_audio_apps: [notepad, zoom],
    list_mic_devices: [usb],
    set_settings: ({ partial }: { partial: object }) => (current = merge(current, partial)),
  });
  useSettings.setState({ settings: current, info: makeInfo() });
  return { user: userEvent.setup(), ...render(<SourcePicker />) };
}

const field = () => screen.getByRole("button", { name: /^ソースを選ぶ:/ });

describe("SourcePicker", () => {
  beforeEach(resetStores);

  it("lists the whole system and the mic first, then the apps playing sound, then the rest", async () => {
    const { user } = setup();
    await user.click(field());
    const list = screen.getByRole("listbox", { name: "ソースを選ぶ" });
    const options = within(list).getAllByRole("option").map((o) => o.textContent);
    expect(options).toEqual(["システム全体", "マイクだけ", "Zoom", "メモ帳"]);
    expect(within(list).getByText("その他")).toBeInTheDocument();
    expect(within(list).getByRole("option", { name: "システム全体" })).toHaveAttribute("aria-selected", "true");
  });

  it("records the app picked and names it on the field", async () => {
    const { user } = setup();
    await user.click(field());
    await user.click(await screen.findByRole("option", { name: "Zoom" }));
    expect(useSource.getState().mode).toBe("app");
    expect(useSource.getState().app).toMatchObject({ exe: "Zoom.exe", rootPid: 100 });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(field()).toHaveAccessibleName("ソースを選ぶ: Zoom");
    expect(field()).toHaveFocus();
  });

  it("asks for an app until one is picked", () => {
    useSource.setState({ mode: "app", app: null });
    setup();
    expect(field()).toHaveAccessibleName("ソースを選ぶ: アプリを選んでください");
  });

  it("offers apps only on Windows 11", async () => {
    const { user } = setup();
    useSettings.setState({ info: makeInfo({ appLoopbackSupported: false }) });
    await user.click(field());
    const option = await screen.findByRole("option", { name: "Zoom" });
    expect(option).toBeDisabled();
    expect(option).toHaveAttribute("title", "この機能には Windows 11 が必要です");
  });

  it("says when no app was found", async () => {
    const { user } = setup();
    answer({ list_audio_apps: [], list_mic_devices: [] });
    await user.click(field());
    expect(await screen.findByText("アプリが見つかりません")).toBeInTheDocument();
  });

  it("closes the list with Escape and gives focus back to the field", async () => {
    const { user } = setup();
    await user.click(field());
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(field()).toHaveFocus();
  });

  it("records a conversation (the mic too) unless it is switched off", async () => {
    const { user } = setup();
    const conversation = screen.getByRole("switch", { name: "会話として録音" });
    expect(conversation).toHaveAttribute("aria-checked", "true");
    await user.click(conversation);
    expect(useSource.getState().includeMic).toBe(false);
    expect(conversation).toHaveAttribute("aria-checked", "false");
  });

  it("with the mic alone there is no conversation to switch, only which mic", async () => {
    const { user } = setup();
    await user.click(field());
    await user.click(screen.getByRole("option", { name: "マイクだけ" }));
    expect(screen.queryByRole("switch", { name: "会話として録音" })).not.toBeInTheDocument();
    expect(screen.getByText("マイク")).toBeInTheDocument();
  });

  it("saves the mic picked", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "マイクを選ぶ" }));
    await user.click(await screen.findByRole("option", { name: "USB Mic" }));
    expect(useSource.getState().micDeviceId).toBe("usb");
    expect(calls("set_settings")).toEqual([{ partial: { source: { micDeviceId: "usb" } } }]);
    expect(screen.getByText("USB Mic")).toBeInTheDocument();
  });

  it("is locked while recording", () => {
    setup();
    act(() => useRecording.setState({ state: "recording" }));
    expect(field()).toBeDisabled();
    expect(screen.getByRole("switch", { name: "会話として録音" })).toBeDisabled();
  });
});
