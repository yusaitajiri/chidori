//! Pieces 設定, the model list and the setup wizard share: the big title with a line beside it,
//! the scrolling body, choices for a dropdown, small tags and cards, and fitting a summary into
//! its column.

use std::fmt;

use iced::widget::{Space, button, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::app::Message;
use crate::theme::{self, line, p, tokens};

/// The big title on the ground, with a quiet line beside it (`設定` under a section's name, `2 / 6`
/// in the wizard). The hairline or rule under it belongs to the page.
pub fn head<'a>(title: String, sub: Option<String>) -> Element<'a, Message> {
    let mut r = row![line(title, tokens::TITLE, p().ink)].spacing(10).align_y(Alignment::End);
    if let Some(sub) = sub {
        // Sits on the title's baseline rather than the bottom of its line box.
        r = r.push(container(line(sub, tokens::CAPTION, p().ink_muted)).padding(Padding::ZERO.bottom(4)));
    }
    container(r)
        .width(Length::Fill)
        .padding(Padding::new(tokens::SPACE_4).top(tokens::SPACE_3).bottom(tokens::SPACE_3))
        .into()
}

/// A page body that scrolls, with the thin scrollbar of the rest of the app.
pub fn scroll<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    scrollable(content)
        .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(4).scroller_width(4).margin(3)))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_, status| {
            let p = p();
            let hovered = matches!(
                status,
                scrollable::Status::Hovered { is_vertical_scrollbar_hovered: true, .. }
                    | scrollable::Status::Dragged { is_vertical_scrollbar_dragged: true, .. }
            );
            let rail = scrollable::Rail {
                background: None,
                border: Border::default(),
                scroller: scrollable::Scroller {
                    background: Background::Color(if hovered { p.ink_muted } else { p.border_strong }),
                    border: Border { radius: 2.0.into(), ..Border::default() },
                },
            };
            scrollable::Style {
                container: container::Style::default(),
                vertical_rail: rail,
                horizontal_rail: rail,
                gap: None,
                auto_scroll: scrollable::AutoScroll {
                    background: Background::Color(p.surface_raised),
                    border: Border { color: p.border, width: 1.0, radius: tokens::RADIUS_SM.into() },
                    shadow: iced::Shadow::default(),
                    icon: p.ink_muted,
                },
            }
        })
        .into()
}

/// One choice in a dropdown: what is saved, and what it says.
#[derive(Debug, Clone)]
pub struct Opt<T> {
    pub value: T,
    pub label: String,
}

impl<T: PartialEq> PartialEq for Opt<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> fmt::Display for Opt<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

pub fn opt<T>(value: T, label: impl Into<String>) -> Opt<T> {
    Opt { value, label: label.into() }
}

/// A dropdown over `options` showing `value`; a pick sends what `on` makes of its value.
pub fn choose<'a, T>(options: Vec<Opt<T>>, value: &T, on: impl Fn(T) -> Message + 'a) -> Element<'a, Message>
where
    T: Clone + PartialEq + 'a,
{
    let selected = options.iter().find(|o| o.value == *value).cloned();
    crate::widgets::select(options, selected, move |o: Opt<T>| on(o.value))
}

/// A small tag beside a name: outlined, or filled in ink for the one that stands out.
pub fn tag<'a>(label: String, filled: bool) -> Element<'a, Message> {
    let fg = if filled { p().surface } else { p().ink_muted };
    container(line(label, tokens::CAPTION, fg).font(theme::SANS_MEDIUM))
        .padding([0.0, 6.0])
        .style(move |_| {
            let p = p();
            container::Style {
                background: filled.then_some(Background::Color(p.ink)),
                border: Border {
                    color: if filled { p.ink } else { p.border },
                    width: 1.0,
                    radius: tokens::RADIUS_SM.into(),
                },
                ..container::Style::default()
            }
        })
        .into()
}

/// A card on the ground: raised, a hairline round it, or ink round the one in use.
pub fn card<'a>(content: impl Into<Element<'a, Message>>, chosen: bool) -> container::Container<'a, Message> {
    container(content).width(Length::Fill).padding(tokens::SPACE_3).style(move |_| card_style(chosen, false))
}

pub fn card_style(chosen: bool, hovered: bool) -> container::Style {
    let p = p();
    container::Style {
        background: (!chosen).then_some(Background::Color(p.surface_raised)),
        border: Border {
            color: if chosen {
                p.ink
            } else if hovered {
                p.border_strong
            } else {
                p.border
            },
            width: 1.0,
            radius: tokens::RADIUS_LG.into(),
        },
        ..container::Style::default()
    }
}

/// A radio mark: a ring, with the accent's dot when chosen.
pub fn radio_mark<'a>(checked: bool) -> Element<'a, Message> {
    let dot = container(Space::new()).width(8).height(8).style(move |_| container::Style {
        background: checked.then_some(Background::Color(p().accent)),
        border: Border { radius: 4.0.into(), ..Border::default() },
        ..container::Style::default()
    });
    container(dot)
        .center(16)
        .style(move |_| {
            let p = p();
            container::Style {
                background: Some(p.surface_raised.into()),
                border: Border {
                    color: if checked { p.accent } else { p.border_strong },
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..container::Style::default()
            }
        })
        .into()
}

/// A quiet text button (ログを開く, ほかのモデル): muted, ink while hovered.
pub fn link<'a>(content: impl Into<Element<'a, Message>>, on: Message) -> Element<'a, Message> {
    button(content)
        .padding([2.0, 4.0])
        .style(|_, status| button::Style {
            text_color: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                p().ink
            } else {
                p().ink_muted
            },
            border: Border { radius: tokens::RADIUS_SM.into(), ..Border::default() },
            ..button::Style::default()
        })
        .on_press(on)
        .into()
}

/// A paragraph of body text.
pub fn para<'a>(s: String) -> Element<'a, Message> {
    theme::t(s, tokens::BODY, p().ink).into()
}

/// Shortens `s` with an ellipsis so it fits about `max_px` at `size` px: Japanese takes a full
/// em, Latin about half of one.
pub fn fit(s: &str, max_px: f32, size: f32) -> String {
    let width = |c: char| if c.is_ascii() { 0.55 } else { 1.0 } * size;
    let total: f32 = s.chars().map(width).sum();
    if total <= max_px {
        return s.to_string();
    }
    let room = max_px - width('…');
    let mut used = 0.0;
    let mut out = String::new();
    for c in s.chars() {
        used += width(c);
        if used > room {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::fit;

    #[test]
    fn long_text_ends_in_an_ellipsis() {
        assert_eq!(fit("日本語", 210.0, 12.0), "日本語");
        let long = fit("Ctrl+Shift+R · Ctrl+Shift+S · Ctrl+Shift+M · Ctrl+Shift+X", 120.0, 12.0);
        assert!(long.ends_with('…'));
        assert!(long.chars().count() < 25);
    }
}
