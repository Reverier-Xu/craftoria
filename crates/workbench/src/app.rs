//! The workbench root view and the desktop application entry point.

use std::{path::PathBuf, sync::Arc, time::Duration};

use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, DockArea, DockEvent, DockPlacement, Icon, IconName,
  Size, StyleSized as _, TitleBar, Tooltip,
  gpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Render, Styled as _, Window, WindowBounds, WindowOptions, div, px, size,
  },
  h_flex, v_flex, window_border,
};

use crate::{
  WorkbenchError, layout, logs,
  panels::{LogPanel, PlaceholderPanel},
};

/// Debounce applied to dock layout persistence: `LayoutChanged` fires for
/// every intermediate drag state, so writes are coalesced.
const LAYOUT_SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// Options handed over from the CLI when launching the desktop workbench.
#[derive(Debug, Clone)]
pub struct GuiOptions {
  /// Ignore the saved dock layout and start from the default one.
  pub reset_layout: bool,
  /// The active log filter directives, displayed in the status bar.
  pub log_filter: String,
}

/// Launch the desktop workbench; returns after the last window closed.
pub fn run(options: GuiOptions) -> Result<(), WorkbenchError> {
  tracing::info!(
    version = env!("CARGO_PKG_VERSION"),
    filter = %options.log_filter,
    "starting the craftoria workbench",
  );

  woocraft::gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(move |cx: &mut App| {
      woocraft::init(cx);
      crate::panels::register(cx);
      cx.activate(true);

      let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
      let options = options.clone();
      match cx.open_window(window_options(bounds), move |window, cx| {
        cx.new(|cx| Workbench::new(options, window, cx))
      }) {
        Ok(_window) => tracing::debug!("main window opened"),
        Err(error) => {
          tracing::error!(%error, "failed to open the main window");
          cx.quit();
        }
      }
    });

  tracing::info!("workbench exited");
  Ok(())
}

fn window_options(bounds: Bounds<woocraft::gpui::Pixels>) -> WindowOptions {
  WindowOptions {
    window_bounds: Some(WindowBounds::Windowed(bounds)),
    titlebar: Some(TitleBar::title_bar_options()),
    app_id: Some("craftoria".to_string()),
    #[cfg(target_os = "linux")]
    window_background: woocraft::gpui::WindowBackgroundAppearance::Transparent,
    #[cfg(target_os = "linux")]
    window_decorations: Some(woocraft::gpui::WindowDecorations::Client),
    ..Default::default()
  }
}

/// The root view: title bar on top, dock area in the middle, status bar at
/// the bottom.
struct Workbench {
  dock_area: Entity<DockArea>,
  log_filter: String,
  layout_file: Option<PathBuf>,
  layout_save_scheduled: bool,
  focus_handle: FocusHandle,
}

impl Workbench {
  fn new(options: GuiOptions, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let dock_area =
      cx.new(|cx| DockArea::new("craftoria.main", Some(layout::LAYOUT_VERSION), window, cx));
    let layout_file = layout::layout_file();

    if !Self::restore_layout(
      &dock_area,
      options.reset_layout,
      layout_file.as_deref(),
      window,
      cx,
    ) {
      Self::default_layout(&dock_area, window, cx);
    }

    cx.subscribe(
      &dock_area,
      |this: &mut Self, _dock, event: &DockEvent, cx| {
        if matches!(event, DockEvent::LayoutChanged) {
          this.schedule_layout_save(cx);
        }
      },
    )
    .detach();

    // Refresh the status bar counters once a second.
    cx.spawn(async move |this, cx| {
      loop {
        cx.background_executor().timer(Duration::from_secs(1)).await;
        if this.update(cx, |_, cx| cx.notify()).is_err() {
          break;
        }
      }
    })
    .detach();

    Self {
      dock_area,
      log_filter: options.log_filter,
      layout_file,
      layout_save_scheduled: false,
      focus_handle: cx.focus_handle(),
    }
  }

