//! The craftoria i18n backbone.
//!
//! Everything builds on woocraft's i18n infrastructure rather than a second
//! rust-i18n backend: locale catalogs are injected through
//! [`woocraft::extend_locale`], so woocraft's locale normalization
//! (`zh-cn` → `zh-hans`, `zh-tw` → `zh-hant`, …), fallback chain, display-name
//! lookup, and the protection of its own `tech.woooo.woocraft` keys apply
//! unchanged. Craftoria code never invokes a second `rust_i18n::i18n!`
//! backend; there is exactly one registration path for built-in modules and
//! future plugins alike.
//!
//! # Domains
//!
//! The i18n domain is the reverse of <https://craft.woooo.tech>:
//!
//! - [`CRAFTORIA_I18N_DOMAIN`] (`tech.woooo.craft`) — core workbench strings;
//! - `tech.woooo.craft.ext.<id>` — strings contributed by the extension `<id>`,
//!   built with [`craftoria_ext_key`].
//!
//! # Catalogs
//!
//! A [`LocaleCatalog`] is a flat map of fully-qualified dotted keys to
//! strings, parsed from TOML whose table headers spell the domain:
//!
//! ```toml
//! [tech.woooo.craft.panel.logs]
//! title = "Logs"
//! ```
//!
//! parses to the key `tech.woooo.craft.panel.logs.title`. Keys outside the
//! craftoria domain tree are rejected at parse time, so a typo fails loudly
//! at startup instead of rendering raw keys in the UI.

use std::{
  collections::BTreeMap,
  sync::{LazyLock, RwLock},
};

use thiserror::Error;
use woocraft::gpui::{App, SharedString};

/// The craftoria i18n domain: the reverse of <https://craft.woooo.tech>.
pub const CRAFTORIA_I18N_DOMAIN: &str = "tech.woooo.craft";

/// The locales craftoria ships translations for, in their canonical (already
/// normalized) spelling. `zh-cn`/`zh-sg` map to `zh-hans`, `zh-tw`/`zh-hk`
/// to `zh-hant` through woocraft's locale normalization.
pub const SUPPORTED_LOCALES: [&str; 4] = ["zh-hans", "zh-hant", "ja-jp", "en-us"];

/// Whether `key` is `tech.woooo.craft` or lives below it.
fn is_craftoria_domain_key(key: &str) -> bool {
  key == CRAFTORIA_I18N_DOMAIN
    || key
      .strip_prefix(CRAFTORIA_I18N_DOMAIN)
      .is_some_and(|rest| rest.starts_with('.'))
}

/// Builds a core-domain i18n key: `craftoria_key("panel.logs.title")` →
/// `tech.woooo.craft.panel.logs.title`. Already-prefixed keys pass through.
pub fn craftoria_key(key: impl AsRef<str>) -> String {
  let key = key.as_ref();
  if is_craftoria_domain_key(key) {
    key.to_string()
  } else {
    format!("{CRAFTORIA_I18N_DOMAIN}.{key}")
  }
}

/// Builds an extension-domain i18n key:
/// `craftoria_ext_key("terminal", "panel.title")` →
/// `tech.woooo.craft.ext.terminal.panel.title`.
pub fn craftoria_ext_key(extension_id: impl AsRef<str>, key: impl AsRef<str>) -> String {
  format!(
    "{CRAFTORIA_I18N_DOMAIN}.ext.{}.{}",
    extension_id.as_ref(),
    key.as_ref()
  )
}

/// Failures while parsing a locale catalog.
#[derive(Debug, Error)]
pub enum CatalogError {
  #[error("failed to parse the {locale} catalog: {source}")]
  Parse {
    locale: String,
    #[source]
    source: toml::de::Error,
  },
  #[error("invalid value at `{key}` in the {locale} catalog: expected a string, got {kind}")]
  NotAString {
    locale: String,
    key: String,
    kind: &'static str,
  },
  #[error("key `{key}` in the {locale} catalog is outside the {CRAFTORIA_I18N_DOMAIN} i18n domain")]
  OutOfDomain { locale: String, key: String },
}

