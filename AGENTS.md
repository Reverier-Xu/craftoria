# AGENTS.md

本文件为协助在本仓库中执行自动化 agent（或人工协同）任务的快速指南，包含编码原则、本地可用工具、常见工作流、重要路径与安全/构建注意事项。所有要求对每次任务、提交与 PR 均为强制标准，除非获得用户明确批准，否则不允许例外。

## 编码原则

实现原则：

1. **长期主义原则**：做长期正确的事，而非追求短期修补。长期正确性定义为：在给定目标下，沿时间维度积分成本最低的决策，而非当前时刻局部成本最低的决策。短期简单通常对应解空间中的局部极小值，其隐性成本会以路径依赖和未来修正开销的形式延后显现。长期正确要求承担一次性必要的结构成本，以换取未来更大的决策自由。
2. **优雅实现优先原则**：保持简单、实用、避免过度设计。优雅定义为：在给定长期目标与固定信息量下，熵最低的方案。

思考原则：

- 默认使用第一性原理思考，拒绝经验主义与路径依赖；不要假设用户完全理解自己的目标；保持谨慎，从原始需求与底层问题出发推理。若目标模糊，先停下来与用户讨论；若目标清晰但路径次优，直接给出更短、成本更低的替代方案。
- 识别用户表述中的隐含假设。若前提有缺陷，先纠正前提再回答。尽量用数字而非形容词；给出明确判断，而不是两面讨好。

与用户的关系：忠诚于**事实**，而非用户的期望。质疑用户观点时保持尊重但不退让；若用户给出更好的事实或推理，立即修正结论，不为立场辩护。

## 本地常用工具

请在需要使用工具时优先使用以下工具，不要使用 grep、find、cat。

- **ripgrep（rg）**：快速全文搜索源码与文本。
- **fd**：快速查找文件（比 find 更友好）。
- **bat**：替代 `cat`，带语法高亮与分页，阅读完整文件时建议使用 `bat -Pp`。
- **eza**：替代 `ls`。
- **fish**：默认 shell 为 fish，运行脚本或演示命令时请使用 fish 语法。

## 仓库概览（要点）

- 本仓库是一个 Rust workspace：**craftoria，一站式多媒体工作平台**，规划覆盖 3D 建模、渲染、音视频处理、图像处理等领域。
- 站点主页：<https://craft.woooo.tech>，后续各能力以子域形式挂在该域下。
- crate 组织约定：按领域拆分为 `crates/<name>`，包名 `craftoria-<name>`；面向用户的统一 CLI 包名固定为 `craftoria`，当前位于 `crates/shell`。
  - `crates/shell`（包名 `craftoria`）：平台统一命令行入口，发布到 crates.io。`src/cli.rs` 定义 CLI 结构，`src/commands/` 下按子命令分模块，`src/error.rs` 为错误类型。
- 其余领域 crate（如 `craftoria-base`、渲染 / 音视频 / 图像等）按规划逐步新增，命名一律遵循 `craftoria-*`。
- 文档：根 `README.md`（英文）与 `README.cn.md`（中文）；各 crate 携带自己的 `README.md`。
- 构建输出位于 `target/`，不要提交。

## 关键技术约束

- edition 2024、rust-version 1.98；代码注释与文档使用英文。
- 所有依赖统一声明在根 `Cargo.toml` 的 `[workspace.dependencies]`，成员 crate 通过 `{ workspace = true }` 引用；新增依赖需说明必要性，禁止引入未使用或超出任务范围的依赖。
- `unsafe` Rust 一律禁止（workspace lints `unsafe_code = "forbid"`）。
- 生产代码禁止 `unwrap()` / `expect()`（workspace clippy deny）；测试代码通过 `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]` 豁免。
- 错误处理：用 `thiserror` 定义语义化错误枚举并携带上下文，禁止用裸字符串错误吞掉上下文。
- 日志统一走 `tracing`；日志输出到 stderr，stdout 保留给命令输出（可能被管道消费）。

## 质量门禁（提交前必须全绿）

```fish
cargo +nightly fmt --all
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features
taplo fmt
```

- 格式化依赖 nightly 工具链（`rustfmt.toml` 启用了 unstable 选项：2 空格缩进、`PreferSameLine`、crate 粒度 import 分组等）。如若影响到无关文件，无需在意，保持格式化状态。
- TOML 文件用 taplo 格式化（键值与注释对齐、表内键排序）。

## 版本与发布

- 所有 crate 的版本号统一继承根 `Cargo.toml` 的 `[workspace.package] version`，发版在根 `Cargo.toml` 统一调整。
- `crates/shell`（包名 `craftoria`）发布到 crates.io；发布前先 `cargo publish --dry-run -p craftoria`，并确认工作区干净、改动已提交。
- crates.io 上的版本不可删除、不可覆盖：发布前务必核对版本号与元数据（description / homepage / license / repository / keywords）。
- Git tag 与工作区版本保持一致（不带 `v` 前缀），供后续 CI 发布流程使用。
- GitHub workflows 暂缓引入；本地质量门禁即为当前唯一门禁。

## 典型工作流

- 本地构建（debug / release）：

  ```fish
  cargo build
  cargo build --release
  ```

- 运行测试：

  ```fish
  cargo test --workspace --all-features
  ```

- 运行 CLI：

  ```fish
  cargo run -p craftoria -- info
  ```

- 提交前检查（必须全绿）：见「质量门禁」。

## 提交规范

- gitmoji + 全小写摘要 + 列表式正文；一次提交只做一件事（原子提交）。

  ```text
  :sparkles: add xxx
  - bullet 1
  - bullet 2
  ```

- 常用 gitmoji：`:tada:` 初始化、`:sparkles:` 新功能、`:bug:` 修 bug、`:memo:` 文档、`:recycle:` 重构、`:art:` 格式、`:white_check_mark:` 测试、`:wrench:` 构建/工具、`:arrow_up:` 依赖升级。
- 本仓库提交规范优先于全局 agent 钩子约定（如 bigpowers Conventional Commits）；全局 git 安全守则（保护分支、破坏性命令拦截）仍然生效。

## 注意事项与陷阱

- 本机默认 shell 为 fish —— 不要在自动化脚本中假设 bash，除非显式调用 `bash -c`。
- 并发修改多个 crate 或公共接口时，先 `cargo build` 捕获类型/接口回归。
- 跨平台：媒体领域代码涉及平台后端时，平台相关代码按 `cfg(unix)` / `cfg(windows)` 分离；本机无法直接运行其它平台测试时，用 `cargo check --target <t>` 做交叉编译检查。
- 禁止 `git reset --hard`、`git clean -fd`、`rm -rf` 等破坏性命令，除非用户明确批准。
- 工作区必须在任务结束时保持干净：无未跟踪文件、无未提交改动、无临时产物。

## 任务结束检查清单

1. `cargo +nightly fmt --all -- --check` 通过。
2. `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` 零警告。
3. `cargo test --workspace --all-features` 全部通过。
4. 提交符合 gitmoji + 全小写摘要 + 列表正文规范，且为原子提交。
5. 工作区干净：无未跟踪文件、无未提交改动。
6. 文件操作优先使用 fd / rg / bat / eza。
