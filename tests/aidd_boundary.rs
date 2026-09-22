#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
//! AIDD 对抗 / 边界用例（特性 005）。
//!
//! 候选由 AI 生成，逐条人工复核后仅保留「结论=保留」项；丢弃项登记于 PR 描述。
//!
//! // AIDD: 曲线产品必须在 parse_id / 守卫 / 解析层三处都拒绝 | 来源=AI | 复核=ZoneCNH/2026-09-22 | 依据=标准.md §4 曲线路由 | 结论=保留
//! // AIDD: DS04 经采集守卫与解析入口都必须被拒 | 来源=AI | 复核=ZoneCNH/2026-09-22 | 依据=标准.md §6 NY Fed 与 DS04 | 结论=保留
//! // AIDD: 权威键写入拒绝须覆盖全部四个校验序列 | 来源=AI | 复核=ZoneCNH/2026-09-22 | 依据=标准.md §5 写入主权与校验面 | 结论=保留
//! // AIDD: meta.count 与 data 行数不一致必须拒绝 | 来源=AI | 复核=ZoneCNH/2026-09-22 | 依据=标准.md §7 解析与合成夹具（信封自洽） | 结论=保留
//! // AIDD: 金额的 JSON 数字与数值字符串等价，非数值字符串拒绝 | 来源=AI | 复核=ZoneCNH/2026-09-22 | 依据=标准.md §7 解析与合成夹具 | 结论=保留
//! // AIDD: 全缺金额须报 Missing 而非静默转 0 | 来源=AI | 复核=ZoneCNH/2026-09-22 | 依据=标准.md §7 解析与合成夹具（缺失具名） | 结论=保留
//! // AIDD: 授权证据纯空白与 live 请求一律 Denied | 来源=AI | 复核=ZoneCNH/2026-09-22 | 依据=标准.md §1 需求标准（fail-closed 授权） | 结论=保留
//! // AIDD: RRPONTSYD 的 Billions 与 WALCL 的 Millions 不得混同 | 来源=AI | 复核=ZoneCNH/2026-09-22 | 依据=标准.md §5 写入主权与校验面（单位风险） | 结论=保留
//! // AIDD: NY Fed 数据集 ID 是封闭集合（3 个，无回退） | 来源=AI | 复核=ZoneCNH/2026-09-22 | 依据=标准.md §6 NY Fed 与 DS04 | 结论=保留

use treasuryx::{
    authorize_treasury, guard_collection_dataset, guard_curve_product, guard_ny_fed,
    guard_write_authority, is_curve_product, parse_treasury_records, unit_for_validation_series,
    validate_record, Date, FredValidationSeries, Frequency, NyFedDatasetId, Period, TreasuryAmount,
    TreasuryAuthorization, TreasuryAuthorizationEvidence, TreasuryDatasetId, TreasuryErrorKind,
    TreasuryMissingReason, TreasuryPayload, TreasuryRecord, TreasuryScope, Unit, DECISION_ID,
    FD_VALIDATION_SERIES, NY_FED_DATASETS,
};

/// 边界：曲线产品在三处入口都必须被拒。
#[test]
fn curve_products_are_rejected_at_every_entry() {
    for curve in [TreasuryDatasetId::Ds06, TreasuryDatasetId::Ds07] {
        assert!(is_curve_product(curve));
        assert_eq!(
            TreasuryDatasetId::parse_id(curve.id())
                .expect_err("parse_id")
                .kind(),
            TreasuryErrorKind::RoutedElsewhere
        );
        assert_eq!(
            guard_curve_product(curve).expect_err("守卫").kind(),
            TreasuryErrorKind::RoutedElsewhere
        );
        assert_eq!(
            guard_collection_dataset(curve)
                .expect_err("采集守卫")
                .kind(),
            TreasuryErrorKind::RoutedElsewhere
        );
    }
    // 曲线符号（`^TNX` 类的 10Y 收益率）不属于本域的任何数据集 ID。
    assert!(TreasuryDatasetId::parse_id("DGS10").is_err());
}

