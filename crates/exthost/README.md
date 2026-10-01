# craftoria-exthost

The extension host for [craftoria](https://craft.woooo.tech): the foundation of
the upcoming plugin system and the shared i18n backbone of the workbench.

## Extension skeleton

The crate defines the minimal [`Extension`] contract that every craftoria
extension — built-in module or future dynamic plugin — satisfies:

- stable identity ([`ExtensionMetadata`]),
- locale catalogs contributed to the shared translation pool.

Registration is static today (`register_extension`), but the API is shaped so a
dynamic loader can register plugins through the exact same path: contributions
flow through one registry and one flush (`init`), never through ad-hoc
shortcuts.

## i18n

Translations build on woocraft's i18n infrastructure instead of a second
rust-i18n backend: catalogs are injected through `woocraft::extend_locale`, so
woocraft's locale normalization (`zh-cn` → `zh-hans`, …), fallback chain,
display-name lookup, and host-app protection all apply unchanged.

- Domain: `tech.woooo.craft` (reverse of <https://craft.woooo.tech>);
  extensions get `tech.woooo.craft.ext.<id>`.
- Catalogs are TOML files whose table headers spell fully-qualified keys:
  `[tech.woooo.craft.panel.logs]` → key `tech.woooo.craft.panel.logs.title`.
- `tr`/`tr_with`/`tr_static` translate core-domain keys; `tr_static` caches
  `SharedString`s for render hot paths and is invalidated on locale changes
  and catalog registration.
