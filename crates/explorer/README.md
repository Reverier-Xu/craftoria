# craftoria-explorer

The workspace explorer of [craftoria](https://craft.woooo.tech), contributed
to the workbench as the first real extension.

Two panels come with it, exercising both roles of the extension host's panel
contract:

- **Workspace tree** — a persistent, non-closable left-dock panel showing the
  current working directory as a lazy-feeling file tree (ignored directories
  like `.git`/`target`/`node_modules` are skipped, folders sort first).
- **Directory listing** — a transient, closable, multi-instance panel backed
  by woocraft's `Table`: double-clicking a folder (in the tree or in another
  listing) opens one for that subdirectory; each instance carries its
  directory in the persisted panel state, so open listings survive a layout
  save/restore.

Strings live under the `tech.woooo.craft.ext.explorer` i18n subtree and are
registered through the extension host like every other contribution.