  /// Try to restore the persisted dock layout; returns `false` when the
  /// default layout should be applied instead.
  fn restore_layout(
    dock_area: &Entity<DockArea>, reset: bool, layout_file: Option<&std::path::Path>,
    window: &mut Window, cx: &mut App,
  ) -> bool {
    if reset {
      tracing::debug!("--reset-layout given, ignoring the saved dock layout");
      return false;
    }
    let Some(path) = layout_file else {
      return false;
    };
    let state = match layout::load(path) {
      Ok(Some(state)) => state,
      Ok(None) => return false,
      Err(error) => {
        tracing::warn!(%error, "ignoring the saved dock layout");
        return false;
      }
    };
    let restored = dock_area.update(cx, |dock, cx| dock.load(state, window, cx));
    match restored {
      Ok(()) => {
        tracing::info!(path = %path.display(), "restored the saved dock layout");
        true
      }
      Err(error) => {
        tracing::warn!(%error, "failed to restore the dock layout");
        false
      }
    }
  }

  /// The default layout: explorer on the left, welcome in the center, log
  /// streaming in the bottom dock.
  fn default_layout(dock_area: &Entity<DockArea>, window: &mut Window, cx: &mut App) {
    tracing::debug!("applying the default dock layout");
    let explorer = cx.new(PlaceholderPanel::explorer);
    let welcome = cx.new(PlaceholderPanel::welcome);
    let logs = cx.new(|cx| LogPanel::new(window, cx));

    dock_area.update(cx, |dock, cx| {
      dock.add_to_left_dock(Arc::new(explorer), window, cx);
      dock.add_to_center(Arc::new(welcome), window, cx);
      dock.add_to_bottom_dock(Arc::new(logs), window, cx);
      dock.set_dock_size(DockPlacement::Left, px(260.), window, cx);
      dock.set_dock_size(DockPlacement::Bottom, px(240.), window, cx);
      // New docks start collapsed; open the ones the default layout uses.
      dock.set_dock_collapsed(DockPlacement::Left, false, window, cx);
      dock.set_dock_collapsed(DockPlacement::Bottom, false, window, cx);
    });
  }

  fn schedule_layout_save(&mut self, cx: &mut Context<Self>) {
    if self.layout_save_scheduled {
      return;
    }
    self.layout_save_scheduled = true;
    cx.spawn(async move |this, cx| {
      cx.background_executor().timer(LAYOUT_SAVE_DEBOUNCE).await;
      let _dropped = this.update(cx, |this, cx| {
        this.layout_save_scheduled = false;
        this.save_layout(cx);
      });
    })
    .detach();
  }

  fn save_layout(&mut self, cx: &mut Context<Self>) {
    let Some(path) = self.layout_file.clone() else {
      return;
    };
    let state = self.dock_area.read(cx).dump(cx);
    match layout::save(&path, &state) {
      Ok(()) => tracing::debug!(path = %path.display(), "saved the dock layout"),
      Err(error) => tracing::warn!(%error, "failed to save the dock layout"),
    }
  }

  /// Standard status bar tier: the same medium container as the title bar.
  const STATUS_BAR_SIZE: Size = Size::Medium;

  fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    h_flex()
      .w_full()
      .flex_shrink_0()
      .container_size(Self::STATUS_BAR_SIZE)
      .container_gap(Self::STATUS_BAR_SIZE)
      .items_center()
      .border_t_1()
      .border_color(cx.theme().border)
      .bg(cx.theme().background)
      .child(
        Button::new("craftoria-tips")
          .flat()
          .icon(Icon::new(IconName::Lightbulb))
          .tooltip(|window, cx| Tooltip::new("Tips").build(window, cx)),
      )
      .child(
        Button::new("craftoria-task-center")
          .flat()
          .icon(Icon::new(IconName::FlashFlow))
          .label("No tasks")
          .tooltip(|window, cx| Tooltip::new("Task Center").build(window, cx)),
      )
      .child(div().flex_1())
      .child(
        h_flex()
          .container_gap(Self::STATUS_BAR_SIZE)
          .items_center()
          .text_color(muted)
          .child(format!("craftoria v{}", env!("CARGO_PKG_VERSION")))
          .child(format!("{} log lines", logs::global().total()))
          .child(format!("filter: {}", self.log_filter))
          .child(format!(
            "{} / {}",
            std::env::consts::OS,
            std::env::consts::ARCH
          )),
      )
      .child(
        Button::new("craftoria-notification-center")
          .flat()
          .icon(Icon::new(IconName::AlertBadge))
          .tooltip(|window, cx| Tooltip::new("Notification Center").build(window, cx)),
      )
  }
}

impl Focusable for Workbench {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for Workbench {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .child(TitleBar::new().title("Craftoria").theme_button(true))
        .child(div().flex_1().min_h_0().child(self.dock_area.clone()))
        .child(self.status_bar(cx)),
    )
  }
}
