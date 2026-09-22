# treasuryx 公开 API

**角色**：美国财政部源事实的离线类型层（无网络能力）

## 公开消费面

### 值对象与信封

| 类型 / 函数 | 一行语义 |
| --- | --- |
| `Date` | 业务日期（年 / 月 / 日）；`Date::new` 校验月日与闰年，`Date::parse` 严格解析 `YYYY-MM-DD` |
| `Period` | 业务期间枚举；`Period::parse_day` 得到单日期间 |
| `Frequency` | 源侧频率（7 值） |
| `Unit` | 源侧单位：`Unknown` / `BillionsOfUsd` / `MillionsOfUsd`（**不做换算**） |
| `TreasuryAmount` | 金额槽：`Present(f64)` 或 `Missing(TreasuryMissingReason)`；`present()` 缺失返回 `None` |
| `TreasuryMissingReason` | 具名缺失原因：`SourceOmitted` / `ExplicitNull` |
| `TreasuryCashBalance` | DS01 行：`account_type` / `open_today_bal` / `close_today_bal` |
| `TreasuryDebtToPenny` | DS05 行：`debt_held_public_amt` / `intragov_hold_amt` / `tot_pub_debt_out_amt` |
| `TreasuryPayload` | 数据集专用载荷（`CashBalance` / `DebtToPenny`） |
| `TreasuryRecord` | 一条源记录：数据集 + 期间 + 频率 + 单位 + 修订标识（恒 `None`）+ 载荷 |
| `TreasuryEnvelopeMeta` | 信封 `meta`：`count` / `total_count` / `total_pages` |
| `TreasuryEnvelope` | 解析结果：`records` + `meta` |
| `validate_record` | 校验记录完整性（采集面 / 单位 / 期间 / 频率 / 修订 / 载荷 / 金额） |

### 数据集常量

| 类型 / 常量 | 一行语义 |
| --- | --- |
| `TreasuryDatasetId` | Phase 1 数据集 ID（`DS01`–`DS10`）；`id()` / `frequency()` / `parse_id()` |
| `NyFedDatasetId` | NY Fed 数据集 ID（`FR01`–`FR03`）；`id()` / `frequency()` / `parse_id()` |
| `FredValidationSeries` | FRED 校验序列（`FD01`–`FD04`）；`id()` / `series_id()` / `label()` / `canonical_write_owner()` / `parse_id()` |
| `PHASE1_DATASETS` / `WIRE_COLLECT_DATASETS` | Phase 1 全集 / 本域采集面（`DS01` + `DS05`） |
| `DRY_RUN_DECLARED_DATASETS` / `LOSSY_SKELETON_DATASETS` | dry-run 声明集 / lossy skeleton 集（`DS04`） |
| `NY_FED_DATASETS` / `NY_FED_DECISION` | NY Fed 数据集全集 / 决策结论（`NO-GO`） |
| `FD_VALIDATION_SERIES` / `CANONICAL_WRITE_OWNER` | 永久校验面 / 权威写入方（`fredx`） |

### 守卫

| 函数 | 一行语义 |
| --- | --- |
| `is_curve_product` | 该数据集是否为曲线产品（`DS06` / `DS07`） |
| `is_validation_only_series` | 该 series 是否属 FD01–FD04 永久校验面 |
| `guard_curve_product` | 曲线产品 → `RoutedElsewhere`（路由 `yieldx`） |
| `guard_collection_dataset` | 采集面守卫：`DS01`/`DS05` 放行；`DS06`/`DS07` → `RoutedElsewhere`；`DS04` → `SemanticallyRejected`；其余 → `NotApplicable` |
| `guard_write_authority` | 尝试写权威键（`WALCL`/`WTREGEN`/`RRPONTSYD`/`WRESBAL`）→ `WriteAuthorityDenied` |
| `guard_ny_fed` | 任何 NY Fed 数据集 → `NotApplicable`（整体 `NO-GO`） |
| `unit_for_validation_series` | 校验序列的源侧单位（`RRPONTSYD` 为 Billions，`WALCL`/`WTREGEN` 为 Millions，`WRESBAL` 为 `Unknown`） |

### 授权判定与 publication 语义

| 类型 / 函数 | 一行语义 |
| --- | --- |
| `TreasuryAuthorization` | 判定结果：`Authorized { scope }` / `Denied { reason }` |
| `TreasuryScope` | 请求范围：`OfflineFixtureOnly` / `LiveCollection`（后者恒拒） |
| `TreasuryAuthorizationEvidence` | 证据快照（决策 ID / 签署者 / 覆盖范围） |
| `authorize_treasury` | fail-closed 判定；`Authorized` 的 scope 显式声明仅覆盖 offline fixture |
| `DECISION_ID` / `SUPERSEDED_DECISION_ID` | 当前生效决策 ID / 已失效旧决策 ID |
| `AUTHORIZED_SCOPE` / `LIVE_PREAUTH_CHANNEL` / `LIVE_REQUIRES_NEW_DECISION` | 授权范围 / live 预授权通道 / live 须另批 |
| `TimePrecision` / `AvailabilityEvidence` / `PitEligibility` | publication 三元组的三轴枚举 |
| `publication_semantics` | 恒返回 `(Date, Inferred, NotEligible)` |
| `is_formal_pit_eligible` | 恒返回 `false` |

### 离线解析

| 函数 | 一行语义 |
| --- | --- |
| `parse_treasury_records` | 合成样本 JSON（字符串）→ `TreasuryEnvelope`；未知字段原子失败、重复身份拒绝、信封自洽校验 |

## 安全语义

- 本库**零 HTTP 客户端依赖、零端点字面量、零凭据读取、零代理配置**。
- 解析入口的参数只有字符串：不接受 URL、客户端或任何认证信息。
- 错误消息为简体中文，不回显原始样本正文、凭据或整行配置源码。
- 金额缺失以具名原因表达，**MUST NOT** 静默转 0。
- 曲线产品不得洗白为普通观测；权威 canonical key 不得由本域写入。

## 最小用法

```rust
use treasuryx::{parse_treasury_records, TreasuryError};

# fn demo() -> Result<(), TreasuryError> {
let sample = r#"{"_dataset":"DS05",
    "data":[{"record_date":"2026-08-18","tot_pub_debt_out_amt":36500000000000.0}],
    "meta":{"count":1,"total-count":1,"total-pages":1}}"#;
let envelope = parse_treasury_records(sample)?;
assert_eq!(envelope.records.len(), 1);
# Ok(())
# }
```
