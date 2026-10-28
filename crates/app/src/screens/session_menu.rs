//! A session's one ⋯ menu (docs/OBJECTS.md), the same wherever the session is shown: open or as
//! a row of 履歴, which adds what the open session has elsewhere (copy, export, rename, project).

use chidori_core::events::UiState;
use chidori_core::session::projects::Project;

use crate::app::App;
use crate::i18n::t;
use crate::native::menu::Item;

/// The session a menu is for.
#[derive(Debug, Clone, Copy)]
pub struct SessionRef<'a> {
    pub id: &'a str,
    /// It ended in a crash and can be rebuilt from its log (FR-61).
    pub recoverable: bool,
    /// Its project's ID (FR-64).
    pub project: Option<&'a str>,
}

/// The ⋯ menu's items; `row` for a session shown as a row of 履歴. An open session leaves out what
/// its deck and header already hold (copy, 書き出し, the title, the project); a row has neither, so
/// it holds everything.
pub fn items(app: &App, s: SessionRef<'_>, row: bool) -> Vec<Item> {
    build(app.rec.state, &app.projects, s, row)
}

fn build(state: UiState, projects: &[Project], s: SessionRef<'_>, row: bool) -> Vec<Item> {
    let id = s.id;
    let mut items = Vec::new();
    if s.recoverable {
        items.push(Item::action(format!("action:recover:{id}"), t("recover")));
        items.push(Item::Separator);
    }
    items.push(
        Item::action(format!("action:continue:{id}"), t("continueRecording"))
            .enabled(!s.recoverable && state == UiState::Ready),
    );
    items.push(Item::Separator);
    if row {
        items.push(Item::submenu(t("copy"), copy_items(id)));
        items.push(Item::action(format!("dialog:export:{id}"), t("exportDialog")));
        items.push(Item::Separator);
        items.push(Item::action(format!("action:rename:{id}"), t("rename")));
        items.push(Item::submenu(t("project"), project_choices(projects, &[id.to_string()], s.project)));
    }
    items.push(Item::action(format!("action:open-folder:{id}"), t("openFolder")));
    items.push(Item::Separator);
    items.push(Item::action(format!("action:delete:{id}"), t("delete")));
    items
}

/// A row's copy formats, by their short names.
fn copy_items(id: &str) -> Vec<Item> {
    vec![
        Item::action(format!("action:copy-plain:{id}"), t("copyPlainShort")),
        Item::action(format!("action:copy-agent:{id}"), t("copyAgentShort")),
        Item::action(format!("action:copy-markdown:{id}"), t("copyMarkdownShort")),
    ]
}

/// The project choices for sessions: no project, each project, then 新しいプロジェクト…. A session
/// whose project was deleted shows as in none.
pub fn project_items(app: &App, session_ids: &[String], current: Option<&str>) -> Vec<Item> {
    project_choices(&app.projects, session_ids, current)
}

fn project_choices(projects: &[Project], session_ids: &[String], current: Option<&str>) -> Vec<Item> {
    let ids = session_ids.join(",");
    let known = current.is_some_and(|c| projects.iter().any(|p| p.id == c));
    let mut items = vec![Item::check(format!("dialog:assign:-:{ids}"), t("noProject"), !known)];
    for p in projects {
        items.push(Item::check(
            format!("dialog:assign:{}:{ids}", p.id),
            menu_text(&p.name),
            current == Some(p.id.as_str()),
        ));
    }
    items.push(Item::Separator);
    items.push(Item::action(format!("dialog:new-project:{ids}"), t("newProjectMenu")));
    items
}

/// A name as a native menu shows it: a lone `&` would underline the next letter instead.
pub fn menu_text(name: &str) -> String {
    name.replace('&', "&&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: &str) -> Project {
        Project { id: id.into(), name: id.into(), color: "blue".into() }
    }

    fn checked(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .filter_map(|i| match i {
                Item::Check { label, checked: true, .. } => Some(label.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn checks_the_sessions_project_or_none() {
        crate::i18n::set_locale(chidori_core::settings::Locale::Ja);
        let projects = [p("a"), p("b")];
        let ids = ["s1".to_string()];
        assert_eq!(checked(&project_choices(&projects, &ids, Some("b"))), ["b"]);
        assert_eq!(checked(&project_choices(&projects, &ids, None)), ["プロジェクトなし"]);
        // A deleted project's ID.
        assert_eq!(checked(&project_choices(&projects, &ids, Some("gone"))), ["プロジェクトなし"]);
    }

    #[test]
    fn picks_name_the_sessions() {
        let items = project_choices(&[p("a")], &["s1".into(), "s2".into()], None);
        let ids: Vec<_> = items
            .iter()
            .filter_map(|i| match i {
                Item::Check { id, .. } | Item::Action { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, ["dialog:assign:-:s1,s2", "dialog:assign:a:s1,s2", "dialog:new-project:s1,s2"]);
    }

    fn labels(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|i| match i {
                Item::Action { label, .. } | Item::Check { label, .. } | Item::Submenu { label, .. } => label.clone(),
                Item::Separator => "─".into(),
            })
            .collect()
    }

    #[test]
    fn a_row_adds_what_a_deck_and_header_hold() {
        crate::i18n::set_locale(chidori_core::settings::Locale::Ja);
        let s = SessionRef { id: "s1", recoverable: false, project: None };
        assert_eq!(labels(&build(UiState::Ready, &[], s, false)), ["続きを録音", "─", "フォルダを開く", "─", "削除"]);
        assert_eq!(
            labels(&build(UiState::Ready, &[], s, true)),
            [
                "続きを録音",
                "─",
                "コピー",
                "書き出し…",
                "─",
                "名前を変更",
                "プロジェクト",
                "フォルダを開く",
                "─",
                "削除"
            ]
        );
    }

    #[test]
    fn a_crashed_session_offers_recovery_first_and_nothing_to_record_onto() {
        crate::i18n::set_locale(chidori_core::settings::Locale::Ja);
        let items = build(UiState::Ready, &[], SessionRef { id: "s1", recoverable: true, project: None }, true);
        assert_eq!(labels(&items)[..2], ["復元", "─"]);
        assert!(matches!(&items[2], Item::Action { enabled: false, .. }));
    }

    #[test]
    fn menu_text_keeps_ampersands() {
        assert_eq!(menu_text("R&D"), "R&&D");
    }
}
