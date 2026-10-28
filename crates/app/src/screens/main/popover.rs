//! A list floating over everything, under its anchor, or above it when there is no room below:
//! the source picker's lists, which show each app's icon and its sound, as no native menu can.
//! It is as wide as the anchor, or `min_width`; `align_end` lines up its right edge with the
//! anchor's. A press outside it, or Escape, closes it.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell, mouse};
use iced::keyboard;
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

type El<'a, M> = Element<'a, M, Theme, Renderer>;

pub struct Popover<'a, M> {
    anchor: El<'a, M>,
    popup: Option<El<'a, M>>,
    on_close: M,
    min_width: f32,
    align_end: bool,
}

/// An anchor with its list open (`Some`) or closed.
pub fn popover<'a, M: Clone + 'a>(
    anchor: impl Into<El<'a, M>>,
    popup: Option<El<'a, M>>,
    on_close: M,
) -> Popover<'a, M> {
    Popover { anchor: anchor.into(), popup, on_close, min_width: 0.0, align_end: false }
}

impl<M> Popover<'_, M> {
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    pub fn align_end(mut self) -> Self {
        self.align_end = true;
        self
    }
}

impl<M: Clone> Widget<M, Theme, Renderer> for Popover<'_, M> {
    fn children(&self) -> Vec<Tree> {
        let mut c = vec![Tree::new(&self.anchor)];
        if let Some(p) = &self.popup {
            c.push(Tree::new(p));
        }
        c
    }

    fn diff(&self, tree: &mut Tree) {
        match &self.popup {
            Some(p) => tree.diff_children(&[&self.anchor, p]),
            None => tree.diff_children(&[&self.anchor]),
        }
    }

    fn size(&self) -> Size<Length> {
        self.anchor.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.anchor.as_widget().size_hint()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.anchor.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.anchor.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.anchor.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.anchor.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.anchor.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, Theme, Renderer>> {
        let anchor_bounds = layout.bounds() + translation;
        let mut children = tree.children.iter_mut();
        let own = children.next().expect("the anchor's tree");
        let inner = self.anchor.as_widget_mut().overlay(own, layout, renderer, viewport, translation);
        let list = match (&mut self.popup, children.next()) {
            (Some(popup), Some(tree)) => Some(overlay::Element::new(Box::new(List {
                popup,
                tree,
                anchor: anchor_bounds,
                on_close: self.on_close.clone(),
                min_width: self.min_width,
                align_end: self.align_end,
            }))),
            _ => None,
        };
        match (inner, list) {
            (None, None) => None,
            (a, b) => Some(overlay::Group::with_children(a.into_iter().chain(b).collect()).overlay()),
        }
    }
}

impl<'a, M: Clone + 'a> From<Popover<'a, M>> for El<'a, M> {
    fn from(p: Popover<'a, M>) -> Self {
        Element::new(p)
    }
}

struct List<'a, 'b, M> {
    popup: &'b mut El<'a, M>,
    tree: &'b mut Tree,
    anchor: Rectangle,
    on_close: M,
    min_width: f32,
    align_end: bool,
}

/// The gap between the anchor and the list, and the margin kept from the window's edges.
const GAP: f32 = 6.0;
const MARGIN: f32 = 8.0;
const MAX_HEIGHT: f32 = 300.0;

impl<M: Clone> overlay::Overlay<M, Theme, Renderer> for List<'_, '_, M> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let a = self.anchor;
        let below = bounds.height - (a.y + a.height) - MARGIN;
        let above = a.y - MARGIN;
        let up = below < 160.0 && above > below;
        let max_h = MAX_HEIGHT.min(if up { above } else { below }).max(0.0);
        let width = a.width.max(self.min_width).min(bounds.width - 2.0 * MARGIN);
        let left = if self.align_end { a.x + a.width - width } else { a.x };
        let left = left.min(bounds.width - MARGIN - width).max(MARGIN);
        let limits = layout::Limits::new(Size::new(width, 0.0), Size::new(width, max_h));
        let node = self.popup.as_widget_mut().layout(self.tree, renderer, &limits);
        let h = node.size().height;
        let y = if up { a.y - GAP - h } else { a.y + a.height + GAP };
        node.move_to(Point::new(left, y))
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();
        self.popup.as_widget().draw(self.tree, renderer, theme, style, layout, cursor, &bounds);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
    ) {
        let bounds = layout.bounds();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(_)) if !cursor.is_over(bounds) && !cursor.is_over(self.anchor) => {
                shell.publish(self.on_close.clone());
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => {
                shell.publish(self.on_close.clone());
                shell.capture_event();
                return;
            }
            _ => {}
        }
        self.popup.as_widget_mut().update(self.tree, event, layout, cursor, renderer, clipboard, shell, &bounds);
    }

    fn mouse_interaction(&self, layout: Layout<'_>, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        let bounds = layout.bounds();
        self.popup.as_widget().mouse_interaction(self.tree, layout, cursor, &bounds, renderer)
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn widget::Operation) {
        self.popup.as_widget_mut().operate(self.tree, layout, renderer, operation);
    }

    fn overlay<'c>(
        &'c mut self,
        layout: Layout<'c>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'c, M, Theme, Renderer>> {
        self.popup.as_widget_mut().overlay(self.tree, layout, renderer, &layout.bounds(), Vector::ZERO)
    }
}
