# craftoria-workbench

The desktop workbench application framework of craftoria, built on GPUI via the
woocraft component library.

It provides the application shell used by the `craftoria` CLI:

- a title bar with theme switching,
- a dock layout (left / center / bottom) with JSON layout persistence,
- a status bar with build, log, and platform information,
- a bottom-dock log panel that streams `tracing` events live.

`woocraft-terminal` is a declared dependency reserved for upcoming terminal
panels; it is not wired into the UI yet.
