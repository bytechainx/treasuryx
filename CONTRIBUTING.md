# CONTRIBUTING.md — 贡献指南（treasuryx）

本文件面向贡献者，汇总本地门禁与提交约定。
AI Agent 的工作约定另见 [`AGENTS.md`](./AGENTS.md)；术语与领域语言见 [`CONTEXT.md`](./CONTEXT.md)。

## 开发流程

- 本仓库是**独立的单 crate 仓库**，不依赖 `xhyper.rs` 主工程及其内部 crate；
  **没有任何 path 依赖**，也不依赖任何 `bytechainx/*` crate。
- substantial 变更走 feature branch → PR → review → merge，**禁止直接 push `main`**。
- `main` 已启用分支保护：要求 PR + 必需检查 `fmt / clippy / test`，
  `required_approving_review_count = 0`（单人也能合并），禁止强推与删除。
- 合并方式固定为 **create a merge commit**。注意仓库设置是
  `merge_commit_title = MERGE_MESSAGE` + `merge_commit_message = PR_TITLE`，因此
  `gh pr merge` 必须显式传 `--subject` 与 `--body`，否则会产出通用
  `Merge pull request #N from …` 标题。
- 提交信息遵循 Conventional Commits（`feat:` / `fix:` / `docs:` / `ci:` / `chore:` / `refactor:`），
  描述用简体中文。

## 本地门禁（四件套）

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo package --no-verify
```

本 crate 没有 feature 开关，`--all-features` 与默认等价，但按工作区口径一律带上。
`cargo test --all-features` **不加** `--all-targets`（`--all-targets` 会漏掉 doctest）。

## 复用口径（不发布 crates.io）

- 本 crate **不发布到 crates.io**，仅以 GitHub 源码 / git 依赖形式复用。
- 文档与元数据中不得出现「可独立发布」「可直接 `cargo publish`」等表述，
  也不得放置 crates.io / docs.rs 徽章与外链。
- `Cargo.toml` 的 `documentation` 指向 `https://github.com/bytechainx/treasuryx#readme`。
- 消费方引入方式（README「安装」小节为准）：

  ```toml
  [dependencies]
  treasuryx = { git = "https://github.com/bytechainx/treasuryx" }
  ```

## 开发约定

- 注释、文档、错误消息使用**简体中文**；标识符保持英文。
- MSRV 为 Rust 1.71、edition 2021；不得使用高于该下界的语言特性或 std API。
- 错误模型：`TreasuryError`（thiserror 风格枚举）+ `#[non_exhaustive]` +
  `TreasuryErrorKind`；禁止公共 API 返回 `String` / `anyhow::Error`。
- 不在库代码里裸 `unwrap()` / `expect()` / `panic!` / `println!`
  （`[lints.clippy]` 已 `deny` 相应 lint）。
- 所有 `pub` 项必须有中文 `///` 文档（`missing_docs` 已 `deny`）。
- **禁止引入** HTTP 客户端（`reqwest` / `hyper` / `ureq` / `curl` / `isahc`）、
  异步运行时（`tokio` / `async-std`）、`chrono` / `time` / `rand`。
- **禁止**在源码中出现 `https://…` 端点字面量；**禁止**读取环境变量或凭据。
- 集成测试**必须离线运行**：样本经 `include_str!` 内联，不访问外网。
- 值对象不得实现派生指标（净流动性、TGA 脉冲、财政脉冲等）；单位换算归下游。
- **曲线产品**（`DS06` / `DS07`）MUST NOT 被洗白为普通观测；新增入口时须经曲线守卫。
- **权威 canonical key**（`WALCL` / `WTREGEN` / `RRPONTSYD` / `WRESBAL`）MUST NOT 由本域写入。
- 缺失值必须具名表达（`TreasuryMissingReason`），禁止静默转 0。
- 新增数据集前先核对 `specs/adapter/treasury.md`；**禁止**猜端点、猜字段、猜统计码。

## 提交前自检清单

- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 通过
- [ ] `cargo test --all-features` 通过（含 doctest）
- [ ] `cargo package --no-verify` 通过
- [ ] 新增 `pub` 项都有中文 `///` 文档
- [ ] `docs/标准.md` 的 `##` 章节与 `tests/sdd_spec.rs` 的 `// SPEC-MAP:` 仍 1:1
- [ ] `tests/tdd_contracts.rs` 的 `// TDD-PROBE:` 表覆盖了新增的公开入口
- [ ] 文档中无「可独立发布」/ crates.io / docs.rs 表述
- [ ] 夹具仍带 `_synthetic` 标注，且未被表述为「实测」
