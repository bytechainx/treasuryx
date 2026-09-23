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

## E2E 全公开面覆盖核对（手工，**不进 CI**）

除四件套外，本仓有一条**公开面覆盖**核对：`tests/e2e_treasury.rs` 必须把本仓**核对器口径内的全部公开条目**
逐条真实执行一遍。核对器在**元仓库根目录**执行：

```bash
cd /home/workspace/bytechainx
node scripts/verify-e2e-coverage.mjs treasuryx \
  --target-dir /home/workspace/bytechainx/.cargo/e2e-cov/treasuryx
```

- 退出码：**0** 全过 / **1** 有发现（含未覆盖）/ **2** 工具自身或环境错误（缺工具时**一律 2**，不降级成「通过」）
- 三层判据，缺一不可：① 权威公开面由 `cargo +nightly public-api --simplified` 派生（**不采信测试自述**）；
  ② 测试内 `E2E_MANIFEST`（`const E2E_MANIFEST: &[(&str, &str)]`）与权威公开面**双向 diff**
  （少一条 = missing、多一条 = ghost，都判红）；③ `-C instrument-coverage` + `cargo-llvm-cov` **按函数**
  取执行次数，每条公开 `fn` 的 count 必须 > 0
- **本仓权威条数（独立复算，谓词 = `cargo +nightly public-api --simplified` + 核对器提取口径）**：
  **156** 条 = `type` 24 / `variant` 65 / `field` 23 / `const` 13 / `fn` 31
- **为何本仓特别需要它**：`treasuryx` 是**公开模块**（`pub mod`）形态的仓 —— `src/lib.rs` 有 5 个
  `pub mod`（`authz` / `error` / `parse` / `pit` / `value`）；`cargo public-api` 对这类仓的定义行与固有
  impl 方法**一律带模块段**（`pub fn treasuryx::value::Date::new(…)`、`impl treasuryx::value::Date`），
  只有门面 `pub use` 再导出的项才是 crate 根形态。核对器在 `pub mod` 形态上曾**同时漏项与造幽灵**，
  且**双向 diff 恒绿**（两侧都由它自己派生）⇒ **跑之前先确认取到的是已修该形态（F4）的核对器版本**
- **口径边界（**不得**当成「已覆盖全部公开接口」）**：
  - **枚举的结构体变体字段**（**两级嵌套**，如 `TreasuryAuthorization::Authorized::scope` /
    `TreasuryAuthorization::Denied::reason`）**不在**提取口径内 ⇒ 应写「**未登记，故三层判据不保护**」，
    **不得**写「未覆盖」—— 这些字段在行为上确实被测到（构造 / 穷尽解构 / 真读其值），只是不在权威
    公开面的提取子集里；删字段或改名时核对器**不报**，只能靠**编译失败**兜底。这是核对器的**全局口径
    下界**，不是 `treasuryx` 一个仓的问题
  - **元组结构体的公开字段是单级**（在口径内、**必须**登记），与本条的两级嵌套不同
  - derive / auto impl 不计入（`clone` / `eq` / `fmt` / `serialize` …）⇒ 口径实为「公开**条目**（子集）」，
    故正确表述是「已覆盖**核对器口径内的**全部公开条目（156 条）」
- 另需外部工具 `cargo +nightly public-api` / `cargo-llvm-cov` / `rustfilt`；口径与判定细节（含 F1–F4
  四类假绿）见元仓库 `scripts/AGENTS.md` §2.1.1

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
