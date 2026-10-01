//! Panel contributions.
//!
//! Extensions contribute dock panels through [`PanelContribution`]s: a
//! stable registration key, the role the panel plays in the shell, and the
//! same builder signature woocraft's panel registry uses for
//! deserialization — so a contributed panel restores from a saved dock
//! layout exactly like a built-in one.
//!
//! Two roles exist:
//!
//! - [`PanelRole::Persistent`] panels form the shell's default layout and
//!   cannot be closed by the user (the file tree, the log console);
//! - [`PanelRole::Transient`] panels are opened on demand — possibly many at
//!   once, each carrying per-instance state — and can be closed again (a
//!   directory listing). They join the default layout only implicitly: they are
//!   never part of it, but a transient panel that was open when a layout was
//!   saved restores with it, because its builder receives the persisted
//!   [`PanelState`].
//!
//! The module knows nothing about concrete panels. It talks only in
//! woocraft framework types (`PanelView`, `DockPlacement`, the builder
//! signature) and per-instance JSON state; which panels exist, how they
//! render, and what their state means is entirely the extensions'
//! business.

use std::sync::Arc;

use woocraft::{
  DockArea, DockPlacement, PanelInfo, PanelState, PanelView,
  gpui::{App, SharedString, WeakEntity, Window},
};

/// Signature of a contributed panel builder: builds the panel entity for a
/// dock area, optionally consulting the persisted [`PanelState`] and
/// [`PanelInfo`] it is being restored from. Transient panels read their
/// per-instance state from `info` here.
///
/// This mirrors the deserialization builder [`woocraft::register_panel`]
/// takes, so contributions feed the same registry the layout restore path
/// reads.
pub type PanelBuilder = Arc<
  dyn Fn(WeakEntity<DockArea>, &PanelState, &PanelInfo, &mut Window, &mut App) -> Box<dyn PanelView>
    + Send
    + Sync,
>;

/// Where a persistent panel prefers to live in the default dock layout.
///
/// A hint, not a command: the host composes contributions into a layout
/// and remains free to adjust (persisted layouts always win over hints).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelPlacement {
  /// The dock the panel prefers.
  pub dock: DockPlacement,
  /// Preferred dock size in pixels; when several panels share a dock, the
  /// first hint wins.
  pub size: Option<woocraft::gpui::Pixels>,
  /// Whether the dock should start collapsed.
  pub collapsed: bool,
}

impl PanelPlacement {
  /// A placement for `dock`, collapsed, without a size hint.
  pub fn docked(dock: DockPlacement) -> Self {
    Self {
      dock,
      size: None,
      collapsed: true,
    }
  }

  /// Sets a preferred dock size.
  pub fn with_size(mut self, size: woocraft::gpui::Pixels) -> Self {
    self.size = Some(size);
    self
  }

  /// Marks the dock as expanded in the default layout.
  pub fn expanded(mut self) -> Self {
    self.collapsed = false;
    self
  }
}

/// The role a contributed panel plays in the workbench shell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelRole {
  /// Part of the default layout and not closable by the user. The host
  /// composes these (in extension registration order) into its default
  /// dock layout using the carried placement hint.
  Persistent(PanelPlacement),
  /// Opened on demand — possibly many instances at once, each built with
  /// its own per-instance state — and closable at any time. Where a
  /// transient panel lands is decided by whoever opens it, not by the
  /// host.
  Transient,
}

/// A panel an extension contributes to the workbench shell.
#[derive(Clone)]
pub struct PanelContribution {
  /// The stable registration key — the contribution's
  /// [`woocraft::Panel::panel_name`]. It must never change once layouts
  /// containing it have been persisted.
  pub name: SharedString,
  /// Whether the panel is part of the default layout or opened on demand.
  pub role: PanelRole,
  /// Builds the panel entity. Called on the main thread; may be called
  /// multiple times (default layout, on-demand opens, and every restore of
  /// a saved one).
  pub build: PanelBuilder,
}

/// Panels contributed by the registered extensions, in extension
/// registration order. The host composes its default dock layout from
/// these.
pub fn contributions() -> Vec<PanelContribution> {
  crate::extension::extensions()
    .iter()
    .flat_map(|extension| extension.panels())
    .collect()
}

/// Builds a fresh instance of the contributed panel `name` without
/// per-instance state (the default-layout path); `None` when no registered
/// extension contributes a panel under that name.
pub fn build(
  name: &str, dock_area: WeakEntity<DockArea>, window: &mut Window, cx: &mut App,
) -> Option<Arc<dyn PanelView>> {
  build_with_state(name, serde_json::Value::Null, dock_area, window, cx)
}

/// [`build`] with per-instance state: the JSON value is handed to the
/// builder as the panel's [`PanelInfo`] payload, the same shape a
/// persisted [`Panel::dump`](woocraft::Panel::dump) writes. This is the
/// on-demand open path for transient panels.
pub fn build_with_state(
  name: &str, state: serde_json::Value, dock_area: WeakEntity<DockArea>, window: &mut Window,
  cx: &mut App,
) -> Option<Arc<dyn PanelView>> {
  let contribution = contributions().into_iter().find(|c| c.name == name)?;
  let panel_state = PanelState {
    panel_name: name.to_string(),
    children: Vec::new(),
    info: PanelInfo::Panel(state),
  };
  let panel = (contribution.build)(dock_area, &panel_state, &panel_state.info, window, cx);
  Some(Arc::from(panel))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::extension::{Extension, ExtensionMetadata};

  struct StubExtension {
    metadata: ExtensionMetadata,
    panels: Vec<PanelContribution>,
  }

  impl Extension for StubExtension {
    fn metadata(&self) -> &ExtensionMetadata {
      &self.metadata
    }

    fn panels(&self) -> Vec<PanelContribution> {
      self.panels.clone()
    }
  }

  fn contribution(name: &str, role: PanelRole) -> PanelContribution {
    PanelContribution {
      name: name.into(),
      role,
      build: Arc::new(|_dock, _state, _info, _window, _cx| {
        unreachable!("the builder is opaque to the host and never called here")
      }),
    }
  }

  #[test]
  fn contributions_come_from_extensions_in_order() {
    crate::extension::register_extension(Arc::new(StubExtension {
      metadata: ExtensionMetadata {
        id: "panel-test.roles".into(),
        name: "Roles".into(),
        vendor: "craftoria".into(),
        version: "0.0.0".into(),
      },
      panels: vec![
        contribution(
          "test.panel-persistent",
          PanelRole::Persistent(PanelPlacement::docked(DockPlacement::Left)),
        ),
        contribution("test.panel-transient", PanelRole::Transient),
      ],
    }));

    let all = contributions();
    let names: Vec<String> = all.iter().map(|c| c.name.to_string()).collect();
    assert!(names.contains(&"test.panel-persistent".to_string()));
    assert!(names.contains(&"test.panel-transient".to_string()));
    // Registration order is preserved across the flat map.
    let a = names.iter().position(|n| n == "test.panel-persistent");
    let b = names.iter().position(|n| n == "test.panel-transient");
    assert!(a < b);
  }

  #[test]
  fn placements_carry_size_and_expansion() {
    let placement = PanelPlacement::docked(DockPlacement::Left);
    assert_eq!(placement.dock, DockPlacement::Left);
    assert_eq!(placement.size, None);
    assert!(placement.collapsed, "new docks start collapsed");

    let placement = placement.with_size(woocraft::gpui::px(260.)).expanded();
    assert_eq!(placement.size, Some(woocraft::gpui::px(260.)));
    assert!(!placement.collapsed);
  }
}
