import { useState } from "react";
import { useTranslation } from "react-i18next";
import { acceleratorFrom } from "../lib/hotkeys";
import { Button, Field, inputClass } from "./ui";

/** A shortcut: off (empty) until you press 設定 and then the keys; オフにする turns it off again. */
export function HotkeyInput({ value, onChange, label, error }: { value: string; onChange: (v: string) => void; label: string; error?: string }) {
  const { t } = useTranslation();
  const [recording, setRecording] = useState(false);
  return (
    <Field label={label}>
      <div className="flex gap-1.5">
        <button
          type="button"
          className={`${inputClass} min-w-0 flex-1 text-left ${value && !recording ? "font-mono" : "text-muted"} ${recording ? "!border-accent !bg-info-bg !text-fg" : ""}`}
          onClick={() => setRecording(true)}
          onBlur={() => setRecording(false)}
          onKeyDown={(e) => {
            if (!recording) return;
            e.preventDefault();
            if (e.key === "Escape") {
              setRecording(false);
              return;
            }
            const acc = acceleratorFrom(e.nativeEvent);
            if (acc) {
              setRecording(false);
              onChange(acc);
            }
          }}
        >
          {recording ? t("hotkeyRecord") : value || t("hotkeyOff")}
        </button>
        {value && !recording && <Button onClick={() => onChange("")}>{t("hotkeyClear")}</Button>}
      </div>
      {error && <div className="mt-1 text-xs font-medium text-err">{t("hotkeyFailed", { error })}</div>}
    </Field>
  );
}
