// A session's ⋯ menu, built once for every place a saved session is shown (docs/OBJECTS.md), so it
// offers the same actions wherever it is: the open session's deck and each of 履歴's rows.

import i18n from "../i18n";
import { useProjects } from "../store/projects";
import { useRecording } from "../store/recording";
import { useUi } from "../store/ui";
import { continueRecording, copy, openFolder, recoverSession, reportError } from "./actions";
import { projectEntries } from "./projects";
import type { MenuEntry } from "./popupMenu";

export type MenuSession = { id: string; title: string; folder?: string; project?: string; recoverable?: boolean };

/** The copy formats; the deck's コピー button is the first, its chevron the other two. */
export const copyEntries = (id: string, short: boolean): MenuEntry[] => {
  const t = (k: string) => i18n.t(k);
  return [
    ...(short ? [{ text: t("copyPlainShort"), action: () => void copy("plain", id) }] : []),
    { text: t(short ? "copyAgentShort" : "copyForAgent"), action: () => void copy("agent", id) },
    { text: t(short ? "copyMarkdownShort" : "copyMarkdown"), action: () => void copy("markdown", id) },
  ];
};

/**
 * The menu's entries. An open session (`row` false) leaves out what its deck and header already
 * hold (copy, 書き出し, the title, the project); a row has neither, so it holds everything.
 * `changed` runs once an action has changed the session, for a list to reload.
 */
export function sessionEntries(s: MenuSession, row: boolean, changed?: () => void): MenuEntry[] {
  const t = (k: string) => i18n.t(k);
  const ui = useUi.getState();
  const assign = (id: string | null) =>
    void useProjects
      .getState()
      .assign([s.id], id)
      .catch((e) => reportError(e, "saveFailed"));
  const entries: MenuEntry[] = [];
  if (s.recoverable) entries.push({ text: t("recover"), action: () => void recoverSession(s.id).then(changed) }, "separator");
  entries.push({ text: t("continueRecording"), enabled: !s.recoverable && useRecording.getState().state === "ready", action: () => void continueRecording(s.id) }, "separator");
  if (row) {
    entries.push(
      { text: t("copy"), items: copyEntries(s.id, true) },
      { text: t("exportDialog"), action: () => ui.setExportFor(s.id) },
      "separator",
      { text: t("rename"), action: () => ui.askRename({ id: s.id, title: s.title, done: changed }) },
      { text: t("project"), items: projectEntries(t, useProjects.getState().projects, s.project, assign, () => ui.askNewProject([s.id])) },
    );
  }
  entries.push(
    { text: t("openFolder"), enabled: !!s.folder, action: () => void openFolder(s.folder) },
    "separator",
    { text: t("delete"), action: () => ui.askDelete({ ids: [s.id], title: s.title, done: changed }) },
  );
  return entries;
}