/// A parsed locale catalog: fully-qualified dotted keys to translated
/// strings for one locale.
///
/// Build one from TOML with [`LocaleCatalog::from_toml`] (see the
/// [module docs](self) for the format), then hand it to
/// [`register_catalog`] — directly for the core workbench, or through an
/// [`Extension`](crate::extension::Extension) for anything plugin-shaped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocaleCatalog {
  locale: String,
  translations: BTreeMap<String, String>,
}

impl LocaleCatalog {
  /// An empty catalog for `locale`.
  pub fn new(locale: impl Into<String>) -> Self {
    Self {
      locale: locale.into(),
      translations: BTreeMap::new(),
    }
  }

  /// Parses a catalog from TOML. Nested tables flatten into dotted keys;
  /// leaf values must be strings; every key must live inside the craftoria
  /// domain tree.
  pub fn from_toml(locale: impl Into<String>, text: &str) -> Result<Self, CatalogError> {
    let locale = locale.into();
    let table = match toml::from_str::<toml::Table>(text) {
      Ok(table) => table,
      Err(source) => return Err(CatalogError::Parse { locale, source }),
    };

    let mut translations = BTreeMap::new();
    let mut stack: Vec<(String, &toml::Table)> = Vec::new();
    stack.push((String::new(), &table));
    while let Some((prefix, table)) = stack.pop() {
      for (segment, value) in table {
        let key = if prefix.is_empty() {
          segment.clone()
        } else {
          format!("{prefix}.{segment}")
        };
        match value {
          toml::Value::String(text) => {
            if !is_craftoria_domain_key(&key) {
              return Err(CatalogError::OutOfDomain {
                locale: locale.clone(),
                key,
              });
            }
            translations.insert(key, text.clone());
          }
          toml::Value::Table(nested) => {
            stack.push((key, nested));
          }
          other => {
            return Err(CatalogError::NotAString {
              locale: locale.clone(),
              key,
              kind: describe_value(other),
            });
          }
        }
      }
    }

    Ok(Self {
      locale,
      translations,
    })
  }

  /// The catalog locale, in whatever spelling it was created with.
  pub fn locale(&self) -> &str {
    &self.locale
  }

  /// The flattened translations: fully-qualified dotted keys to strings.
  pub fn translations(&self) -> &BTreeMap<String, String> {
    &self.translations
  }

  /// Consumes the catalog into its translations.
  pub fn into_translations(self) -> BTreeMap<String, String> {
    self.translations
  }
}

/// Describes a non-string TOML leaf for [`CatalogError::NotAString`].
fn describe_value(value: &toml::Value) -> &'static str {
  match value {
    toml::Value::String(_) => "a string",
    toml::Value::Integer(_) => "an integer",
    toml::Value::Float(_) => "a float",
    toml::Value::Boolean(_) => "a boolean",
    toml::Value::Datetime(_) => "a datetime",
    toml::Value::Array(_) => "an array",
    toml::Value::Table(_) => "a table",
  }
}

/// Registers a catalog into the shared translation pool. The locale is
/// normalized by woocraft (`zh-cn` → `zh-hans`, …) and the keys merge over
/// previously registered ones for the same locale. Catalog registration is
/// what invalidates the [`tr_static`] cache, so late-registered catalogs
/// become visible on the next render.
pub fn register_catalog(catalog: LocaleCatalog) {
  let locale = catalog.locale.clone();
  let count = catalog.translations.len();
  woocraft::extend_locale(locale.clone(), catalog.into_translations());
  clear_static_cache();
  tracing::debug!(locale = %locale, keys = count, "registered a locale catalog");
}

/// Translates a core-domain key in the active locale; returns the key
/// itself when no translation exists. `tr("panel.logs.title")` resolves
/// `tech.woooo.craft.panel.logs.title`; already-prefixed keys pass through.
pub fn tr(key: impl AsRef<str>) -> String {
  woocraft::translate(craftoria_key(key))
}

/// [`tr`] without the missing-translation fallback: `None` when the
/// key has no entry in the active locale's fallback chain.
pub fn try_tr(key: impl AsRef<str>) -> Option<String> {
  woocraft::try_translate(craftoria_key(key))
}

