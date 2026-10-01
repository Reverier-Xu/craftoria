//! Transient directory-listing panels: closable, multi-instance table
//! views of a single directory.
//!
//! Opened on demand — from the workspace tree's double-click, or from
//! another listing's double-click on a subdirectory — through
//! [`open`](self::open), the extension-host build path carrying the
//! directory as per-instance state. That state is also what the panel
//! persists in [`Panel::dump`], so open listings survive a layout
//! save/restore.

use std::path::{Path, PathBuf};

use craftoria_exthost as exthost;
use serde::{Deserialize, Serialize};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, Column, ColumnSort, DockArea, Icon, IconName, Panel,
  PanelEvent, PanelInfo, PanelState, Table, TableDelegate, TableEvent, TableState,
  gpui::{
    App, AppContext as _, Context, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, WeakEntity, Window, div, px,
  },
  h_flex,
};

use crate::fs_tree::{self, EntryRow};

/// Stable panel registration key for directory listings.
pub(crate) const PANEL_NAME: &str = "craftoria.explorer.dir-list";

/// i18n keys of this panel, in the explorer's extension subtree.
mod keys {
  pub const COL_NAME: &str = "tech.woooo.craft.ext.explorer.dir_list.col.name";
  pub const COL_SIZE: &str = "tech.woooo.craft.ext.explorer.dir_list.col.size";
  pub const COL_KIND: &str = "tech.woooo.craft.ext.explorer.dir_list.col.kind";
  pub const KIND_DIR: &str = "tech.woooo.craft.ext.explorer.dir_list.kind.dir";
  pub const KIND_FILE: &str = "tech.woooo.craft.ext.explorer.dir_list.kind.file";
  pub const REFRESH: &str = "tech.woooo.craft.ext.explorer.panel.refresh";
}

/// The per-instance state a listing persists and restores with: the
/// directory it shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirState {
  dir: String,
}

/// The JSON payload of a listing's [`PanelInfo`]; the null value for
/// listings opened without state (they fall back to the workspace root).
fn state_value(info: &PanelInfo) -> serde_json::Value {
  match info {
    PanelInfo::Panel(value) => value.clone(),
    _ => serde_json::Value::Null,
  }
}

/// Opens a directory listing for `dir` in the center dock area.
pub(crate) fn open(dock: &WeakEntity<DockArea>, dir: PathBuf, window: &mut Window, cx: &mut App) {
  let Some(dock_area) = dock.upgrade() else {
    return;
  };

  let state = serde_json::to_value(DirState {
    dir: dir.display().to_string(),
  })
  .unwrap_or(serde_json::Value::Null);
  let Some(panel) = exthost::panel::build_with_state(PANEL_NAME, state, dock.clone(), window, cx)
  else {
    return;
  };

  dock_area.update(cx, |dock, cx| dock.add_to_center(panel, window, cx));
  tracing::debug!(dir = %dir.display(), "opened a directory listing");
}

/// A closable, multi-instance table view of one directory.
pub struct DirListPanel {
  /// The listed directory.
  dir: PathBuf,
  /// The dock further listings are opened into.
  dock: WeakEntity<DockArea>,
  table_state: woocraft::gpui::Entity<TableState<DirTableDelegate>>,
  focus_handle: FocusHandle,
  _subscriptions: Vec<Subscription>,
}

impl DirListPanel {
  /// Builds a listing for `dir`.
  fn new(
    dir: PathBuf, dock: WeakEntity<DockArea>, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self {
    let table_state = cx.new(|cx| TableState::new(DirTableDelegate::new(&dir), window, cx));

    let subscriptions =
      vec![
        cx.subscribe_in(&table_state, window, |this, _, event, window, cx| {
          if let TableEvent::DoubleClickedRow(ix) = event {
            this.open_row(*ix, window, cx);
          }
        }),
      ];

    Self {
      dir,
      dock,
      table_state,
      focus_handle: cx.focus_handle(),
      _subscriptions: subscriptions,
    }
  }

  /// Builds a listing from persisted state; falls back to the workspace
  /// root when the state is missing or malformed.
  pub(crate) fn from_state(
    info: &PanelInfo, dock: WeakEntity<DockArea>, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self {
    let dir = match serde_json::from_value::<DirState>(state_value(info)) {
      Ok(state) => PathBuf::from(state.dir),
      Err(error) => {
        tracing::warn!(
          %error,
          "directory listing state was malformed, listing the workspace root"
        );
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
      }
    };
    Self::new(dir, dock, window, cx)
  }

  /// Double-clicked rows navigate: directories open nested listings,
  /// files log for now (the editor integration lands later).
  fn open_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
    let row = self.table_state.read(cx).delegate().rows.get(ix).cloned();
    let Some(row) = row else {
      return;
    };

    let path = self.dir.join(&row.name);
    if !row.is_dir {
      tracing::info!(path = %path.display(), "requested to open a file");
      return;
    }

    // Opening a panel mutates the dock area, which re-activates the panels
    // of the target tab group — including this very listing when it is the
    // active one. That would be a re-entrant entity update, so defer past
    // the current update batch.
    let dock = self.dock.clone();
    window.defer(cx, move |window, cx| {
      open(&dock, path, window, cx);
    });
  }

  /// Re-lists the directory.
  fn refresh(&mut self, cx: &mut Context<Self>) {
    let dir = self.dir.clone();
    self.table_state.update(cx, |state, cx| {
      state.delegate_mut().reload(&dir);
      cx.notify();
    });
  }
}

impl Panel for DirListPanel {
  fn panel_name(&self) -> &'static str {
    PANEL_NAME
  }

