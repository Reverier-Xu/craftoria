//! The workbench root view and the desktop application entry point.

use std::{path::PathBuf, time::Duration};

use craftoria_exthost as exthost;
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, Divider, DockArea, DockEvent, DockPlacement,
  DropdownMenu as _, Icon, IconName, PopupMenuItem, Size, StyleSized as _, TitleBar, Tooltip,
  gpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Render, Styled as _, Window, WindowBounds, WindowOptions, div, px, size,
  },
  h_flex, v_flex, window_border,
};

use crate::{
  WorkbenchError, layout, logs,
  settings::{self, Settings},
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

      // The composition root (the shell) registered its extensions by
      // now; flush their contributions — panel builders into woocraft's
      // registry (so saved dock layouts restore), strings into the shared
      // i18n pool — and apply the saved locale on top of the
      // environment-derived default before any window opens.
      exthost::extension::init(cx);

      let settings_path = settings::settings_file();
      let settings = match settings_path.as_deref() {
        Some(path) => match settings::load(path) {
          Ok(settings) => settings,
          Err(error) => {
            tracing::warn!(%error, "ignoring the saved workbench settings");
            Settings::default()
          }
        },
        None => Settings::default(),
      };
      if let Some(locale) = settings.locale.clone() {
        exthost::i18n::set_locale(locale, cx);
      }

      cx.activate(true);

      let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
      let options = options.clone();
      let startup = StartupSettings {
        settings,
        file: settings_path,
      };
      match cx.open_window(window_options(bounds), move |window, cx| {
        cx.new(|cx| Workbench::new(options, startup, window, cx))
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

/// The settings loaded at startup, handed over to the root view so it can
/// persist later changes back to the same file.
struct StartupSettings {
  settings: Settings,
  file: Option<PathBuf>,
}

/// The root view: title bar on top, dock area in the middle, status bar at
/// the bottom.
struct Workbench {
  dock_area: Entity<DockArea>,
  log_filter: String,
  layout_file: Option<PathBuf>,
  layout_save_scheduled: bool,
  settings: Settings,
  settings_file: Option<PathBuf>,
  focus_handle: FocusHandle,
}

impl Workbench {
  fn new(
    options: GuiOptions, startup: StartupSettings, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self {
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
      settings: startup.settings,
      settings_file: startup.file,
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

  /// The default layout, composed from the extensions' persistent panel
  /// contributions: within a dock, extension registration order wins, and
  /// the first contribution's placement hint picks the dock's size and
  /// initial collapse state.
  fn default_layout(dock_area: &Entity<DockArea>, window: &mut Window, cx: &mut App) {
    tracing::debug!("applying the default dock layout");

    let contributions = exthost::panel::contributions();
    for dock in [
      DockPlacement::Left,
      DockPlacement::Center,
      DockPlacement::Bottom,
      DockPlacement::Right,
    ] {
      let placed = contributions
        .iter()
        .filter_map(|contribution| match contribution.role {
          exthost::panel::PanelRole::Persistent(placement) if placement.dock == dock => {
            Some((contribution, placement))
          }
          _ => None,
        })
        .collect::<Vec<_>>();
      if placed.is_empty() {
        continue;
      }

      for (contribution, _) in &placed {
        let Some(panel) = exthost::panel::build(
          contribution.name.as_ref(),
          dock_area.downgrade(),
          window,
          cx,
        ) else {
          continue;
        };
        match dock {
          DockPlacement::Center => {
            dock_area.update(cx, |area, cx| area.add_to_center(panel, window, cx))
          }
          DockPlacement::Left => {
            dock_area.update(cx, |area, cx| area.add_to_left_dock(panel, window, cx))
          }
          DockPlacement::Right => {
            dock_area.update(cx, |area, cx| area.add_to_right_dock(panel, window, cx))
          }
          DockPlacement::Bottom => {
            dock_area.update(cx, |area, cx| area.add_to_bottom_dock(panel, window, cx))
          }
        }
      }

      // The center dock has no size or collapse state of its own.
      if dock != DockPlacement::Center {
        let placement = placed[0].1;
        if let Some(size) = placement.size {
          dock_area.update(cx, |area, cx| area.set_dock_size(dock, size, window, cx));
        }
        dock_area.update(cx, |area, cx| {
          area.set_dock_collapsed(dock, placement.collapsed, window, cx)
        });
      }
    }
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

  /// Switches the UI locale and persists the choice.
  fn select_locale(&mut self, locale: &str, cx: &mut Context<Self>) {
    exthost::i18n::set_locale(locale, cx);
    self.settings.locale = Some(locale.to_string());
    self.save_settings();
    cx.notify();
  }

  fn save_settings(&self) {
    let Some(path) = self.settings_file.as_deref() else {
      return;
    };
    match settings::save(path, &self.settings) {
      Ok(()) => tracing::debug!(path = %path.display(), "saved the workbench settings"),
      Err(error) => tracing::warn!(%error, "failed to save the workbench settings"),
    }
  }

  /// Standard status bar tier: the same medium container as the title bar.
  const STATUS_BAR_SIZE: Size = Size::Medium;

  /// The locale picker: one checked entry per supported locale, switching
  /// through [`Workbench::select_locale`].
  fn language_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let this = cx.entity();
    Button::new("craftoria-language")
      .flat()
      .icon(Icon::new(IconName::LocalLanguage))
      .tooltip(|window, cx| {
        Tooltip::new(exthost::i18n::tr_static("status_bar.language")).build(window, cx)
      })
      .dropdown_menu(move |mut menu, _window, _cx| {
        let current = exthost::i18n::locale();
        for locale in exthost::i18n::SUPPORTED_LOCALES {
          let label = exthost::i18n::locale_display_name(locale);
          let target = locale.to_string();
          let this = this.clone();
          menu = menu.item(
            PopupMenuItem::label(label)
              .checked(locale == current)
              .on_click(move |_, _window, cx| {
                let target = target.clone();
                this.update(cx, |this, cx| this.select_locale(&target, cx));
              }),
          );
        }
        menu
      })
  }

  fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let total = logs::global().total();
    h_flex()
      .w_full()
      .flex_shrink_0()
      .container_size(Self::STATUS_BAR_SIZE)
      .container_gap(Self::STATUS_BAR_SIZE)
      .items_center()
      .bg(cx.theme().background)
      .child(
        Button::new("craftoria-tips")
          .flat()
          .icon(Icon::new(IconName::Lightbulb))
          .tooltip(|window, cx| {
            Tooltip::new(exthost::i18n::tr_static("status_bar.tips")).build(window, cx)
          }),
      )
      .child(
        Button::new("craftoria-task-center")
          .flat()
          .icon(Icon::new(IconName::FlashFlow))
          .label(exthost::i18n::tr_static("status_bar.no_tasks"))
          .tooltip(|window, cx| {
            Tooltip::new(exthost::i18n::tr_static("status_bar.task_center")).build(window, cx)
          }),
      )
      .child(div().flex_1())
      .child(
        h_flex()
          .container_gap(Self::STATUS_BAR_SIZE)
          .items_center()
          .text_color(muted)
          .child(format!("craftoria v{}", env!("CARGO_PKG_VERSION")))
          .child(exthost::i18n::tr_with(
            "status_bar.log_lines",
            &[("count", &total.to_string())],
          ))
          .child(exthost::i18n::tr_with(
            "status_bar.filter",
            &[("filter", self.log_filter.as_str())],
          ))
          .child(format!(
            "{} / {}",
            std::env::consts::OS,
            std::env::consts::ARCH
          )),
      )
      .child(self.language_picker(cx))
      .child(
        Button::new("craftoria-notification-center")
          .flat()
          .icon(Icon::new(IconName::AlertBadge))
          .tooltip(|window, cx| {
            Tooltip::new(exthost::i18n::tr_static("status_bar.notification_center"))
              .build(window, cx)
          }),
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
        .child(Divider::horizontal())
        .child(div().flex_1().min_h_0().child(self.dock_area.clone()))
        .child(Divider::horizontal())
        .child(self.status_bar(cx)),
    )
  }
}
