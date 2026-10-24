import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";
import i18n from "../i18n";
import { transcribeFile } from "../lib/actions";
import { useUi } from "../store/ui";

/**
 * A file dropped on the window is transcribed (FR-18) while `enabled` (nothing recording). Returns
 * whether files are being dragged over the window now, for the drop hint.
 */
export function useFileDrop(enabled: boolean): boolean {
  const [over, setOver] = useState(false);
  const on = useRef(enabled);
  useEffect(() => {
    on.current = enabled;
  }, [enabled]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let gone = false;
    getCurrentWebview()
      .onDragDropEvent(({ payload: p }) => {
        if (p.type === "enter") setOver(on.current && p.paths.length > 0);
        else if (p.type === "leave") setOver(false);
        else if (p.type === "drop") {
          setOver(false);
          if (!on.current || p.paths.length === 0) return;
          if (p.paths.length > 1) useUi.getState().toast(i18n.t("dropOneFile"), "warn");
          else void transcribeFile(p.paths[0]);
        }
      })
      .then((u) => (gone ? u() : (unlisten = u)))
      .catch(() => {});
    return () => {
      gone = true;
      unlisten?.();
    };
  }, []);
  return over && enabled;
}
