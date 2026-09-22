# treasuryx 上下文

本文件定义 `treasuryx` 与其使用方共享的核心词汇，记录领域含义与能力边界，
不记录具体实现细节或部署决定。

## 角色与边界

**源事实类型层**：把美国财政部 Fiscal Data 等源侧的数据集标识、字段形态与跨源关系
收敛成稳定 Rust 类型的一层。它只表达「源侧事实是什么」，不采集、不换算、不派生。
_Avoid_: 采集器 / 客户端 SDK（本库无任何网络能力）

**采集面**（`WIRE_COLLECT_DATASETS`）：本域登记的**最小**采集集，恰为 `DS01` + `DS05`。
_Avoid_: 数据集全集（`PHASE1_DATASETS` 是目录，不是采集义务）

**dry-run 声明集**（`DRY_RUN_DECLARED_DATASETS`）：已接 dry-run 的 `DS01` / `DS04` / `DS05`。
**dry-run ≠ live**。
_Avoid_: 已接线（dry-run 只证明离线映射可用）

**lossy skeleton map**（`DS04`）：拍卖结果的骨架映射**有损**，**禁作拍卖事实源**，
且**不进采集常量**。
_Avoid_: 拍卖数据源（它不足以支撑拍卖事实）

## 语义身份

**数据集 ID ≠ 采集义务**：`DS01`–`DS10` 是清单目录；能否进入采集面由
`guard_collection_dataset` 判定。
_Avoid_: 数据集（不带守卫的「数据集」会掩盖采集面边界）

**曲线产品**（`DS06` / `DS07`）：日度收益率曲线与实际收益率曲线是**曲线产品**，
MUST 路由 `yieldx`，MUST NOT 自动映射为普通观测。
_Avoid_: 收益率数据（曲线点的归宿是 `yieldx`，不是本域的普通观测）

**永久校验面**（`FD01`–`FD04`）：`WALCL` / `WTREGEN` / `RRPONTSYD` / `WRESBAL`
四个序列在本域**只作交叉校验**；权威 Observation 的唯一写入方是 `fredx`。
_Avoid_: 副本来（本域不持有权威值，也不双写 canonical key）

**单位风险**：`RRPONTSYD` 的源单位是 **Billions of U.S. Dollars**，`WALCL` / `WTREGEN` 为 **Millions**；
`WRESBAL` 未在单位风险条中登记，故记 `Unknown`。
_Avoid_: 单位换算（换算归下游 Normalize，源层 MUST NOT 静默换算）

**具名缺失**：缺省与显式 `null` 是两种不同原因；缺失 MUST 具名表达，MUST NOT 静默转 0。
_Avoid_: 空值（不区分原因则无法机械校验）

**重复身份**：`DS01` = 数据集 + 日期 + 账户类型；`DS05` = 数据集 + 日期。
解析器**拒绝**重复，而非静默去重。
_Avoid_: 去重（静默去重会掩盖源侧重复，破坏可审计性）

**合成样本层面的自洽不变量**：本库对合成样本要求 `meta.count` 等于 `data` 行数，不满足即拒绝。
它**不是**对真实 Fiscal Data 信封语义的结论（清单 §7 把分页语义登记为部分收敛 / 仍 UNKNOWN）。
_Avoid_: 分页契约（本层未核验真实分页语义，不得据此表述为源侧保证）

## 授权与证据

**offline-first 授权**：`TREASURY-B1-2026-08-20-OFFLINE`（v2，覆盖旧签）**只覆盖
offline fixture 范围**；授权通道（Fiscal Data 官方 API）**仅为未来 live 阶段预授权**。
_Avoid_: live 授权（live 须另批决策 ID + B2–B8 门禁）

**fail-closed 授权判定**：证据缺失 / 决策 ID 空白 / 签署者空白 / 覆盖范围空白 / live 请求
→ **一律** `Denied`。MUST NOT 默认放行。
_Avoid_: 生产就绪判定（本判定回答「该范围是否被授权访问」）

**publication 语义**：`TimePrecision::Date` + `AvailabilityEvidence::Inferred` +
`PitEligibility::NotEligible`；Fiscal Data 无官方发布时刻字段，亦无签批日历。
_Avoid_: 发布日期（本层表达的是**身份**与**证据层**）

**禁止静默升格**：接入官方字段或签批日历前，证据层保持 `Inferred`。
_Avoid_: 缺省时刻（补造 `00:00 UTC` 把 `Date` 伪装成 `Instant` 是违规形态）

**合成样本**：`tests/fixtures/` 内的样本全部为自拟合成样本，不是真实源数据，
不构成任何证据，不得表述为「实测」「核验 PASS」。
_Avoid_: 夹具数据（容易被误读为真实样本）

## 已知缺口

- 本库不含采集或 live 能力：`treasury_runtime` 未落地，`DS01` / `DS05` 的持续 live 为 `NO-GO`。
- `_dataset` 是**合成夹具的标注字段**，真实源信封不具备；本库不声称可解析真实源响应。
- 清单未声明 `DS01` / `DS05` 的源侧单位，故记录层单位恒为 `Unit::Unknown`。
- 清单未声明官方 revision 面，故 `TreasuryRecord::revision` 恒为 `None` 且校验拒绝非 `None`。

## rust-version 推导

规则：`rust-version` = 依赖图中所有依赖所声明 `rust_version` 的最大值
（`cargo metadata --format-version 1` 的 `rust_version` 字段）。

| 依赖 | 版本 | 其 `rust_version` |
| --- | --- | --- |
| `thiserror` | 2.0.20 | 1.71 |
| `thiserror-impl` | 2.0.20 | 1.71 |
| `serde` | 1.0.229 | 1.56 |
| `serde_core` | 1.0.229 | 1.56 |
| `serde_derive` | 1.0.229 | 1.71 |
| `serde_json` | 1.0.151 | 1.71 |
| `proc-macro2` | 1.0.107 | 1.71 |
| `quote` | 1.0.47 | 1.71 |
| `syn` | 3.0.6 | 1.71 |
| `unicode-ident` | 1.0.26 | 1.71 |
| `itoa` | 1.0.18 | 1.68 |
| `memchr` | 2.8.3 | 1.61 |
| `zmij` | 1.0.23 | 1.71 |

**最大值 = `1.71`**，故本 crate 声明 `rust-version = "1.71"`。
无依赖缺失 `rust_version` 的情形，无需保守取值。