/// [`tr`] with `%{name}` placeholder substitution, the interpolation
/// convention of the underlying rust-i18n catalogs:
///
/// ```
/// # craftoria_exthost::i18n::tr_with("status_bar.log_lines", &[("count", "3")]);
/// // "status_bar.log_lines" = "%{count} log lines"  →  "3 log lines"
/// ```
pub fn tr_with(key: impl AsRef<str>, args: &[(&str, &str)]) -> String {
  let mut text = tr(key);
  for (name, value) in args {
    text = text.replace(&format!("%{{{name}}}"), value);
  }
  text
}

/// A cache entry of the hot-path translation cache, keyed by
/// `(locale, key)`.
type CachedTranslation = (String, String, SharedString);

/// Cache of core-domain [`tr_static`] lookups, mirroring woocraft's
/// `translate_static` cache. Cleared whenever the active locale changes or
/// a catalog is registered, so a cached value can never outlive the
/// translation data it was built from.
static STATIC_TRANSLATION_CACHE: LazyLock<RwLock<Vec<CachedTranslation>>> =
  LazyLock::new(|| RwLock::new(Vec::new()));

fn clear_static_cache() {
  STATIC_TRANSLATION_CACHE
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
    .clear();
}

/// [`tr`] for render hot paths: returns a cheaply cloneable
/// [`SharedString`] and serves repeats from a per-`(locale, key)` cache.
/// The panel `title`/`tab_name` methods that run on every frame should use
/// this; everything else can afford [`tr`].
pub fn tr_static(key: &'static str) -> SharedString {
  let locale = woocraft::locale();
  let locale: &str = &locale;

  {
    let cache = STATIC_TRANSLATION_CACHE
      .read()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, _, value)) = cache
      .iter()
      .find(|(cached_locale, cached_key, _)| cached_locale == locale && cached_key == key)
    {
      return value.clone();
    }
  }

  let value = SharedString::from(tr(key));
  STATIC_TRANSLATION_CACHE
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
    .push((locale.to_string(), key.to_string(), value.clone()));
  value
}

/// Switches the active locale and redraws every window, using the same
/// refresh mechanism woocraft's theme switching relies on. Locale changes
/// must go through this function — it also invalidates the
/// [`tr_static`] cache — instead of calling `woocraft::set_locale` directly.
pub fn set_locale(locale: impl AsRef<str>, cx: &mut App) {
  woocraft::set_locale(locale.as_ref());
  clear_static_cache();
  cx.refresh_windows();
  let active = woocraft::locale();
  tracing::info!(locale = %*active, "switched the active locale");
}

/// The active locale, in its canonical spelling.
pub fn locale() -> String {
  woocraft::locale().to_string()
}

/// Every locale that has translations available: woocraft's built-ins plus
/// everything registered through [`register_catalog`].
pub fn available_locales() -> Vec<String> {
  woocraft::available_locales()
}

