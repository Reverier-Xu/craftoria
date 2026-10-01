//! Placeholder panels that reserve dock regions until the real domain panels
//! (modeling, rendering, media tools) land.

use craftoria_exthost::i18n as exthost_i18n;
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

  /// The i18n key of this panel's title.
  fn title_key(&self) -> &'static str {
    match self.kind {
      PlaceholderKind::Explorer => "panel.explorer.title",
      PlaceholderKind::Welcome => "panel.welcome.title",
    }
  }

  /// The i18n key of this panel's description.
  fn description_key(&self) -> &'static str {
    match self.kind {
      PlaceholderKind::Explorer => "panel.explorer.description",
      PlaceholderKind::Welcome => "panel.welcome.description",
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
    Some(exthost_i18n::tr_static(self.title_key()))
  }

  fn title(&self, _cx: &App) -> SharedString {
    exthost_i18n::tr_static(self.title_key())
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
          .child(exthost_i18n::tr_static(self.title_key())),
      )
      .child(
        div()
          .max_w(px(420.))
          .text_center()
          .text_color(cx.theme().muted_foreground)
          .child(exthost_i18n::tr_static(self.description_key())),
      )
  }
}
