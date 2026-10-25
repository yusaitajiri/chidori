import { ChevronDown } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Project } from "../ipc/types";
import { reportError } from "../lib/actions";
import { colorKey, freshColor, PROJECT_COLORS, useProjectMenu } from "../lib/projects";
import { projectOf, useProjects } from "../store/projects";
import { useUi } from "../store/ui";
import { Button, Dialog, inputClass } from "./ui";

/** A project's colour as a small rounded square (the dot is the app's only circle); hollow for none. */
export function ProjectMark({ color, size = 10, className = "" }: { color?: string; size?: number; className?: string }) {
  return (
    <span
      aria-hidden
      className={`inline-block shrink-0 rounded-[3px] ${color ? "" : "border-[1.5px] border-line-strong"} ${className}`}
      style={{ width: size, height: size, ...(color ? { background: `var(--kk-p-${color}, var(--kk-p-slate))` } : {}) }}
    />
  );
}

/** The colour choice: one square per colour, the chosen one ringed. */
export function ColorChoice({ value, onChange }: { value: string; onChange: (c: string) => void }) {
  const { t } = useTranslation();
  return (
    <div role="radiogroup" aria-label={t("projectColor")} className="flex flex-wrap gap-1.5">
      {PROJECT_COLORS.map((c) => (
        <button
          key={c}
          type="button"
          role="radio"
          aria-checked={value === c}
          aria-label={t(colorKey(c))}
          title={t(colorKey(c))}
          onClick={() => onChange(c)}
          className={`grid size-7 place-items-center rounded-md border transition-colors ${value === c ? "border-fg" : "border-transparent hover:border-line-strong"}`}
        >
          <ProjectMark color={c} size={16} className="!rounded-[4px]" />
        </button>
      ))}
    </div>
  );
}

/** Names a new project and puts the sessions it was asked for in it. */
export function NewProjectDialog() {
  const ask = useUi((s) => s.newProject);
  // A fresh form for each ask.
  return ask ? <ProjectForm key={ask.n} ask={ask} /> : null;
}

/** Changes a project's name and colour, from its page in 履歴. */
export function EditProjectDialog() {
  const project = useUi((s) => s.editingProject);
  return project ? <ProjectForm key={project.id} project={project} /> : null;
}

/** A project's name and colour: a new one (`ask`) or one being changed (`project`). */
function ProjectForm({ ask, project }: { ask?: { sessionIds: string[]; done?: (id: string) => void }; project?: Project }) {
  const { t } = useTranslation();
  const close = () => (project ? useUi.getState().setEditingProject(undefined) : useUi.getState().askNewProject(undefined));
  const [name, setName] = useState(project?.name ?? "");
  const [color, setColor] = useState(() => project?.color ?? freshColor(useProjects.getState().projects));

  const save = async () => {
    if (!name.trim()) return;
    close();
    try {
      const { create, update, assign } = useProjects.getState();
      if (project) {
        await update({ ...project, name: name.trim(), color });
        return;
      }
      const made = await create(name.trim(), color);
      if (ask?.sessionIds.length) await assign(ask.sessionIds, made.id);
      ask?.done?.(made.id);
    } catch (e) {
      reportError(e, "saveFailed");
    }
  };
  return (
    <Dialog
      title={t(project ? "editProject" : "newProject")}
      onClose={close}
      actions={
        <>
          <Button onClick={close}>{t("cancel")}</Button>
          <Button variant="primary" disabled={!name.trim()} onClick={save}>
            {t(project ? "save" : "createProject")}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <input
          aria-label={t("projectName")}
          placeholder={t("projectName")}
          className={inputClass}
          value={name}
          maxLength={40}
          autoFocus
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void save()}
        />
        <ColorChoice value={color} onChange={setColor} />
      </div>
    </Dialog>
  );
}

/**
 * A session's project in its header: the colour and the name (or a hollow square and 「プロジェクトなし」),
 * opening the project menu. `bare` shows the square alone (the recording's header, beside its title).
 */
export function ProjectButton({ sessionId, project, bare = false }: { sessionId: string; project?: string; bare?: boolean }) {
  const { t } = useTranslation();
  const projects = useProjects((s) => s.projects);
  const menu = useProjectMenu();
  const p = projectOf(projects, project);
  const label = p ? `${t("project")}: ${p.name}` : t("noProject");

  useEffect(() => {
    void useProjects.getState().load().catch(() => {});
  }, []);

  if (bare) {
    return (
      <button type="button" aria-haspopup="menu" aria-label={label} title={label} onClick={() => menu([sessionId], project)} className="grid size-7 shrink-0 place-items-center rounded-md transition-colors hover:bg-surface-2">
        <ProjectMark color={p?.color} size={11} />
      </button>
    );
  }
  return (
    <button type="button" aria-haspopup="menu" aria-label={label} onClick={() => menu([sessionId], project)} className="-mx-1 inline-flex max-w-full items-center gap-1.5 rounded-sm px-1 py-0.5 text-[12px] transition-colors hover:bg-surface-2">
      <ProjectMark color={p?.color} />
      <span className={`min-w-0 truncate ${p ? "font-medium text-fg" : "text-muted"}`}>{p ? p.name : t("noProject")}</span>
      <ChevronDown size={12} className="shrink-0 text-muted" />
    </button>
  );
}
