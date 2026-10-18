import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import type { BenchmarkResult, ModelEntry } from "../ipc/types";
import { useSettings } from "../store/settings";
import { makeInfo, makeModel, makeSettings, resetStores } from "../test/fixtures";
import { answer, calls, emit } from "../test/tauri";
import { SetupWizard } from "./SetupWizard";

let catalog: ModelEntry[];
let bench: BenchmarkResult;

function setup(models: ModelEntry[]) {
  catalog = models;
  bench = { seconds: 2.5, tier: "comfortable", gpu: true, text: "" };
  answer({
    models_list: () => structuredClone(catalog),
    model_select: ({ id }: { id: string }) => {
      catalog = catalog.map((m) => ({ ...m, selected: m.id === id }));
    },
    run_benchmark: () => bench,
    mic_test: { peakDbfs: -12, text: "テスト中です", device: "USB Mic" },
    get_app_info: makeInfo(),
  });
  useSettings.setState({ settings: makeSettings(), info: makeInfo({ setupDone: false }) });
  return { user: userEvent.setup(), ...render(<SetupWizard />) };
}

const next = () => screen.getByRole("button", { name: "次へ" });
const title = () => screen.getByRole("heading", { level: 2 });

/** Marks a model downloaded, as the backend does before it reports the download done. */
function finishDownload(id: string) {
  catalog = catalog.map((m) => (m.id === id ? { ...m, installed: true } : m));
  act(() => emit("model://download", { id, received: 1, total: 1, bytesPerSec: 0, phase: "done" }));
}

const turbo = () => makeModel("turbo-q5", { nameJa: "標準", recommended: true });
const small = () => makeModel("small-q5", { nameJa: "軽量" });

describe("SetupWizard", () => {
  beforeEach(resetStores);

  it("offers the visible models with the recommended one chosen", async () => {
    const { user } = setup([turbo(), small(), makeModel("base-q5", { nameJa: "隠し", hidden: true })]);
    expect(title()).toHaveTextContent("ようこそ");
    expect(screen.queryByRole("button", { name: "戻る" })).not.toBeInTheDocument();
    await user.click(next());
    expect(title()).toHaveTextContent("モデルを選ぶ");
    const radios = await screen.findAllByRole("radio");
    expect(radios).toHaveLength(2);
    expect(screen.getByRole("radio", { name: /標準/ })).toBeChecked();
    expect(screen.getByText("おすすめ")).toBeInTheDocument();
    expect(screen.queryByText("隠し")).not.toBeInTheDocument();
  });

  it("downloads the chosen model, selects and measures it, and only then goes on", async () => {
    const { user } = setup([turbo(), small()]);
    await user.click(next());
    await user.click(await screen.findByRole("radio", { name: /軽量/ }));
    await user.click(next());
    expect(title()).toHaveTextContent("ダウンロード");
    expect(calls("model_download")).toEqual([{ id: "small-q5" }]);
    expect(next()).toBeDisabled();
    act(() => emit("model://download", { id: "small-q5", received: 50 * 2 ** 20, total: 100 * 2 ** 20, bytesPerSec: 2 ** 20, phase: "downloading" }));
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "50");
    finishDownload("small-q5");
    expect(await screen.findByText("10秒の音声を 2.5 秒で処理（快適）")).toBeInTheDocument();
    expect(calls("model_select")).toEqual([{ id: "small-q5" }]);
    expect(calls("run_benchmark")).toEqual([{ modelId: "small-q5" }]);
    expect(next()).toBeEnabled();
  });

  it("offers to retry a failed download", async () => {
    const { user } = setup([turbo()]);
    await user.click(next());
    await user.click(next());
    act(() => emit("model://download", { id: "turbo-q5", received: 0, total: 1, bytesPerSec: 0, phase: "error", error: { code: "E_NETWORK", message: "" } }));
    expect(await screen.findByText("ネットワークに接続できません")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "再試行" }));
    expect(calls("model_download")).toHaveLength(2);
  });

  it("suggests the light model when the chosen one is heavy for this PC", async () => {
    const { user } = setup([{ ...turbo(), installed: true, selected: true }, small()]);
    bench = { seconds: 14, tier: "heavy", gpu: false, text: "" };
    await user.click(next());
    await user.click(next());
    expect(await screen.findByText(/「軽量」モデルがおすすめです/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "ダウンロード" }));
    expect(calls("model_download")).toEqual([{ id: "small-q5" }]);
  });

  it("tests the mic, shows what it heard, then names the shortcuts and finishes", async () => {
    const { user } = setup([{ ...turbo(), installed: true, selected: true }]);
    await user.click(next());
    await user.click(next());
    await screen.findByText(/快適/);
    await user.click(next());
    expect(title()).toHaveTextContent("マイクのテスト");
    await user.click(screen.getByRole("button", { name: "テストを開始" }));
    expect(calls("mic_test")).toEqual([{ seconds: 10 }]);
    expect(await screen.findByText("テスト中です")).toBeInTheDocument();
    expect(screen.getByText("USB Mic")).toBeInTheDocument();
    await user.click(next());
    expect(title()).toHaveTextContent("ショートカット");
    expect(screen.getAllByText("Shift")).toHaveLength(2);
    expect(screen.getByText("R")).toBeInTheDocument();
    await user.click(next());
    expect(title()).toHaveTextContent("準備ができました");
    await user.click(screen.getByRole("button", { name: "はじめる" }));
    expect(calls("complete_setup")).toHaveLength(1);
    expect(calls("get_app_info")).toHaveLength(1);
  });

  it("reassures when the mic test heard no words, and the test can be skipped", async () => {
    const { user } = setup([{ ...turbo(), installed: true, selected: true }]);
    answer({ models_list: () => structuredClone(catalog), run_benchmark: () => bench, mic_test: { peakDbfs: -80, text: "", device: null } });
    await user.click(next());
    await user.click(next());
    await screen.findByText(/快適/);
    await user.click(next());
    await user.click(screen.getByRole("button", { name: "テストを開始" }));
    expect(await screen.findByText("言葉は聞き取れませんでしたが、続けて大丈夫です")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "戻る" }));
    await user.click(next());
    await user.click(screen.getByRole("button", { name: "スキップ" }));
    expect(title()).toHaveTextContent("ショートカット");
  });
});
