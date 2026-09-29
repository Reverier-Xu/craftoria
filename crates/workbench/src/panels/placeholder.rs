//! Placeholder panels that reserve dock regions until the real domain panels
//! (modeling, rendering, media tools) land.

use woocraft::{
  ActiveTheme, Icon, IconName, Panel, PanelEvent, Sizable, Size, StyledExt as _,
  gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, px,
  },
  v_flex,
};

/// The fixed set of placeholder panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaceholderKind {
  Explorer,
  Welcome,
}

/// A dock panel showing a centered icon, a title, and a short description.
pub struct PlaceholderPanel {
  kind: PlaceholderKind,
  focus_handle: FocusHandle,
}

impl PlaceholderPanel {
  pub const EXPLORER_NAME: &'static str = "craftoria.explorer";
  pub const WELCOME_NAME: &'static str = "craftoria.welcome";

  /// The left-dock workspace explorer placeholder.
  pub fn explorer(cx: &mut Context<Self>) -> Self {
    Self::new(PlaceholderKind::Explorer, cx)
  }

  /// The center-area welcome placeholder.
  pub fn welcome(cx: &mut Context<Self>) -> Self {
    Self::new(PlaceholderKind::Welcome, cx)
  }

  fn new(kind: PlaceholderKind, cx: &mut Context<Self>) -> Self {
    Self {
      kind,
      focus_handle: cx.focus_handle(),
    }
  }

  fn tab_title(&self) -> &'static str {
    match self.kind {
      PlaceholderKind::Explorer => "Explorer",
      PlaceholderKind::Welcome => "Welcome",
    }
  }

  fn body(&self) -> &'static str {
    match self.kind {
      PlaceholderKind::Explorer => "Workspace files and project assets will live here.",
      PlaceholderKind::Welcome => {
        "The modeling, rendering, and media tools will dock here. \
         Log output streams in the bottom dock."
      }
    }
  }

  fn kind_icon(&self) -> IconName {
    match self.kind {
      PlaceholderKind::Explorer => IconName::Folder,
      PlaceholderKind::Welcome => IconName::Home,
    }
  }
}

impl Panel for PlaceholderPanel {
  fn panel_name(&self) -> &'static str {
    match self.kind {
      PlaceholderKind::Explorer => Self::EXPLORER_NAME,
      PlaceholderKind::Welcome => Self::WELCOME_NAME,
    }
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some(self.tab_title().into())
  }

  fn title(&self, _cx: &App) -> SharedString {
    self.tab_title().into()
  }

  fn icon(&self, _cx: &App) -> IconName {
    self.kind_icon()
  }
}

impl EventEmitter<PanelEvent> for PlaceholderPanel {}

impl Focusable for PlaceholderPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for PlaceholderPanel {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .size_full()
      .items_center()
      .justify_center()
      .gap_3()
      .p_4()
      .bg(cx.theme().background)
      .child(
        Icon::new(self.kind_icon())
          .with_size(Size::Large)
          .text_color(cx.theme().muted_foreground),
      )
      .child(
        div()
          .font_semibold()
          .text_color(cx.theme().foreground)
          .child(self.tab_title()),
      )
      .child(
        div()
          .max_w(px(420.))
          .text_center()
          .text_color(cx.theme().muted_foreground)
          .child(self.body()),
      )
  }
}