  /// Directory-scoped identity, so several listings coexist and restore
  /// independently across sessions.
  fn panel_id(&self, _cx: &App) -> SharedString {
    format!("{PANEL_NAME}:{}", self.dir.display()).into()
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some(dir_label(&self.dir))
  }

  fn title(&self, _cx: &App) -> SharedString {
    self.dir.display().to_string().into()
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::FolderList
  }

  /// Listings persist their directory so they restore where they were.
  fn dump(&self, _cx: &App) -> PanelState {
    PanelState {
      panel_name: self.panel_name().to_string(),
      children: Vec::new(),
      info: PanelInfo::Panel(
        serde_json::to_value(DirState {
          dir: self.dir.display().to_string(),
        })
        .unwrap_or(serde_json::Value::Null),
      ),
    }
  }

  fn toolbar_buttons(
    &mut self, _window: &mut Window, cx: &mut Context<Self>,
  ) -> Option<Vec<Button>> {
    let this = cx.entity();
    Some(vec![
      Button::new("craftoria-explorer-dir-refresh")
        .flat()
        .icon(Icon::new(IconName::ArrowSync))
        .tooltip(|window, cx| {
          woocraft::Tooltip::new(exthost::i18n::tr_static(keys::REFRESH)).build(window, cx)
        })
        .on_click(move |_, _window, cx| {
          this.update(cx, |this, cx| this.refresh(cx));
        }),
    ])
  }
}

/// The label shared by the tab: the directory's own name, falling back to
/// the full path for roots like `/`.
fn dir_label(dir: &Path) -> SharedString {
  dir
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .filter(|name| !name.is_empty())
    .map(SharedString::from)
    .unwrap_or_else(|| dir.display().to_string().into())
}

impl EventEmitter<PanelEvent> for DirListPanel {}

impl Focusable for DirListPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for DirListPanel {
  fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    // The table owns its surface and its header/cell rules. The default
    // outer frame plus radius are chrome for a table embedded in a
    // document, not for one filling its dock.
    Table::new(&self.table_state).bordered(false)
  }
}

/// The table data of one listing: the directory's rows plus the active
/// sort, re-applied on reload.
struct DirTableDelegate {
  rows: Vec<EntryRow>,
  sort: Option<(usize, ColumnSort)>,
}

impl DirTableDelegate {
  fn new(dir: &Path) -> Self {
    Self {
      rows: fs_tree::list_dir(dir),
      sort: None,
    }
  }

  fn reload(&mut self, dir: &Path) {
    self.rows = fs_tree::list_dir(dir);
    if let Some((col_ix, sort)) = self.sort {
      self.sort_rows(col_ix, sort);
    }
  }

  /// Folders always lead, whatever the column sort is.
  fn sort_rows(&mut self, col_ix: usize, sort: ColumnSort) {
    let ascending = matches!(sort, ColumnSort::Ascending);
    self.rows.sort_by(|a, b| {
      let ordering = match col_ix {
        0 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        1 => a.size.cmp(&b.size),
        _ => a.is_dir.cmp(&b.is_dir),
      };
      let ordering = if ascending {
        ordering
      } else {
        ordering.reverse()
      };
      match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => ordering,
      }
    });
  }
}

impl TableDelegate for DirTableDelegate {
  fn columns_count(&self, _cx: &App) -> usize {
    3
  }

  fn rows_count(&self, _cx: &App) -> usize {
    self.rows.len()
  }

  fn column(&self, col_ix: usize, _cx: &App) -> Column {
    match col_ix {
      0 => Column::new("name", exthost::i18n::tr_static(keys::COL_NAME))
        .width(px(240.))
        .sortable(),
      1 => Column::new("size", exthost::i18n::tr_static(keys::COL_SIZE))
        .width(px(90.))
        .text_right()
        .sortable(),
      _ => Column::new("kind", exthost::i18n::tr_static(keys::COL_KIND))
        .width(px(110.))
        .sortable(),
    }
  }

  fn perform_sort(
    &mut self, col_ix: usize, sort: ColumnSort, _window: &mut Window,
    _cx: &mut Context<TableState<Self>>,
  ) {
    self.sort = Some((col_ix, sort));
    self.sort_rows(col_ix, sort);
  }

  fn render_td(
    &mut self, row_ix: usize, col_ix: usize, _window: &mut Window,
    cx: &mut Context<TableState<Self>>,
  ) -> impl IntoElement {
    let Some(row) = self.rows.get(row_ix) else {
      return div().into_any_element();
    };

    match col_ix {
      0 => h_flex()
        .w_full()
        .items_center()
        .gap_2()
        .child(
          Icon::new(if row.is_dir {
            IconName::Folder
          } else {
            IconName::Document
          })
          .text_color(cx.theme().muted_foreground),
        )
        .child(row.name.clone())
        .into_any_element(),
      1 => h_flex()
        .w_full()
        .justify_end()
        .child(if row.is_dir {
          "—".to_string()
        } else {
          fs_tree::format_size(row.size)
        })
        .into_any_element(),
      _ => h_flex()
        .w_full()
        .child(exthost::i18n::tr_static(if row.is_dir {
          keys::KIND_DIR
        } else {
          keys::KIND_FILE
        }))
        .into_any_element(),
    }
  }
}
