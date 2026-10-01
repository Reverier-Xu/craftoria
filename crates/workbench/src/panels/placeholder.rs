//! The welcome placeholder reserving the center dock until the real
//! domain panels (modeling, rendering, media tools) land.

use craftoria_exthost as exthost;
use woocraft::{
  ActiveTheme, Icon, IconName, Panel, PanelEvent, Sizable, Size, StyledExt as _,
  gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, px,
  },
  v_flex,
};

/// The center-area welcome placeholder.
pub struct PlaceholderPanel {
  focus_handle: FocusHandle,
}

impl PlaceholderPanel {
  pub const WELCOME_NAME: &'static str = "craftoria.welcome";

  /// The center-area welcome placeholder.
  pub fn welcome(cx: &mut Context<Self>) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
    }
  }
}

impl Panel for PlaceholderPanel {
  fn panel_name(&self) -> &'static str {
    Self::WELCOME_NAME
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some(exthost::i18n::tr_static("panel.welcome.title"))
  }

  fn title(&self, _cx: &App) -> SharedString {
    exthost::i18n::tr_static("panel.welcome.title")
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::Home
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
      .child(
        Icon::new(IconName::Home)
          .with_size(Size::Large)
          .text_color(cx.theme().muted_foreground),
      )
      .child(
        div()
          .font_semibold()
          .text_color(cx.theme().foreground)
          .child(exthost::i18n::tr_static("panel.welcome.title")),
      )
      .child(
        div()
          .max_w(px(420.))
          .text_center()
          .text_color(cx.theme().muted_foreground)
          .child(exthost::i18n::tr_static("panel.welcome.description")),
      )
  }
}
