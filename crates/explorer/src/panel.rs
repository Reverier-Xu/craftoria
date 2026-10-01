//! The workspace tree panel: a persistent, non-closable left-dock panel
//! rendering the current working directory as a lazily loaded file tree.
//!
//! Expanding a folder loads exactly two levels — its entries plus a peek
//! into each subdirectory to mark the empty ones (see
//! [`crate::fs_tree::dir_items`]). The panel owns the authoritative item
//! tree (`roots`) and mirrors it into the tree widget via `update_items`,
//! which preserves expansion and selection across reloads.
//!
//! Double-clicking a folder opens a transient
//! [`DirListPanel`](crate::dir_list::DirListPanel) for it; the panel's
//! builder hands us the dock area, so sibling panels are opened through
//! the same extension-host path a future plugin would use.

use std::path::PathBuf;

use craftoria_exthost as exthost;
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, DockArea, Icon, IconName, Panel, PanelEvent, TreeEvent,
  TreeItem, TreeState,
  gpui::{
    App, AppContext as _, ClickEvent, Context, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, WeakEntity, Window, div,
  },
  tree, v_flex,
};

use crate::{dir_list, fs_tree};

/// Stable panel registration key — inherited from the placeholder this
/// panel replaces, so persisted layouts keep restoring.
pub(crate) const PANEL_NAME: &str = "craftoria.explorer";

/// i18n keys of this panel, in the explorer's extension subtree.
mod keys {
  pub const TITLE: &str = "tech.woooo.craft.ext.explorer.panel.title";
  pub const EMPTY: &str = "tech.woooo.craft.ext.explorer.panel.empty";
  pub const REFRESH: &str = "tech.woooo.craft.ext.explorer.panel.refresh";
}

/// The lazily loaded workspace file-tree panel.
pub struct ExplorerPanel {
  /// The workspace root; tree ids are paths relative to it.
  root: PathBuf,
  /// The authoritative item tree. Folders carry a pending placeholder
  /// until they are expanded for the first time.
  roots: Vec<TreeItem>,
  tree_state: woocraft::gpui::Entity<TreeState>,
  dock: WeakEntity<DockArea>,
  focus_handle: FocusHandle,
  _subscriptions: Vec<Subscription>,
}

impl ExplorerPanel {
  /// Builds the panel; `dock` is the area sibling panels are opened into.
  pub(crate) fn new(
    dock: WeakEntity<DockArea>, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let roots = fs_tree::dir_items(&root, "");

    let tree_state = cx.new(|cx| {
      TreeState::new(cx)
        .items(roots.clone())
        .multi_selectable(false)
        .draggable(false)
    });

    // `Select` fires right after a click toggled a folder's expansion, so
    // it doubles as the lazy-load hook; `DoubleClicked` opens directory
    // listings. Both need the window, hence `subscribe_in`.
    let subscriptions = vec![
      cx.subscribe_in(&tree_state, window, |this, _, event, window, cx| {
        this.handle_tree_event(event, window, cx);
      }),
    ];

    Self {
      root,
      roots,
      tree_state,
      dock,
      focus_handle: cx.focus_handle(),
      _subscriptions: subscriptions,
    }
  }

  /// Lazy-loads expanded folders and opens directory listings.
  fn handle_tree_event(&mut self, event: &TreeEvent, window: &mut Window, cx: &mut Context<Self>) {
    match event {
      TreeEvent::Select(ix) => self.load_expanded(*ix, cx),
      TreeEvent::DoubleClicked(ix) => {
        let Some(entry) = self.tree_state.read(cx).model().entry(*ix) else {
          return;
        };
        let path = self
          .root
          .join(fs_tree::relative_path(entry.item().id.as_ref()));
        if entry.is_folder() {
          dir_list::open(&self.dock, path, window, cx);
        } else {
          tracing::info!(path = %path.display(), "requested to open a file");
        }
      }
      _ => {}
    }
  }

  /// Replaces the pending placeholder of a just-expanded folder with its
  /// real children (two levels: the folder's entries plus emptiness peeks).
  fn load_expanded(&mut self, ix: usize, cx: &mut Context<Self>) {
    let id = {
      let Some(entry) = self.tree_state.read(cx).model().entry(ix) else {
        return;
      };
      if !entry.is_folder() || !entry.is_expanded() {
        return;
      }
      let pending = self
        .roots
        .iter()
        .any(|root| root.id.as_ref() == entry.item().id.as_ref() && fs_tree::is_pending(root));
      if !pending {
        return;
      }
      entry.item().id.to_string()
    };

    let dir = self.root.join(fs_tree::relative_path(&id));

    let mut roots = self.roots.clone();
    let mut children = fs_tree::dir_items(&dir, &id);
    if !replace_children(&mut roots, &id, &mut children) {
      return;
    };
    self.roots = roots.clone();

    self.tree_state.update(cx, |state, cx| {
      state.model_mut().update_items(roots);
      cx.notify();
    });
    tracing::debug!(dir = %dir.display(), "expanded a workspace folder");
  }

  /// Re-scans the workspace root, preserving the expansion of folders that
  /// still exist (still lazily: one level plus emptiness peeks).
  fn refresh(&mut self, cx: &mut Context<Self>) {
    let roots = fs_tree::dir_items(&self.root, "");
    self.roots = roots.clone();
    self.tree_state.update(cx, |state, cx| {
      state.model_mut().update_items(roots);
      cx.notify();
    });
  }
}

/// Replaces the children of the item with id `id`; `false` when no such
/// item exists. Consumes `children` on success only.
fn replace_children(items: &mut [TreeItem], id: &str, children: &mut Vec<TreeItem>) -> bool {
  for item in items.iter_mut() {
    if item.id.as_ref() == id {
      item.children = std::mem::take(children);
      return true;
    }
    if replace_children(&mut item.children, id, children) {
      return true;
    }
  }
  false
}

impl Panel for ExplorerPanel {
  fn panel_name(&self) -> &'static str {
    PANEL_NAME
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some(exthost::i18n::tr_static(keys::TITLE))
  }

  fn title(&self, _cx: &App) -> SharedString {
    exthost::i18n::tr_static(keys::TITLE)
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::Folder
  }

  /// The workspace tree is part of the shell, not a view the user opens
  /// and closes.
  fn closable(&self, _cx: &App) -> bool {
    false
  }

  fn toolbar_buttons(
    &mut self, _window: &mut Window, cx: &mut Context<Self>,
  ) -> Option<Vec<Button>> {
    let this = cx.entity();
    Some(vec![
      Button::new("craftoria-explorer-refresh")
        .flat()
        .icon(Icon::new(IconName::ArrowSync))
        .tooltip(|window, cx| {
          woocraft::Tooltip::new(exthost::i18n::tr_static(keys::REFRESH)).build(window, cx)
        })
        .on_click(move |_: &ClickEvent, _window: &mut Window, cx: &mut App| {
          this.update(cx, |this, cx| this.refresh(cx));
        }),
    ])
  }
}

impl EventEmitter<PanelEvent> for ExplorerPanel {}

impl Focusable for ExplorerPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for ExplorerPanel {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_empty = self.tree_state.read(cx).model().is_empty();

    v_flex()
      .size_full()
      .bg(cx.theme().background)
      .p_2()
      .child(if is_empty {
        div()
          .flex_1()
          .flex()
          .items_center()
          .justify_center()
          .text_color(cx.theme().muted_foreground)
          .child(exthost::i18n::tr_static(keys::EMPTY))
          .into_any_element()
      } else {
        tree(&self.tree_state).into_any_element()
      })
  }
}
