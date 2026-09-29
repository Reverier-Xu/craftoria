English | [中文](README.cn.md)

# Craftoria

Craftoria 是一个一站式多媒体工作平台：3D 建模、渲染、音视频处理、图像处理，以单一 Cargo workspace 内的一族 Rust crate 形式交付。

> [!WARNING]
>
> craftoria 处于非常早期的阶段。当前 workspace 仅包含统一的命令行入口，
> 各媒体领域 crate 会逐步落地。

## 规划领域

- **3D 建模** —— 网格与场景建模工具链
- **渲染** —— 交互式与离线渲染管线
- **音视频** —— 解码、编码、剪辑与处理管线
- **图像处理** —— 栅格与矢量图像工具

站点位于 [craft.woooo.tech](https://craft.woooo.tech)，后续各能力将以子域形式提供。

## 代码组织

本仓库是一个 Cargo workspace。领域 crate 命名为 `craftoria-<domain>`；面向用户的二进制保留 `craftoria` 名称。

```text
crates/
  shell      `craftoria` 二进制：平台统一命令行入口，发布到 crates.io
```

## 安装

从 crates.io：

```bash
cargo install craftoria
```

或作为依赖引用：

```toml
[dependencies]
craftoria = "0.1"
```

## 使用

```bash
craftoria --help
craftoria info
```

## 开发

```bash
cargo +nightly fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

完整的贡献规范与质量门禁见 [AGENTS.md](AGENTS.md)。

## 许可证

基于 GNU General Public License v2.0 或更新版本授权
（[LICENSE](LICENSE)）。
