import { Check, ChevronDown, Ellipsis, History as HistoryIcon, Search, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { SessionLine } from "../components/SessionLine";
import { Button, Dialog, IconButton, inputClass } from "../components/ui";
import { PageTitle } from "../components/PageTitle";
import { commands } from "../ipc/commands";
import type { Project, SessionSummary } from "../ipc/types";
import { reportError } from "../lib/actions";
import { dayName, dayOf, dayText, durationText, timeOf } from "../lib/format";
import { shapeFromActivity } from "../lib/line";
import { menuText, popupMenu, type MenuEntry } from "../lib/popupMenu";
import { ProjectMark } from "../components/Projects";
import { projectEntries } from "../lib/projects";
import { sessionEntries } from "../lib/sessionMenu";
import { projectOf, useProjects } from "../store/projects";
import { useUi } from "../store/ui";

const norm = (s: string) => s.normalize("NFKC").toLowerCase();

/** Shorter than this, a recording is most likely a test or started by accident. */
const SHORT_MS = 10_000;

/** A recording under 10 s or with no transcript: greyed out (still readable, still openable), with no line. */
const isEmptySession = (s: Pick<SessionSummary, "segments" | "durationMs">) => s.segments === 0 || s.durationMs < SHORT_MS;

/** A row's checkbox in selection mode: a rounded square, ticked in ink. */
function Tick({ on }: { on: boolean }) {
  return (
    <span aria-hidden className={`grid size-[18px] shrink-0 place-items-center rounded-sm border-[1.5px] transition-colors duration-150 ${on ? "border-fg bg-fg text-bg" : "border-line-strong bg-surface"}`}>
      {on && <Check size={12} strokeWidth={3.2} />}
    </span>
  );
}

/** `inProject`: the list is one project's page, whose square is in the title, so rows leave theirs out. */
function Row({ s, onMenu, selecting, checked, onToggle, inProject }: { s: SessionSummary; onMenu: () => void; selecting: boolean; checked: boolean; onToggle: () => void; inProject: boolean }) {
  const { t } = useTranslation();
  const go = useUi((st) => st.go);
  const two = s.sources.includes("mic") && s.sources.some((id) => id !== "mic");
  const empty = isEmptySession(s);
  const shape = useMemo(() => (s.activity ? shapeFromActivity(s.activity) : null), [s.activity]);
  const project = useProjects((st) => projectOf(st.projects, s.project));
  return (
    <li className="group relative animate-fade-up">
      <button
        type="button"
        role={selecting ? "checkbox" : undefined}
        aria-checked={selecting ? checked : undefined}
        onClick={() => (selecting ? onToggle() : go("session", { sessionId: s.id }))}
        className={`flex w-full items-center gap-3 rounded-lg px-2 pt-2.5 pb-2 text-left transition-colors hover:bg-surface-2 ${checked ? "bg-surface-2" : ""} ${empty ? "text-muted" : ""}`}
      >
        {selecting && <Tick on={checked} />}
        <span className="grid min-w-0 flex-1 grid-cols-[minmax(0,1fr)_auto] gap-x-2.5">
          <span className="flex min-w-0 items-center gap-2">
            {project && !inProject && (
              <span title={project.name} className="flex shrink-0">
                <ProjectMark color={project.color} />
                <span className="sr-only">{project.name}</span>
              </span>
            )}
            <span className={`min-w-0 truncate text-[13.5px] ${empty ? "" : "font-medium"}`}>{s.title}</span>
            {s.recoverable && <span className="shrink-0 rounded-sm bg-warning-tint px-1.5 text-[10.5px] font-medium text-warning">{t("needsRecovery")}</span>}
          </span>
          <span className={`num text-[11.5px] text-muted transition-opacity ${selecting ? "" : "group-focus-within:opacity-0 group-hover:opacity-0"}`}>{timeOf(s.startedAt)}</span>
          {shape && !empty && <SessionLine shape={shape} two={two} className="col-span-2 mt-1.5 mb-1" />}
          <span className="min-w-0 truncate text-[12px] text-muted">{s.preview || t("emptyTranscript")}</span>
          <span className="text-[11.5px] text-muted">{durationText(s.durationMs, t)}</span>
        </span>
      </button>
      {!selecting && (
        <IconButton
          size="sm"
          label={t("moreActions")}
          aria-haspopup="menu"
          onClick={onMenu}
          className="absolute top-1.5 right-1.5 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
        >
          <Ellipsis size={16} />
        </IconButton>
      )}
    </li>
  );
}

/** Which sessions the list shows: all, one project's, or those in none. */
type ProjectFilter = { kind: "all" } | { kind: "none" } | { kind: "project"; id: string };

/**
 * Past sessions by day, each with its own line; searchable by title and opening words (FR-62,
 * FR-63), and narrowed to one project (FR-64), which makes the page that project's: its name on
 * top and its own menu to change or delete it. 選択 picks several to move to the Recycle Bin at
 * once, such as all the short and empty ones, or to put in a project.
 */
export function History() {
  const { t, i18n } = useTranslation();
  const [sessions, setSessions] = useState<SessionSummary[] | null>(null);
  const [query, setQuery] = useState("");
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<ReadonlySet<string>>(() => new Set());
  const [deletingProject, setDeletingProject] = useState<Project | null>(null);
  const confirming = useUi((s) => !!s.deleting);
  const [now] = useState(() => new Date());
  const [chosenFilter, setFilter] = useState<ProjectFilter>({ kind: "all" });
  const projects = useProjects((s) => s.projects);
  const askNewProject = useUi((s) => s.askNewProject);
  // A deleted project can't stay the filter.
  const filter = useMemo<ProjectFilter>(
    () => (chosenFilter.kind === "project" && !projectOf(projects, chosenFilter.id) ? { kind: "all" } : chosenFilter),
    [chosenFilter, projects],
  );

  const refresh = async () => {
    try {
      setSessions(await commands.listSessions());
    } catch (e) {
      reportError(e);
      setSessions([]);
    }
  };

  useEffect(() => {
    void useProjects.getState().load().catch(() => {});
  }, []);

  // A project changed somewhere (a menu here, the new-project dialog): reload the rows.
  useEffect(
    () =>
      useProjects.subscribe((s, prev) => {
        if (s.version !== prev.version) void refresh();
      }),
    [], // eslint-disable-line react-hooks/exhaustive-deps
  );

  useEffect(() => {
    let alive = true;
    commands
      .listSessions()
      .then((s) => alive && setSessions(s))
      .catch((e) => {
        reportError(e);
        if (alive) setSessions([]);
      });
    return () => {
      alive = false;
    };
  }, []);

  const days = useMemo(() => {
    const q = norm(query.trim());
    const inFilter = (s: SessionSummary) => {
      const known = projectOf(projects, s.project);
      if (filter.kind === "none") return !known;
      if (filter.kind === "project") return known?.id === filter.id;
      return true;
    };
    const found = (sessions ?? []).filter((s) => inFilter(s) && (!q || norm(s.title).includes(q) || norm(s.preview).includes(q)));
    const out: { day: string; items: SessionSummary[] }[] = [];
    for (const s of found) {
      const day = dayOf(s.startedAt);
      const last = out[out.length - 1];
      if (last?.day === day) last.items.push(s);
      else out.push({ day, items: [s] });
    }
    return out;
  }, [sessions, query, filter, projects]);

  // Only what the search shows can be selected, so nothing hidden is deleted.
  const visible = useMemo(() => days.flatMap((d) => d.items), [days]);
  const chosen = visible.filter((s) => selected.has(s.id));
  const empties = visible.filter(isEmptySession);
  const allChosen = visible.length > 0 && chosen.length === visible.length;
  const choose = (list: SessionSummary[]) => setSelected(new Set(list.map((s) => s.id)));
  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const stopSelecting = () => {
    setSelecting(false);
    setSelected(new Set());
  };

  // Escape leaves selection mode (in the search field it clears the search instead).
  useEffect(() => {
    if (!selecting || confirming) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || (e.target as HTMLElement).tagName === "INPUT") return;
      setSelecting(false);
      setSelected(new Set());
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selecting, confirming]);

  const deleteChosen = () =>
    useUi.getState().askDelete({
      ids: chosen.map((s) => s.id),
      done: () => {
        stopSelecting();
        void refresh();
      },
    });

  const assign = (ids: string[], project: string | null) =>
    void useProjects
      .getState()
      .assign(ids, project)
      .catch((e) => reportError(e, "saveFailed"));
  const projectMenu = (ids: string[], current?: string) => projectEntries(t, projects, current, (id) => assign(ids, id), () => askNewProject(ids));

  const filterMenu = async () => {
    const entries: MenuEntry[] = [
      { text: t("allProjects"), checked: filter.kind === "all", action: () => setFilter({ kind: "all" }) },
      ...projects.map((p) => ({ text: menuText(p.name), checked: filter.kind === "project" && filter.id === p.id, action: () => setFilter({ kind: "project", id: p.id }) })),
      { text: t("noProject"), checked: filter.kind === "none", action: () => setFilter({ kind: "none" }) },
      "separator",
      // A new project opens as its own page, empty until sessions join it.
      { text: t("newProjectMenu"), action: () => askNewProject([], (id) => setFilter({ kind: "project", id })) },
    ];
    await popupMenu(entries).catch(reportError);
  };
  const filtered = filter.kind === "project" ? projectOf(projects, filter.id) : undefined;

  // The project's own menu, on its page.
  const projectPageMenu = async (p: Project) => {
    const entries: MenuEntry[] = [
      { text: t("editProject"), action: () => useUi.getState().setEditingProject(p) },
      "separator",
      { text: t("deleteProject"), action: () => setDeletingProject(p) },
    ];
    await popupMenu(entries).catch(reportError);
  };

  // The menu the session has when open, plus what its deck and header would hold there.
  const menu = (s: SessionSummary) => void popupMenu(sessionEntries(s, true, () => void refresh())).catch(reportError);

  const dayLabel = (day: string) => {
    const name = dayName(day, now);
    return { name: name === "today" ? t("today") : name === "yesterday" ? t("yesterday") : null, date: dayText(day, i18n.language, now.getFullYear()) };
  };

  return (
    <div className="flex h-full flex-col">
      <PageTitle
        title={
          filtered ? (
            <span className="flex min-w-0 items-center gap-2.5 break-words">
              <ProjectMark color={filtered.color} size={14} />
              {filtered.name}
            </span>
          ) : (
            t("historyTitle")
          )
        }
        sub={filtered ? t("historyCount", { n: (sessions ?? []).filter((x) => x.project === filtered.id).length }) : sessions && sessions.length > 0 ? t("historyCount", { n: sessions.length }) : undefined}
        action={
          <span className="flex items-center gap-1">
            {filtered && (
              <IconButton size="sm" label={t("projectMenu")} aria-haspopup="menu" onClick={() => void projectPageMenu(filtered)}>
                <Ellipsis size={16} />
              </IconButton>
            )}
            {sessions && sessions.length > 0 && (
              <Button variant="ghost" size="sm" onClick={() => (selecting ? stopSelecting() : setSelecting(true))} aria-pressed={selecting}>
                {selecting ? t("done") : t("select")}
              </Button>
            )}
          </span>
        }
      >
        <div className="mt-2.5 flex gap-1.5">
          <label className="group relative block min-w-0 flex-1">
            <Search size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted transition-colors group-focus-within:text-fg" />
            <input
              type="search"
              aria-label={t("searchHistory")}
              placeholder={t("searchHistory")}
              className={`${inputClass} !h-[38px] !rounded-md pr-8 pl-9 placeholder:text-muted [&::-webkit-search-cancel-button]:hidden`}
              value={query}
              autoFocus
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && setQuery("")}
            />
            {query && (
              <button type="button" aria-label={t("close")} onClick={() => setQuery("")} className="absolute top-1/2 right-2 grid size-5 -translate-y-1/2 place-items-center rounded-full text-muted hover:bg-surface-2 hover:text-fg">
                <X size={12} />
              </button>
            )}
          </label>
          {projects.length > 0 && (
            <button
              type="button"
              aria-haspopup="menu"
              aria-label={`${t("project")}: ${filtered?.name ?? (filter.kind === "none" ? t("noProject") : t("allProjects"))}`}
              onClick={filterMenu}
              className={`flex h-[38px] max-w-[140px] shrink-0 items-center gap-1.5 rounded-md border px-2.5 text-[12.5px] transition-colors hover:border-line-strong ${filter.kind === "all" ? "border-line text-muted" : "border-fg font-medium"}`}
            >
              {filter.kind !== "all" && <ProjectMark color={filtered?.color} />}
              <span className="min-w-0 truncate">{filtered?.name ?? (filter.kind === "none" ? t("noProject") : t("project"))}</span>
              <ChevronDown size={13} className="shrink-0 text-muted" />
            </button>
          )}
        </div>
        {selecting && (
          <div className="mt-2 flex animate-fade-in flex-wrap items-center gap-x-4 gap-y-1 px-0.5">
            <button type="button" className="kk-link !px-0" onClick={() => choose(allChosen ? [] : visible)}>
              {allChosen ? t("selectNone") : t("selectAll")}
            </button>
            {empties.length > 0 && (
              <button type="button" className="kk-link !px-0" onClick={() => choose(empties)}>
                {t("selectEmpty", { n: empties.length })}
              </button>
            )}
          </div>
        )}
      </PageTitle>
      <div className="min-h-0 flex-1 overflow-y-auto border-t border-line px-2.5 pb-3">
        {sessions === null ? (
          <div className="space-y-2 px-1 pt-3" aria-label={t("loading")}>
            {[0, 1, 2].map((i) => (
              <div key={i} className="h-[86px] animate-pulse rounded-lg bg-surface-2" style={{ animationDelay: `${i * 120}ms` }} />
            ))}
          </div>
        ) : sessions.length === 0 ? (
          <div className="flex h-full animate-fade-in flex-col items-center justify-center gap-3 pb-10 text-center text-muted">
            <HistoryIcon size={26} />
            {t("historyEmpty")}
          </div>
        ) : days.length === 0 ? (
          <div className="animate-fade-in p-8 text-center text-muted">{filtered && !query.trim() ? t("projectEmpty") : t("historyNoMatch")}</div>
        ) : (
          days.map(({ day, items }) => {
            const label = dayLabel(day);
            return (
              <section key={day}>
                <h3 className="sticky top-0 z-10 flex items-baseline gap-2 bg-bg px-2 pt-3.5 pb-1 text-[12px]">
                  {label.name && <b className="font-medium">{label.name}</b>}
                  <span className={label.name ? "text-muted" : "font-medium"}>{label.date}</span>
                </h3>
                <ul className="space-y-0.5">
                  {items.map((s) => (
                    <Row key={s.id} s={s} onMenu={() => menu(s)} selecting={selecting} checked={selected.has(s.id)} onToggle={() => toggle(s.id)} inProject={!!filtered} />
                  ))}
                </ul>
              </section>
            );
          })
        )}
      </div>
      {selecting && (
        <div className="flex shrink-0 animate-fade-in items-center justify-between gap-3 border-t border-line px-[18px] py-2.5">
          <span className="text-[12.5px] text-muted" aria-live="polite">
            {t("selectedCount", { n: chosen.length })}
          </span>
          <span className="flex gap-2">
            <Button
              size="lg"
              disabled={chosen.length === 0}
              aria-haspopup="menu"
              onClick={() => void popupMenu(projectMenu(chosen.map((s) => s.id))).catch(reportError)}
            >
              {t("project")}
            </Button>
            <Button variant="primary" size="lg" disabled={chosen.length === 0} onClick={deleteChosen}>
              {t("delete")}
            </Button>
          </span>
        </div>
      )}
      {deletingProject && (
        <Dialog
          title={t("confirmDeleteProject", { name: deletingProject.name })}
          onClose={() => setDeletingProject(null)}
          actions={
            <>
              <Button onClick={() => setDeletingProject(null)}>{t("cancel")}</Button>
              <Button
                variant="danger"
                onClick={() => {
                  const id = deletingProject.id;
                  setDeletingProject(null);
                  void useProjects
                    .getState()
                    .remove(id)
                    .catch((e) => reportError(e, "deleteFailed"));
                }}
              >
                {t("delete")}
              </Button>
            </>
          }
        >
          {t("deleteProjectNote")}
        </Dialog>
      )}
    </div>
  );
}
