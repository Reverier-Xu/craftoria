# craftoria (shell)

`craftoria` is the unified binary entry point of the Craftoria multimedia
workstation platform. It dispatches to platform capabilities via subcommands.

## Subcommands

- `craftoria info`: print platform and build information.

## Design Notes

The shell only handles command parsing, logging setup, and dispatch; the
actual media capabilities (3D modeling, rendering, audio/video, image
processing) live in dedicated `craftoria-*` crates and are wired in as they
land.

Logs go to stderr so stdout stays reserved for command output (which may be
piped). The default log level is `info`; `RUST_LOG` overrides it.
