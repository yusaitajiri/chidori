import type { Settings } from "../ipc/types";
import { reportError } from "../lib/actions";
import { useSettings } from "../store/settings";

/** Saves one shortcut, then rereads which ones Windows refused. */
export function useSetHotkey() {
  const update = useSettings((s) => s.update);
  const refreshInfo = useSettings((s) => s.refreshInfo);
  return async (k: keyof Settings["hotkeys"], v: string) => {
    try {
      await update({ hotkeys: { [k]: v } });
      await refreshInfo();
    } catch (e) {
      reportError(e, "saveFailed");
    }
  };
}
