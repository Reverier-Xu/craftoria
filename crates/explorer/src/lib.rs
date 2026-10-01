#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! The workspace explorer of craftoria.
//!
//! The first real extension built on [`craftoria_exthost`]: it contributes
//! a persistent, lazily loaded file-tree panel and transient,
//! multi-instance directory-listing panels, plus the locale catalogs for
//! both. Everything flows through the extension host — panels, strings,
//! and the dock-area access needed to open sibling panels — so the crate
//! doubles as the reference shape for future extensions.
//!
//! The composition root (the `craftoria` shell) registers
//! [`ExplorerExtension`] alongside the workbench's own extension before
//! the first window opens.

pub mod dir_list;
pub mod extension;
pub mod fs_tree;
mod panel;

pub use extension::ExplorerExtension;
