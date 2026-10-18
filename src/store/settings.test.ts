import { afterEach, beforeEach, describe, expect, it } from "vitest";
import i18n from "../i18n";
import { answer, calls } from "../test/tauri";
import { makeInfo, makeModel, makeSettings, merge, resetStores } from "../test/fixtures";
import { useSettings } from "./settings";

describe("settings store", () => {
  beforeEach(resetStores);
  afterEach(() => i18n.changeLanguage("ja"));

  it("loads the settings and app info together and shows the saved language", async () => {
    const s = { ...makeSettings(), locale: "en" as const };
    answer({ get_settings: s, get_app_info: makeInfo() });
    await useSettings.getState().load();
    expect(useSettings.getState().settings).toEqual(s);
    expect(useSettings.getState().info?.version).toBe("0.3.0");
    expect(i18n.language).toBe("en");
  });

  it("saves a change and keeps what the backend made of it", async () => {
    let current = makeSettings();
    answer({ set_settings: ({ partial }: { partial: object }) => (current = merge(current, partial)) });
    const saved = await useSettings.getState().update({ echoGuard: false, locale: "en" });
    expect(calls("set_settings")).toEqual([{ partial: { echoGuard: false, locale: "en" } }]);
    expect(saved.echoGuard).toBe(false);
    expect(useSettings.getState().settings).toEqual(current);
    expect(i18n.language).toBe("en");
  });

  it("refreshes the model catalog", async () => {
    answer({ models_list: [makeModel("turbo-q5")] });
    await useSettings.getState().refreshModels();
    expect(useSettings.getState().models.map((m) => m.id)).toEqual(["turbo-q5"]);
  });
});