/// 边界：DS04 经采集守卫与解析入口都必须被拒，且不在采集常量内。
#[test]
fn ds04_is_rejected_through_guards_and_parser() {
    assert_eq!(
        guard_collection_dataset(TreasuryDatasetId::Ds04)
            .expect_err("采集守卫")
            .kind(),
        TreasuryErrorKind::SemanticallyRejected
    );
    let sample =
        r#"{"_dataset":"DS04","data":[],"meta":{"count":0,"total-count":0,"total-pages":0}}"#;
    assert_eq!(
        parse_treasury_records(sample).expect_err("解析入口").kind(),
        TreasuryErrorKind::SemanticallyRejected
    );
    // DS04 的载荷即使结构正确也不得通过校验。
    let mut record = base_ds05();
    record.dataset_id = TreasuryDatasetId::Ds04;
    assert!(validate_record(&record).is_err());
}

/// 边界：权威键写入拒绝覆盖全部四个校验序列，且 `series_id` 是精确身份。
#[test]
fn write_authority_rejection_covers_every_validation_series() {
    let ids: Vec<&str> = FD_VALIDATION_SERIES.iter().map(|s| s.series_id()).collect();
    assert_eq!(ids.len(), 4);
    for series_id in ids {
        assert_eq!(
            guard_write_authority(series_id)
                .expect_err("越权写入")
                .kind(),
            TreasuryErrorKind::WriteAuthorityDenied,
            "{series_id}"
        );
    }
    // 非校验序列不受本守卫约束（本库不主张对它们的主权）。
    assert!(guard_write_authority("SOFR").is_ok());
    assert!(guard_write_authority("EFFR").is_ok());
}

/// 边界：`meta.count` 与行数不一致、meta 缺字段或形状非法都必须拒绝。
#[test]
fn envelope_self_consistency_is_enforced() {
    let mismatch = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18",
        "tot_pub_debt_out_amt":1.0}],"meta":{"count":5,"total-count":5,"total-pages":1}}"#;
    assert!(parse_treasury_records(mismatch).is_err());

    let meta_missing = r#"{"_dataset":"DS05","data":[],"meta":{"count":0,"total-count":0}}"#;
    assert_eq!(
        parse_treasury_records(meta_missing)
            .expect_err("meta 缺字段")
            .kind(),
        TreasuryErrorKind::Missing
    );

    let meta_not_object = r#"{"_dataset":"DS05","data":[],"meta":[]}"#;
    assert!(parse_treasury_records(meta_not_object).is_err());

    let meta_negative = r#"{"_dataset":"DS05","data":[],"meta":{"count":-1,"total-count":0,
        "total-pages":0}}"#;
    assert!(parse_treasury_records(meta_negative).is_err());
}

/// 边界：金额的 JSON 数字与数值字符串等价；非数值字符串与布尔值拒绝。
#[test]
fn amount_accepts_numbers_and_numeric_strings_only() {
    let mixed = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18",
        "debt_held_public_amt":"29000000000000.0","intragov_hold_amt":7500000000000.0,
        "tot_pub_debt_out_amt":"36500000000000.0"}],
        "meta":{"count":1,"total-count":1,"total-pages":1}}"#;
    let envelope = parse_treasury_records(mixed).expect("数值字符串可解析");
    assert_eq!(
        envelope.records[0].payload,
        TreasuryPayload::DebtToPenny(treasuryx::TreasuryDebtToPenny {
            debt_held_public_amt: TreasuryAmount::Present(29_000_000_000_000.0),
            intragov_hold_amt: TreasuryAmount::Present(7_500_000_000_000.0),
            tot_pub_debt_out_amt: TreasuryAmount::Present(36_500_000_000_000.0),
        })
    );

    for bad in ["\"abc\"", "true", "[]"] {
        let sample = format!(
            r#"{{"_dataset":"DS05","data":[{{"record_date":"2026-08-18",
            "tot_pub_debt_out_amt":{bad}}}],"meta":{{"count":1,"total-count":1,"total-pages":1}}}}"#
        );
        assert!(parse_treasury_records(&sample).is_err(), "{bad} 必须被拒绝");
    }
}

