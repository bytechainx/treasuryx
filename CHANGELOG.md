# Changelog — treasuryx

本文件记录 `treasuryx` 的用户可见变更，遵循 [Keep a Changelog](https://keepachangelog.com/)
与 [Semantic Versioning](https://semver.org/)。

## [Unreleased]

### 新增

- 新增 E2E target `tests/e2e_treasury.rs`：把本仓**核对器口径内的全部公开条目**逐条真实执行
  （权威公开面派生自 `cargo +nightly public-api --simplified`，共 **156 条** = `type` 24 /
  `variant` 65 / `field` 23 / `const` 13 / `fn` 31）。**纯测试新增，不改公开 API、不升版本**；
  核对口径见 `AGENTS.md`「E2E 全公开面覆盖核对」与元仓库 `scripts/AGENTS.md` §2.1.1。

## [0.1.0] - 2026-09-22

### 新增

- 首次落地：美国财政部源事实的离线类型层，采集范围对齐 `specs/adapter/treasury.md`。
- `TreasuryDatasetId`（`DS01`–`DS10`）、`NyFedDatasetId`（`FR01`–`FR03`）、
  `FredValidationSeries`（`FD01`–`FD04`）三个 ID 枚举与访问器（`id` / `frequency` /
  `series_id` / `label` / `canonical_write_owner` / `parse_id`）。
- 常量集：`PHASE1_DATASETS`（10）、`WIRE_COLLECT_DATASETS`（采集面 = `DS01` + `DS05`）、
  `DRY_RUN_DECLARED_DATASETS`、`LOSSY_SKELETON_DATASETS`（`DS04`）、`NY_FED_DATASETS`、
  `FD_VALIDATION_SERIES`、`CANONICAL_WRITE_OWNER`（`fredx`）、`NY_FED_DECISION`（`NO-GO`）。
- 曲线路由守卫：`is_curve_product` / `guard_curve_product`（`DS06` / `DS07` → `yieldx`），
  `TreasuryDatasetId::parse_id` 对曲线产品 fail-closed。
- 采集面守卫 `guard_collection_dataset`：`DS01`/`DS05` 放行、曲线 → `RoutedElsewhere`、
  `DS04` → `SemanticallyRejected`、其余 → `NotApplicable`。
- 写入主权守卫 `guard_write_authority`：`WALCL` / `WTREGEN` / `RRPONTSYD` / `WRESBAL`
  的权威键写入一律 `WriteAuthorityDenied`；`is_validation_only_series` 与
  `unit_for_validation_series`（Billions vs Millions 具名表达）。
- NY Fed `NO-GO` 守卫 `guard_ny_fed`。
- 记录值对象：`TreasuryRecord` / `TreasuryPayload` / `TreasuryCashBalance` /
  `TreasuryDebtToPenny` / `TreasuryAmount` / `TreasuryEnvelope` / `validate_record`。
- `TreasuryAuthorization` / `TreasuryScope` / `authorize_treasury`：fail-closed 授权判定，
  `Authorized` 的 scope 显式声明**仅覆盖 offline fixture**（live 须另批）。
- `TimePrecision` / `AvailabilityEvidence` / `PitEligibility` 与 `publication_semantics`：
  恒为 `(Date, Inferred, NotEligible)`；`is_formal_pit_eligible()` 恒为 `false`。
- `parse_treasury_records`：离线合成信封解析（字段白名单、未知字段原子失败、
  重复身份拒绝、`meta.count` 与行数自洽）。
- 三类测试：`tests/tdd_contracts.rs`（// TDD-PROBE 表）、`tests/sdd_spec.rs`（// SPEC-MAP 与
  `docs/标准.md` 章节 1:1）、`tests/aidd_boundary.rs`（// AIDD 复核表）；夹具为合成样本。

### 说明

本 crate **不实现任何联网采集或 live 路径**：零 HTTP 客户端依赖、零端点字面量、
零凭据读取、零代理配置，也不依赖任何 `bytechainx/*` crate。仓级 `production_decision = NO-GO`。
`tests/fixtures/` 内的全部样本为**合成样本**，不是真实源数据，不构成任何证据。
