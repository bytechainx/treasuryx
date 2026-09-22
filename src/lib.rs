#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable
    )
)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(unreachable_pub)]

//! treasuryx —— 美国财政部源事实的离线类型层：数据集常量、路由与主权守卫、fail-closed 授权判定
//!
//! ## 能力
//!
//! | 能力 | 状态 |
//! | --- | --- |
//! | Fiscal Data Phase 1 数据集常量（[`PHASE1_DATASETS`]，`DS01`–`DS10`） | 已实现 |
//! | 采集面常量（[`WIRE_COLLECT_DATASETS`] = `DS01` + `DS05`） | 已实现 |
//! | 曲线路由守卫（[`guard_curve_product`]：`DS06` / `DS07` → `yieldx`） | 已实现 |
//! | 写入主权守卫（[`guard_write_authority`]：`WALCL` / `WTREGEN` / `RRPONTSYD` / `WRESBAL`） | 已实现 |
//! | NY Fed `NO-GO` 声明与守卫（[`guard_ny_fed`]） | 已实现 |
//! | `DS04` lossy skeleton 拒绝（[`guard_collection_dataset`]） | 已实现 |
//! | 记录值对象与信封解析（[`parse_treasury_records`]） | 已实现 |
//! | fail-closed 授权判定（[`authorize_treasury`]） | 已实现 |
//! | 联网采集 / live / PIT 升级 / 派生指标 | **不实现** |
//!
//! ## 责任边界
//!
//! 本库只表达「美国财政部源侧事实是什么」，并提供离线解析与守卫。
//! 它**不拥有**任何网络能力：无 HTTP 客户端依赖、无端点字面量、不读凭据、不配代理。
//!
//! ## 非目标
//!
//! - 不做联网采集（`treasury_runtime` 与 live 采集未落地；本签不覆盖 live）
//! - 不做派生指标（净流动性、TGA 脉冲、财政脉冲等归 analytics）
//! - 不做单位换算（保留源侧单位；`RRPONTSYD` 的 Billions 与 `WALCL` 的 Millions 不得混同）
//! - 不写权威 canonical key（权威写入归 `fredx`；本域只对账告警）
//! - 不做存储、分发与调度
//!
//! ## 诚实边界
//!
//! `production_decision = NO-GO`；清单 COMPLETE ≠ ship；authorization ≠ Production Ready。
//! `TREASURY-B1-2026-08-20-OFFLINE` 只覆盖 **offline fixture** 范围，
//! 授权通道仅为未来 live 阶段预授权（live 须另批）。
//! `tests/fixtures/` 内的样本全部为**合成样本**，不是真实源数据，不构成任何证据。
//!
//! # 最小示例
//!
//! ```
//! use treasuryx::{guard_write_authority, parse_treasury_records, TreasuryError};
//!
//! # let sample = r#"{"_dataset":"DS05",
//! #     "data":[{"record_date":"2026-08-18","tot_pub_debt_out_amt":36500000000000.0}],
//! #     "meta":{"count":1,"total-count":1,"total-pages":1}}"#;
//! let envelope = parse_treasury_records(sample)?;
//! assert_eq!(envelope.records.len(), 1);
//!
//! // 写入主权：`WALCL` 的权威写入归 fredx，本域不得写权威键
//! assert!(matches!(
//!     guard_write_authority("WALCL"),
//!     Err(TreasuryError::WriteAuthorityDenied(_))
//! ));
//! # Ok::<(), TreasuryError>(())
//! ```

pub mod authz;
pub mod error;
pub mod parse;
pub mod pit;
pub mod value;

pub use authz::{
    authorize_treasury, TreasuryAuthorization, TreasuryAuthorizationEvidence, TreasuryScope,
    AUTHORIZED_SCOPE, DECISION_ID, LIVE_PREAUTH_CHANNEL, LIVE_REQUIRES_NEW_DECISION,
    SUPERSEDED_DECISION_ID,
};
pub use error::{TreasuryError, TreasuryErrorKind, TreasuryResult};
pub use parse::parse_treasury_records;
pub use pit::{
    is_formal_pit_eligible, publication_semantics, AvailabilityEvidence, PitEligibility,
    TimePrecision,
};
pub use value::{
    guard_collection_dataset, guard_curve_product, guard_ny_fed, guard_write_authority,
    is_curve_product, is_validation_only_series, unit_for_validation_series, validate_record, Date,
    FredValidationSeries, Frequency, NyFedDatasetId, Period, TreasuryAmount, TreasuryCashBalance,
    TreasuryDatasetId, TreasuryDebtToPenny, TreasuryEnvelope, TreasuryEnvelopeMeta,
    TreasuryMissingReason, TreasuryPayload, TreasuryRecord, Unit, CANONICAL_WRITE_OWNER,
    DRY_RUN_DECLARED_DATASETS, FD_VALIDATION_SERIES, LOSSY_SKELETON_DATASETS, NY_FED_DATASETS,
    NY_FED_DECISION, PHASE1_DATASETS, WIRE_COLLECT_DATASETS,
};
