# treasuryx Agent 指南

> 本文件为 AI Agent 在本仓库工作时的入口指南。

## 项目定位

美国财政部源事实的**离线类型层**：数据集常量与访问器、采集面守卫、曲线路由守卫、
写入主权守卫、NY Fed `NO-GO` 守卫、记录值对象与信封解析、fail-closed 授权判定。
**不实现联网采集或 live**，不实现派生指标，不做单位换算，不成为应用的组合根。

采集范围权威：`specs/adapter/treasury.md`；跨源语义权威：`contracts/cross-source-routing.md`。

## 技术栈

- Rust edition 2021, rust-version 1.71（= 依赖图 `rust_version` 最大值，推导见 `CONTEXT.md`）
- 关键依赖：`thiserror` 2、`serde` 1（`derive`）、`serde_json` 1
- 无 path 依赖；不依赖任何 `bytechainx/*` crate
- crate 级 lint：`unwrap_used` / `expect_used` / `panic` / `unreachable` / `todo` / `unimplemented`
  全部 `deny`（测试代码经 `cfg_attr(test)` 豁免）

## 代码结构

```text
src/
├── lib.rs        # crate 文档 + 模块声明 + 门面 pub use
├── error.rs      # TreasuryError / TreasuryErrorKind / TreasuryResult
├── value.rs      # Date / Period / Frequency / Unit（基础值对象）
├── value/
│   ├── datasets.rs # 三个 ID 枚举、常量集与三类守卫
│   └── record.rs   # TreasuryRecord / TreasuryAmount / validate_record
├── authz.rs      # TreasuryAuthorization / TreasuryScope / authorize_treasury
├── pit.rs        # TimePrecision / AvailabilityEvidence / PitEligibility
└── parse.rs      # parse_treasury_records（离线、无网络参数）
```

依赖方向单向：`error` ← `value` ← {`authz`, `pit`, `parse`}；`error` 不依赖任何同级模块。
禁止建 `mod.rs`，禁止建 `utils` / `helpers` / `common` / `manager` / `base` / `global` / `misc`。

## 开发约定

- 注释与文档使用简体中文；标识符保持英文
- 错误：`TreasuryError` + `#[non_exhaustive]` + `TreasuryErrorKind`；禁止公共 API 返回 `String`
- 禁止裸 `unwrap()` / `expect()` / `panic!` / `println!`（库代码；lint 已 deny）
- **禁止**引入 HTTP 客户端 / 异步运行时 / `chrono` / `time` / `rand`
- **禁止**源码出现 `https://…` 端点字面量；**禁止**读环境变量或凭据
- 曲线产品（`DS06` / `DS07`）必须路由 `yieldx`；权威 canonical key 不得由本域写入
- 缺失值必须具名表达，禁止静默转 0；`meta.count` 必须与 `data` 行数自洽
- 授权判定必须 fail-closed；`Authorized` 的 scope 必须显式声明仅覆盖 offline fixture
- 解析器参数只有字符串；未知字段原子失败；重复身份拒绝（不去重）

## 门禁四件套

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo package --no-verify
```

边界与自检：

```bash
grep -c '// TDD-PROBE:' tests/tdd_contracts.rs
grep -c '// SPEC-MAP:'  tests/sdd_spec.rs
grep -c '// AIDD:'      tests/aidd_boundary.rs
a=$(grep -c '^## ' docs/标准.md); b=$(grep -c '// SPEC-MAP:' tests/sdd_spec.rs)
echo "SPEC-MAP 1:1: 标准.md=$a sdd_spec.rs=$b"   # 必须相等
grep -rn 'https\?://' src || echo "0 端点字面量"
grep -rnE 'env::var|from_env' src || echo "0 凭据"
```

## 三类测试

| 文件 | 头部标记 | 要求 |
| --- | --- | --- |
| `tests/tdd_contracts.rs` | `// TDD-PROBE:` | 四列（入口 / 变异 / 红 / 绿），覆盖全部公开入口，无空列 |
| `tests/sdd_spec.rs` | `// SPEC-MAP:` | 与 `docs/标准.md` 的 `##` 章节 1:1 |
| `tests/aidd_boundary.rs` | `// AIDD:` | ≥5 条，五列非空（含 `复核=`） |

夹具 `tests/fixtures/*.json` 必须带 `_synthetic` 与 `_note` 标注；
**禁止**把合成样本表述为「实测」「核验 PASS」或任何证据等级。

## 相关文档

- 组织 Rust 规范：`~/org-config/rulesets/rust/RULES.md`
- API 文档：`docs/API.md`；标准与验收：`docs/标准.md`
- 术语与领域语言：`CONTEXT.md`；贡献指南：`CONTRIBUTING.md`
- 变更记录：`CHANGELOG.md`；基准测试：`benches/hot_path.rs`
