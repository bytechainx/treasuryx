# treasuryx

`treasuryx` 是美国财政部源事实的**离线类型层**：把 `specs/adapter/treasury.md` 声明的数据集、
字段与跨源路由关系表达为稳定 Rust 类型，并提供离线解析、路由与主权守卫与 fail-closed 授权判定。

- 只表达源事实：数据集 ID + 期间 + 频率 + 单位 + 具名字段值，**不含任何派生指标**
- 零网络：无 HTTP 客户端依赖、无端点字面量、不读凭据、不配代理
- 零内部耦合：不依赖任何 `bytechainx/*` crate，无 `path` 依赖
- 采集面为最小集 `DS01` + `DS05`；其余数据集一律按路由 / 声明结论拒绝
- 曲线路由硬约束：`DS06` / `DS07` → `yieldx`，**不得**洗白为普通观测
- 写入主权：`WALCL` / `WTREGEN` / `RRPONTSYD` / `WRESBAL` 的权威写入归 `fredx`，本域只对账告警
- NY Fed Markets 整体 `NO-GO`；`DS04` 为 lossy skeleton，**禁作拍卖事实源**
- 授权判定 fail-closed；`Authorized` 的 scope 显式声明**仅覆盖 offline fixture**
- publication 语义恒为 `Date` + `Inferred` + `NotEligible`

## 安装

本 crate **不发布到 crates.io**，通过 git 依赖引入：

```toml
[dependencies]
treasuryx = { git = "https://github.com/bytechainx/treasuryx" }
```

## 最小可运行示例

```rust
use treasuryx::{guard_write_authority, parse_treasury_records, TreasuryError};

fn main() -> Result<(), TreasuryError> {
    let sample = r#"{"_dataset":"DS05",
        "data":[{"record_date":"2026-08-18","tot_pub_debt_out_amt":36500000000000.0}],
        "meta":{"count":1,"total-count":1,"total-pages":1}}"#;
    let envelope = parse_treasury_records(sample)?;
    assert_eq!(envelope.records.len(), 1);
    assert!(envelope.records[0].revision.is_none(), "不得伪造修订标识");

    // 写入主权：WALCL 的权威写入归 fredx，本域不得写权威键
    assert!(matches!(
        guard_write_authority("WALCL"),
        Err(TreasuryError::WriteAuthorityDenied(_))
    ));
    Ok(())
}
```

授权判定（fail-closed）：

```rust
use treasuryx::{authorize_treasury, TreasuryAuthorization, TreasuryScope};

// 证据缺失 → 拒绝
assert!(matches!(
    authorize_treasury(TreasuryScope::OfflineFixtureOnly, None),
    TreasuryAuthorization::Denied { .. }
));
// live 须另批决策 ID + B2–B8 门禁
assert!(matches!(
    authorize_treasury(TreasuryScope::LiveCollection, None),
    TreasuryAuthorization::Denied { .. }
));
```

## 主要内容

| 类型 / 常量 | 作用 |
| --- | --- |
| `TreasuryDatasetId` / `NyFedDatasetId` / `FredValidationSeries` | 三个 ID 枚举与访问器 |
| `PHASE1_DATASETS` / `WIRE_COLLECT_DATASETS` | Phase 1 全集 / 采集面（`DS01` + `DS05`） |
| `NY_FED_DATASETS` / `NY_FED_DECISION` | NY Fed 数据集与 `NO-GO` 结论 |
| `FD_VALIDATION_SERIES` / `CANONICAL_WRITE_OWNER` | 永久校验面 / 权威写入方 `fredx` |
| `guard_curve_product` / `guard_collection_dataset` | 曲线路由与采集面守卫 |
| `guard_write_authority` / `guard_ny_fed` | 越权写入拒绝 / NY Fed `NO-GO` |
| `TreasuryRecord` / `TreasuryAmount` / `TreasuryEnvelope` | 记录、金额槽与信封 |
| `validate_record` / `parse_treasury_records` | 完整性校验与离线解析 |
| `TreasuryAuthorization` / `authorize_treasury` | fail-closed 授权判定 |
| `TimePrecision` / `AvailabilityEvidence` / `PitEligibility` | publication 语义三元组 |

## 生产误用红线

| 禁止 | 原因 |
| --- | --- |
| 把 `authorize_treasury` 的 `Authorized` 当作 live 或生产就绪 | 它只覆盖 offline fixture 范围，live 须另批 |
| 把 `parse_treasury_records` 当作采集器 | 它只解析本地字符串，无网络能力 |
| 把 `DS06` / `DS07` 当普通观测处理 | 曲线产品必须路由 `yieldx`，守卫会拒绝 |
| 在本域写 `WALCL` / `WTREGEN` / `RRPONTSYD` / `WRESBAL` | 权威写入归 `fredx`，本域只对账告警 |
| 把 `RRPONTSYD` 的 Billions 与 `WALCL` 的 Millions 混同 | 单位换算归下游，不得静默发生 |
| 把 `DS04` 当拍卖事实源 | 它是 lossy skeleton map，仅 dry-run 声明 |
| 把 `tests/fixtures/` 的数值表述为「实测」 | 它们是合成样本，不是证据 |

## 非目标

- 不做联网采集与 live（`treasury_runtime` 未落地；live 须另批决策 ID + B2–B8 门禁）
- 不做派生指标（净流动性、TGA 脉冲、财政脉冲等归 analytics）
- 不做单位换算（保留源侧单位，换算归下游 Normalize）
- 不做凭据管理、代理配置、存储、分发与调度
- 不写权威 canonical key，不做 PIT 升格

## 门禁

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo package --no-verify
```

测试全部离线：样本经 `include_str!` 内联，不访问外网，不读环境变量。

`production_decision = NO-GO`（清单 COMPLETE ≠ ship；authorization ≠ Production Ready）。

## 许可

MIT OR Apache-2.0