/// The display name of a locale (for language pickers), resolved through
/// woocraft's built-in `i18n.name` entries.
pub fn locale_display_name(locale: impl AsRef<str>) -> String {
  woocraft::locale_display_name(locale)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A locale unknown to woocraft's normalizer, so tests never touch the
  /// real translation pool of the four supported locales.
  const TEST_LOCALE: &str = "craftoria-test";

  /// Serializes the tests that mutate the *active* locale; without this,
  /// parallel restores race with lookups (woocraft's own test suite uses
  /// the same guard).
  static LOCALE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

  /// Registers a catalog under [`TEST_LOCALE`].
  fn register_test_catalog(toml: &str) {
    let catalog = LocaleCatalog::from_toml(TEST_LOCALE, toml).expect("test catalog parses");
    register_catalog(catalog);
  }

  #[test]
  fn craftoria_key_prefixes_and_passes_through() {
    assert_eq!(
      craftoria_key("panel.logs.title"),
      "tech.woooo.craft.panel.logs.title"
    );
    assert_eq!(
      craftoria_key("tech.woooo.craft.panel.logs.title"),
      "tech.woooo.craft.panel.logs.title"
    );
    // Anything sharing only the prefix is a different domain and must be
    // re-prefixed, not passed through.
    assert_eq!(
      craftoria_key("tech.woooo.craftoria.title"),
      "tech.woooo.craft.tech.woooo.craftoria.title"
    );
  }

  #[test]
  fn ext_key_builds_the_extension_subtree() {
    assert_eq!(
      craftoria_ext_key("terminal", "panel.title"),
      "tech.woooo.craft.ext.terminal.panel.title"
    );
  }

  #[test]
  fn from_toml_flattens_nested_tables_into_dotted_keys() {
    let catalog = LocaleCatalog::from_toml(
      TEST_LOCALE,
      r#"
      [tech.woooo.craft.panel.logs]
      title = "Logs"

      [tech.woooo.craft.status_bar]
      tips = "Tips"
      "#,
    )
    .expect("catalog parses");

    assert_eq!(
      catalog
        .translations()
        .get("tech.woooo.craft.panel.logs.title"),
      Some(&"Logs".to_string())
    );
    assert_eq!(
      catalog
        .translations()
        .get("tech.woooo.craft.status_bar.tips"),
      Some(&"Tips".to_string())
    );
  }

  #[test]
  fn from_toml_rejects_non_string_leaves() {
    let error = LocaleCatalog::from_toml(TEST_LOCALE, "[tech.woooo.craft.status_bar]\ncount = 3\n")
      .expect_err("integer leaf must be rejected");

    assert!(matches!(
      error,
      CatalogError::NotAString {
        ref key,
        kind: "an integer",
        ..
      } if key == "tech.woooo.craft.status_bar.count"
    ));
  }

  #[test]
  fn from_toml_rejects_keys_outside_the_domain() {
    let error = LocaleCatalog::from_toml(TEST_LOCALE, "[somewhere.else]\ntitle = \"Logs\"\n")
      .expect_err("out-of-domain key must be rejected");

    assert!(matches!(
      error,
      CatalogError::OutOfDomain {
        ref key,
        locale: ref l,
      } if key == "somewhere.else.title" && l == TEST_LOCALE
    ));
  }

  #[test]
  fn from_toml_reports_syntax_errors_with_the_locale() {
    let error =
      LocaleCatalog::from_toml(TEST_LOCALE, "[tech.woooo.craft").expect_err("must fail to parse");
    assert!(matches!(error, CatalogError::Parse { ref locale, .. } if locale == TEST_LOCALE));
  }

  #[test]
  fn registered_catalogs_translate_through_woocraft() {
    register_test_catalog("[tech.woooo.craft.greeting]\nhello = \"Hi\"\n");

    // The merged lookup sees the injected key under the test locale …
    assert_eq!(
      woocraft::translate_in_locale(TEST_LOCALE, "tech.woooo.craft.greeting.hello"),
      "Hi"
    );
    // … while woocraft's own domain stays untouchable from outside.
    assert_ne!(
      woocraft::translate_in_locale(TEST_LOCALE, "tech.woooo.woocraft.common.loading"),
      "Hi"
    );
  }

  #[test]
  fn tr_with_substitutes_placeholders() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    // `tr_with` uses the *active* locale, so pin it for the duration of
    // the test and restore the default afterwards.
    register_test_catalog("[tech.woooo.craft.count]\nline = \"%{count} lines, %{state}\"\n");
    woocraft::set_locale(TEST_LOCALE);
    let text = tr_with("count.line", &[("count", "3"), ("state", "streaming")]);
    woocraft::set_locale("en-us");
    assert_eq!(text, "3 lines, streaming");
  }

  #[test]
  fn tr_static_caches_and_follows_catalog_registration() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    const HELLO: &str = "tech.woooo.craft.greeting.static_hello";
    const LATER: &str = "tech.woooo.craft.greeting.registered_later";

    register_test_catalog("[tech.woooo.craft.greeting]\nstatic_hello = \"Hi\"\n");
    woocraft::set_locale(TEST_LOCALE);
    assert_eq!(tr_static(HELLO), "Hi");

    // A late registration invalidates the cache, so the new key resolves
    // on the very next lookup without a locale change.
    register_test_catalog("[tech.woooo.craft.greeting]\nregistered_later = \"There\"\n");
    assert_eq!(tr_static(HELLO), "Hi");
    assert_eq!(tr_static(LATER), "There");

    woocraft::set_locale("en-us");
  }
}