/// 边界：全缺金额报 `Missing`；缺省与 `null` 是两种具名原因。
#[test]
fn all_missing_amounts_are_named_not_zero() {
    let sample = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18",
        "debt_held_public_amt":null,"intragov_hold_amt":null,"tot_pub_debt_out_amt":null}],
        "meta":{"count":1,"total-count":1,"total-pages":1}}"#;
    assert_eq!(
        parse_treasury_records(sample)
            .expect_err("无可用金额")
            .kind(),
        TreasuryErrorKind::Missing
    );

    let omitted = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18"}],
        "meta":{"count":1,"total-count":1,"total-pages":1}}"#;
    assert_eq!(
        parse_treasury_records(omitted).expect_err("缺字段").kind(),
        TreasuryErrorKind::Missing
    );
    assert_ne!(
        TreasuryAmount::Missing(TreasuryMissingReason::SourceOmitted),
        TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull),
        "两种缺失原因不得混同"
    );
}

/// 边界：授权证据纯空白与 live 请求一律 `Denied`。
#[test]
fn blank_evidence_and_live_are_denied() {
    let evidence = TreasuryAuthorizationEvidence::new(DECISION_ID, "ZoneCNH", "treasury_offline");
    for blank in ["", " ", "　"] {
        let blank_scope = TreasuryAuthorizationEvidence::new(DECISION_ID, "ZoneCNH", blank);
        assert!(matches!(
            authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&blank_scope)),
            TreasuryAuthorization::Denied { .. }
        ));
    }
    match authorize_treasury(TreasuryScope::LiveCollection, Some(&evidence)) {
        TreasuryAuthorization::Denied { reason } => assert!(reason.contains("另批")),
        other => panic!("live 必须拒绝：{other:?}"),
    }
    // NY Fed 在任何数据集上都保持 NO-GO。
    for dataset_id in NY_FED_DATASETS {
        assert!(guard_ny_fed(dataset_id).is_err());
    }
}

/// 边界：Billions 与 Millions 不得混同；未登记单位的序列记 `Unknown`。
#[test]
fn unit_risk_is_never_silently_converted() {
    assert_eq!(
        unit_for_validation_series(FredValidationSeries::Fd03),
        Unit::BillionsOfUsd
    );
    assert_eq!(
        unit_for_validation_series(FredValidationSeries::Fd01),
        Unit::MillionsOfUsd
    );
    assert_ne!(
        unit_for_validation_series(FredValidationSeries::Fd03),
        unit_for_validation_series(FredValidationSeries::Fd01)
    );
    assert_eq!(
        unit_for_validation_series(FredValidationSeries::Fd04),
        Unit::Unknown,
        "WRESBAL 未登记单位，不得推断"
    );
    // 记录层单位必须是 Unknown（清单未声明 DS01 / DS05 的源侧单位）。
    let mut wrong_unit = base_ds05();
    wrong_unit.unit = Unit::MillionsOfUsd;
    assert!(validate_record(&wrong_unit).is_err());
}

/// 构造一条合法的 DS05 记录。
fn base_ds05() -> TreasuryRecord {
    TreasuryRecord {
        dataset_id: TreasuryDatasetId::Ds05,
        period: Period::Day(Date {
            year: 2026,
            month: 8,
            day: 18,
        }),
        frequency: Frequency::Daily,
        unit: Unit::Unknown,
        revision: None,
        payload: TreasuryPayload::DebtToPenny(treasuryx::TreasuryDebtToPenny {
            debt_held_public_amt: TreasuryAmount::Present(1.0),
            intragov_hold_amt: TreasuryAmount::Present(1.0),
            tot_pub_debt_out_amt: TreasuryAmount::Present(2.0),
        }),
    }
}

/// 边界：NY Fed 数据集 ID 只能解析为三个固定值（供上面的用例复用）。
#[test]
fn ny_fed_ids_are_a_closed_set() {
    for (text, expected) in [
        ("FR01", NyFedDatasetId::Fr01),
        ("FR02", NyFedDatasetId::Fr02),
        ("FR03", NyFedDatasetId::Fr03),
    ] {
        assert_eq!(NyFedDatasetId::parse_id(text).expect("合法 ID"), expected);
    }
    assert!(NyFedDatasetId::parse_id("FR04").is_err());
}
