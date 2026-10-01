#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! The extension host for craftoria.
//!
//! This crate is the foundation the workbench's future plugin system grows
//! from, and it owns the shared i18n backbone both the built-in UI and the
//! extensions use:
//!
//! - [`extension`] — the [`Extension`] contract, the registry, and the startup
//!   flush that feeds contributions into the host;
//! - [`i18n`] — the `tech.woooo.craft` translation domain, built on woocraft's
//!   i18n infrastructure (see [`i18n`] for why there is exactly one
//!   registration path and no second rust-i18n backend).
//!
//! The host deliberately stays free of filesystem and UI-framework
//! assumptions beyond gpui's `App`: embedding, loading, and persisting
//! anything is the embedding application's job. That keeps the door open
//! for dynamic plugin loading without redesigning the contribution APIs.

pub mod extension;
pub mod i18n;
