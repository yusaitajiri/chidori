import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import i18n from "../i18n";
import type { Settings as SettingsT } from "../ipc/types";
import { useSettings } from "../store/settings";
import { useSource } from "../store/source";
import { type SettingsTab, useUi } from "../store/ui";
import { makeInfo, makeModel, makeSettings, merge, resetStores } from "../test/fixtures";
import { answer, calls, fails } from "../test/tauri";
import { Settings } from "./Settings";

const usb = { id: "usb", name: "USB Mic", isDefault: false };

function setup(tab: SettingsTab, over: (s: SettingsT) => void = () => {}) {
  const s = makeSettings();
  over(s);
  let current = s;
  answer({
    set_settings: ({ partial }: { partial: object }) => (current = merge(current, partial)),
    get_app_info: makeInfo(),
    list_mic_devices: [usb],
    list_projects: [],
    pick_folder: "D:\\Notes",
  });
  useSettings.setState({ settings: s, info: makeInfo(), models: [makeModel("turbo-q5", { nameJa: "標準 · large-v3-turbo q5_0" })] });
  useUi.setState({ view: "settings", settingsTab: tab });
  return { user: userEvent.setup(), ...render(<Settings />) };
}

const saved = () => calls("set_settings").map((c) => c.partial);
const sectionRow = (name: string) => screen.getByRole("button", { name: new RegExp(`^${name}`) });

describe("Settings list", () => {
  beforeEach(resetStores);

  it("lists each section with what it is set to now", async () => {
    useSource.setState({ devices: [usb] });
    setup("index", (s) => (s.source.micDeviceId = "usb"));
    expect(sectionRow("一般")).toHaveTextContent("日本語");
    expect(sectionRow("音声")).toHaveTextContent("USB Mic · エコー対策");
    expect(sectionRow("文字起こし")).toHaveTextContent("日本語 · GPU");
    expect(sectionRow("ショートカット")).toHaveTextContent("Ctrl+Shift+R · Ctrl+Shift+S");
    expect(sectionRow("出力")).toHaveTextContent("Documents › Chidori");
    expect(sectionRow("モデル")).toHaveTextContent("標準 · large-v3-turbo q5_0");
    expect(await screen.findByText("バージョン 0.3.0")).toBeInTheDocument();
  });

  it("says when a shortcut could not be registered", () => {
    setup("index");
    act(() => useSettings.setState({ info: makeInfo({ hotkeyErrors: { toggle: "taken" } }) }));
    expect(sectionRow("ショートカット")).toHaveTextContent("ショートカットを登録できませんでした");
  });

  it("opens a section", async () => {
    const { user } = setup("index");
    await user.click(sectionRow("音声"));
    expect(useUi.getState().settingsTab).toBe("audio");
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("音声");
  });
});

describe("Settings sections", () => {
  beforeEach(resetStores);
  afterEach(() => i18n.changeLanguage("ja"));

  it("saves a switch at once and shows what was saved", async () => {
    const { user } = setup("audio");
    const echo = screen.getByRole("switch", { name: "エコー対策" });
    expect(echo).toBeChecked();
    await user.click(echo);
    expect(saved()).toEqual([{ echoGuard: false }]);
    expect(echo).not.toBeChecked();
  });

  it("picks a mic from the devices Windows lists", async () => {
    const { user } = setup("audio");
    await user.selectOptions(screen.getByRole("combobox", { name: "マイク" }), await screen.findByRole("option", { name: "USB Mic" }));
    expect(saved()).toEqual([{ source: { micDeviceId: "usb" } }]);
    expect(useSource.getState().micDeviceId).toBe("usb");
  });

  it("switches the app's language", async () => {
    const { user } = setup("general");
    await user.selectOptions(screen.getByRole("combobox", { name: "表示言語" }), "en");
    expect(saved()).toEqual([{ locale: "en" }]);
    expect(await screen.findByRole("heading", { name: "General" })).toBeInTheDocument();
  });

  it("saves the vocabulary when leaving it, one word a line, blanks dropped", async () => {
    const { user } = setup("transcription");
    await user.type(screen.getByRole("textbox", { name: "単語リスト" }), " Chidori {Enter}{Enter}whisper.cpp");
    expect(saved()).toEqual([]);
    await user.tab();
    expect(saved()).toEqual([{ vocabulary: ["Chidori", "whisper.cpp"] }]);
  });

  it("offers the GPU only in a GPU build", () => {
    setup("transcription");
    expect(screen.getByRole("switch", { name: "GPUを使う" })).toBeEnabled();
    act(() => useSettings.setState({ info: makeInfo({ gpuCompiled: false }) }));
    expect(screen.getByRole("switch", { name: "GPUを使う" })).toBeDisabled();
  });

  it("says when a change could not be saved", async () => {
    const { user } = setup("audio");
    answer({ set_settings: fails("E_INTERNAL", "read-only"), list_mic_devices: [] });
    await user.click(screen.getByRole("switch", { name: "エコー対策" }));
    expect(useUi.getState().toasts.map((t) => t.text)).toEqual(["保存に失敗しました"]);
    expect(screen.getByRole("switch", { name: "エコー対策" })).toBeChecked();
  });

  it("records a shortcut from the keys pressed, and registers it", async () => {
    const { user } = setup("hotkeys");
    const toggle = screen.getByRole("button", { name: "Ctrl+Shift+R" });
    await user.click(toggle);
    expect(toggle).toHaveTextContent("キーを押してください…");
    // A bare letter would fire while typing anywhere: it is not taken.
    fireEvent.keyDown(toggle, { code: "KeyK" });
    expect(toggle).toHaveTextContent("キーを押してください…");
    fireEvent.keyDown(toggle, { code: "KeyK", ctrlKey: true, altKey: true });
    await screen.findByRole("button", { name: "Ctrl+Alt+K" });
    expect(saved()).toEqual([{ hotkeys: { toggle: "Ctrl+Alt+K" } }]);
    expect(calls("get_app_info")).toHaveLength(1);
  });

  it("leaves a shortcut alone on Escape, and turns one off", async () => {
    const { user } = setup("hotkeys");
    const toggle = screen.getByRole("button", { name: "Ctrl+Shift+R" });
    await user.click(toggle);
    fireEvent.keyDown(toggle, { key: "Escape", code: "Escape" });
    expect(toggle).toHaveTextContent("Ctrl+Shift+R");
    await user.click(screen.getAllByRole("button", { name: "オフにする" })[0]);
    expect(saved()).toEqual([{ hotkeys: { toggle: "" } }]);
  });

  it("shows why a shortcut could not be registered", () => {
    setup("hotkeys");
    act(() => useSettings.setState({ info: makeInfo({ hotkeyErrors: { screenshot: "Ctrl+Shift+S is taken" } }) }));
    expect(screen.getByText("登録できませんでした: Ctrl+Shift+S is taken")).toBeInTheDocument();
  });

  it("changes the output folder, and keeps it when the dialog is cancelled", async () => {
    const { user } = setup("output");
    await user.click(screen.getByRole("button", { name: "変更" }));
    expect(saved()).toEqual([{ output: { root: "D:\\Notes" } }]);
    expect(screen.getByRole("textbox", { name: "保存先フォルダ" })).toHaveValue("D:\\Notes");
    answer({ pick_folder: null });
    await user.click(screen.getByRole("button", { name: "変更" }));
    expect(saved()).toEqual([]);
  });
});
