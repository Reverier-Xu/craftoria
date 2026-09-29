English | [中文](README.cn.md)

# Craftoria

Craftoria is a one-stop multimedia workstation platform: 3D modeling, rendering, audio/video processing, and image processing, delivered as a family of Rust crates under a single workspace.

> [!WARNING]
>
> craftoria is in a very early stage. The workspace currently ships the unified
> command-line shell only; the media domain crates land incrementally.

## Planned Domains

- **3D modeling** — mesh and scene modeling toolchain
- **Rendering** — interactive and offline rendering pipelines
- **Audio/Video** — decoding, encoding, editing, and processing pipelines
- **Image processing** — raster and vector image tooling

The website lives at [craft.woooo.tech](https://craft.woooo.tech); future
capabilities will be exposed under its subdomains.

## Code Organization

The repository is a Cargo workspace. Domain crates are named
`craftoria-<domain>`; the user-facing binary keeps the plain `craftoria` name.

```text
crates/
  shell      The `craftoria` binary: unified command-line entry point,
             published to crates.io
```

## Installation

From crates.io:

```bash
cargo install craftoria
```

or as a dependency:

```toml
[dependencies]
craftoria = "0.1"
```

## Usage

```bash
craftoria --help
craftoria info
```

## Development

```bash
cargo +nightly fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

See [AGENTS.md](AGENTS.md) for the full contribution conventions and quality
gates.

## License

Licensed under the GNU General Public License v2.0 or later
([LICENSE](LICENSE)).
