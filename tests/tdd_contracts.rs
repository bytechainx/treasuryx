#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
//! TDD 行为契约（特性 005）。
//!
//! 入口集合 = 本 crate 全部公开入口（含 `validate*` / 判定函数 / 解析器）。
//! 下表每个入口先在 `/tmp` 变异副本上观测应红、再在本树观测绿。
//!
//! 全部用例离线运行：样本为 `tests/fixtures/` 内的合成样本（经 `include_str!` 内联），
//! 不依赖任何外部服务，不读环境变量。
//!
//! // TDD-PROBE: TreasuryError::kind | 变异：某个变体的分类映射改指相邻变体 | 红=error_kind_maps_each_variant | 绿=error_kind_maps_each_variant
//! // TDD-PROBE: TreasuryError::is_retryable | 变异：改为除 Invariant 外全部返回 true | 红=error_is_retryable_only_for_invariant | 绿=error_is_retryable_only_for_invariant
//! // TDD-PROBE: Date::new | 变异：去掉闰年判定，二月恒为 29 天 | 红=date_new_validates_range | 绿=date_new_validates_range
//! // TDD-PROBE: Date::parse | 变异：放宽长度校验，接受 2026-8-18 | 红=date_parse_is_strict_iso | 绿=date_parse_is_strict_iso
//! // TDD-PROBE: Period::parse_day | 变异：返回 Period::Year 而非 Period::Day | 红=period_parse_day_is_single_day | 绿=period_parse_day_is_single_day
//! // TDD-PROBE: TreasuryDatasetId::id | 变异：Ds10 的登记形改为 DS01 | 红=catalog_accessors_match_the_manifest | 绿=catalog_accessors_match_the_manifest
//! // TDD-PROBE: TreasuryDatasetId::frequency | 变异：Ds04 的频率改为 Daily | 红=catalog_accessors_match_the_manifest | 绿=catalog_accessors_match_the_manifest
//! // TDD-PROBE: TreasuryDatasetId::parse_id | 变异：DS06 / DS07 改为返回 Ok | 红=dataset_parse_id_routes_curves | 绿=dataset_parse_id_routes_curves
//! // TDD-PROBE: NyFedDatasetId::parse_id | 变异：未知 ID 回退为 FR01 | 红=ny_fed_parse_id_is_fail_closed | 绿=ny_fed_parse_id_is_fail_closed
//! // TDD-PROBE: NyFedDatasetId::id | 变异：Fr03 的登记形改为 FR01 | 红=catalog_accessors_match_the_manifest | 绿=catalog_accessors_match_the_manifest
//! // TDD-PROBE: NyFedDatasetId::frequency | 变异：Fr02 的频率改为 Daily | 红=catalog_accessors_match_the_manifest | 绿=catalog_accessors_match_the_manifest
//! // TDD-PROBE: FredValidationSeries::parse_id | 变异：未知 ID 回退为 FD01 | 红=validation_series_accessors_match_the_manifest | 绿=validation_series_accessors_match_the_manifest
//! // TDD-PROBE: FredValidationSeries::series_id | 变异：FD03 的 series 改为 WALCL | 红=validation_series_accessors_match_the_manifest | 绿=validation_series_accessors_match_the_manifest
//! // TDD-PROBE: FredValidationSeries::label | 变异：label 恒返回空串 | 红=validation_series_accessors_match_the_manifest | 绿=validation_series_accessors_match_the_manifest
//! // TDD-PROBE: FredValidationSeries::canonical_write_owner | 变异：返回 treasuryx 而非 fredx | 红=validation_series_accessors_match_the_manifest | 绿=validation_series_accessors_match_the_manifest
//! // TDD-PROBE: is_curve_product | 变异：只认 DS06 不认 DS07 | 红=is_curve_product_covers_both | 绿=is_curve_product_covers_both
//! // TDD-PROBE: is_validation_only_series | 变异：从校验面去掉 WRESBAL | 红=is_validation_only_series_covers_four | 绿=is_validation_only_series_covers_four
//! // TDD-PROBE: guard_curve_product | 变异：曲线产品改为返回 Ok | 红=guard_curve_product_routes_to_yieldx | 绿=guard_curve_product_routes_to_yieldx
//! // TDD-PROBE: guard_collection_dataset | 变异：DS04 改为放行 | 红=guard_collection_dataset_covers_all_ten | 绿=guard_collection_dataset_covers_all_ten
//! // TDD-PROBE: guard_write_authority | 变异：只拒绝 WALCL 一个序列 | 红=guard_write_authority_rejects_validation_series | 绿=guard_write_authority_rejects_validation_series
//! // TDD-PROBE: guard_ny_fed | 变异：FR01 改为放行 | 红=guard_ny_fed_is_no_go | 绿=guard_ny_fed_is_no_go
//! // TDD-PROBE: unit_for_validation_series | 变异：FD03 的单位改为 Millions | 红=unit_risk_is_expressed | 绿=unit_risk_is_expressed
//! // TDD-PROBE: validate_record | 变异：金额全缺时静默补 0 而非报 Missing | 红=validate_record_rejects_incomplete_or_illegal | 绿=validate_record_rejects_incomplete_or_illegal
//! // TDD-PROBE: TreasuryAmount::present | 变异：缺失槽返回 Some(0.0) | 红=amount_present_never_zero_fills | 绿=amount_present_never_zero_fills
//! // TDD-PROBE: TreasuryAmount::is_missing | 变异：Present 也判为缺失 | 红=amount_is_missing_matches_variant | 绿=amount_is_missing_matches_variant
//! // TDD-PROBE: authorize_treasury | 变异：证据缺失时默认放行 | 红=authorize_treasury_is_fail_closed | 绿=authorize_treasury_is_fail_closed
//! // TDD-PROBE: TreasuryAuthorizationEvidence::new | 变异：构造时丢弃 scope 字段 | 红=authorization_evidence_new_copies_fields | 绿=authorization_evidence_new_copies_fields
//! // TDD-PROBE: publication_semantics | 变异：可得性证据层改为 Official | 红=publication_semantics_is_date_inferred_not_eligible | 绿=publication_semantics_is_date_inferred_not_eligible
//! // TDD-PROBE: is_formal_pit_eligible | 变异：改为恒返回 true | 红=is_formal_pit_eligible_is_false | 绿=is_formal_pit_eligible_is_false
//! // TDD-PROBE: parse_treasury_records | 变异：未知字段改为静默忽略 | 红=parse_treasury_records_rejects_unknown_field | 绿=parse_treasury_records_rejects_unknown_field

use treasuryx::{
    authorize_treasury, guard_collection_dataset, guard_curve_product, guard_ny_fed,
    guard_write_authority, is_curve_product, is_formal_pit_eligible, is_validation_only_series,
    parse_treasury_records, publication_semantics, unit_for_validation_series, validate_record,
    AvailabilityEvidence, Date, FredValidationSeries, Frequency, NyFedDatasetId, Period,
    PitEligibility, TimePrecision, TreasuryAmount, TreasuryAuthorization,
    TreasuryAuthorizationEvidence, TreasuryDatasetId, TreasuryError, TreasuryErrorKind,
    TreasuryMissingReason, TreasuryPayload, TreasuryRecord, TreasuryScope, Unit, DECISION_ID,
    FD_VALIDATION_SERIES, NY_FED_DATASETS, PHASE1_DATASETS, WIRE_COLLECT_DATASETS,
};

/// 合成样本（`_synthetic` 标注见文件本体）。
const DS01: &str = include_str!("fixtures/ds01_envelope.json");
const DS05: &str = include_str!("fixtures/ds05_envelope.json");
const CURVE: &str = include_str!("fixtures/curve_envelope.json");

/// `TreasuryError::kind`：8 个变体各归其类。
#[test]
fn error_kind_maps_each_variant() {
    let cases = [
        (
            TreasuryError::Invalid("x".into()),
            TreasuryErrorKind::Invalid,
        ),
        (
            TreasuryError::Missing("x".into()),
            TreasuryErrorKind::Missing,
        ),
        (
            TreasuryError::AuthorizationDenied("x".into()),
            TreasuryErrorKind::AuthorizationDenied,
        ),
        (
            TreasuryError::RoutedElsewhere("x".into()),
            TreasuryErrorKind::RoutedElsewhere,
        ),
        (
            TreasuryError::WriteAuthorityDenied("x".into()),
            TreasuryErrorKind::WriteAuthorityDenied,
        ),
        (
            TreasuryError::SemanticallyRejected("x".into()),
            TreasuryErrorKind::SemanticallyRejected,
        ),
        (
            TreasuryError::NotApplicable("x".into()),
            TreasuryErrorKind::NotApplicable,
        ),
        (
            TreasuryError::Invariant("x".into()),
            TreasuryErrorKind::Invariant,
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.kind(), expected, "分类不符：{error:?}");
    }
}

/// `TreasuryError::is_retryable`：本层无网络，仅 `Invariant` 可重试。
#[test]
fn error_is_retryable_only_for_invariant() {
    assert!(TreasuryError::Invariant("x".into()).is_retryable());
    assert!(!TreasuryError::WriteAuthorityDenied("x".into()).is_retryable());
    assert!(!TreasuryError::RoutedElsewhere("x".into()).is_retryable());
}

/// `Date::new`：月份、日与闰年三重校验。
#[test]
fn date_new_validates_range() {
    assert!(Date::new(2026, 8, 18).is_ok());
    assert!(Date::new(2024, 2, 29).is_ok(), "2024 是闰年");
    assert!(Date::new(2026, 2, 29).is_err(), "2026 不是闰年");
    assert!(Date::new(2026, 13, 1).is_err());
}

/// `Date::parse`：只接受严格的 `YYYY-MM-DD`。
#[test]
fn date_parse_is_strict_iso() {
    assert_eq!(
        Date::parse("2026-08-18").expect("合法日期"),
        Date {
            year: 2026,
            month: 8,
            day: 18
        }
    );
    for bad in ["2026-8-18", "2026/08/18", "2026-08-18T00:00:00"] {
        assert!(Date::parse(bad).is_err(), "{bad} 必须被拒绝");
    }
}

/// `Period::parse_day`：得到单日期间。
#[test]
fn period_parse_day_is_single_day() {
    assert!(matches!(
        Period::parse_day("2026-08-18").expect("合法日"),
        Period::Day(_)
    ));
}

/// 数据集 / NY Fed / 校验序列的登记形与频率与清单一致。
#[test]
fn catalog_accessors_match_the_manifest() {
    let ids: Vec<&str> = PHASE1_DATASETS.iter().map(|id| id.id()).collect();
    assert_eq!(
        ids,
        ["DS01", "DS02", "DS03", "DS04", "DS05", "DS06", "DS07", "DS08", "DS09", "DS10"]
    );
    assert_eq!(TreasuryDatasetId::Ds04.frequency(), Frequency::Event);
    assert_eq!(TreasuryDatasetId::Ds08.frequency(), Frequency::Monthly);
    assert_eq!(TreasuryDatasetId::Ds05.frequency(), Frequency::Daily);

    let ny_fed: Vec<&str> = NY_FED_DATASETS.iter().map(|id| id.id()).collect();
    assert_eq!(ny_fed, ["FR01", "FR02", "FR03"]);
    assert_eq!(NyFedDatasetId::Fr02.frequency(), Frequency::Weekly);
}

/// `TreasuryDatasetId::parse_id`：曲线产品路由，未知 ID 拒绝。
#[test]
fn dataset_parse_id_routes_curves() {
    assert_eq!(
        TreasuryDatasetId::parse_id("DS05").expect("合法 ID"),
        TreasuryDatasetId::Ds05
    );
    for curve in ["DS06", "DS07"] {
        assert_eq!(
            TreasuryDatasetId::parse_id(curve)
                .expect_err("曲线必须路由")
                .kind(),
            TreasuryErrorKind::RoutedElsewhere,
            "{curve}"
        );
    }
    assert_eq!(
        TreasuryDatasetId::parse_id("DS99")
            .expect_err("未知")
            .kind(),
        TreasuryErrorKind::Invalid
    );
}

/// `NyFedDatasetId::parse_id`：未知 ID 不得回退。
#[test]
fn ny_fed_parse_id_is_fail_closed() {
    assert_eq!(
        NyFedDatasetId::parse_id("FR01").expect("合法 ID"),
        NyFedDatasetId::Fr01
    );
    assert!(NyFedDatasetId::parse_id("FR99").is_err());
}

/// 校验序列的四个访问器与清单一致。
#[test]
fn validation_series_accessors_match_the_manifest() {
    let ids: Vec<&str> = FD_VALIDATION_SERIES.iter().map(|s| s.id()).collect();
    assert_eq!(ids, ["FD01", "FD02", "FD03", "FD04"]);
    let series: Vec<&str> = FD_VALIDATION_SERIES.iter().map(|s| s.series_id()).collect();
    assert_eq!(series, ["WALCL", "WTREGEN", "RRPONTSYD", "WRESBAL"]);
    for entry in FD_VALIDATION_SERIES {
        assert!(!entry.label().is_empty(), "{:?} 必须有中文含义", entry.id());
        assert_eq!(entry.canonical_write_owner(), "fredx");
    }
    assert!(FredValidationSeries::parse_id("FD05").is_err());
}

/// `is_curve_product`：`DS06` 与 `DS07` 都是曲线产品。
#[test]
fn is_curve_product_covers_both() {
    assert!(is_curve_product(TreasuryDatasetId::Ds06));
    assert!(is_curve_product(TreasuryDatasetId::Ds07));
    assert!(!is_curve_product(TreasuryDatasetId::Ds05));
}

/// `is_validation_only_series`：永久校验面恰为 4 个序列。
#[test]
fn is_validation_only_series_covers_four() {
    for series_id in ["WALCL", "WTREGEN", "RRPONTSYD", "WRESBAL"] {
        assert!(is_validation_only_series(series_id), "{series_id}");
    }
    assert!(!is_validation_only_series("DGS10"));
}

/// `guard_curve_product`：曲线产品一律路由 `yieldx`。
#[test]
fn guard_curve_product_routes_to_yieldx() {
    assert_eq!(
        guard_curve_product(TreasuryDatasetId::Ds06)
            .expect_err("曲线必须路由")
            .kind(),
        TreasuryErrorKind::RoutedElsewhere
    );
    assert!(guard_curve_product(TreasuryDatasetId::Ds01).is_ok());
}

/// `guard_collection_dataset`：采集面恰为 `DS01` + `DS05`。
#[test]
fn guard_collection_dataset_covers_all_ten() {
    assert_eq!(
        WIRE_COLLECT_DATASETS,
        [TreasuryDatasetId::Ds01, TreasuryDatasetId::Ds05]
    );
    for dataset_id in PHASE1_DATASETS {
        let result = guard_collection_dataset(dataset_id);
        match dataset_id {
            TreasuryDatasetId::Ds01 | TreasuryDatasetId::Ds05 => assert!(result.is_ok()),
            TreasuryDatasetId::Ds06 | TreasuryDatasetId::Ds07 => assert_eq!(
                result.expect_err("曲线").kind(),
                TreasuryErrorKind::RoutedElsewhere
            ),
            TreasuryDatasetId::Ds04 => assert_eq!(
                result.expect_err("lossy skeleton").kind(),
                TreasuryErrorKind::SemanticallyRejected
            ),
            _ => assert_eq!(
                result.expect_err("采集面外").kind(),
                TreasuryErrorKind::NotApplicable
            ),
        }
    }
}

/// `guard_write_authority`：四个校验序列的权威键写入一律拒绝。
#[test]
fn guard_write_authority_rejects_validation_series() {
    for series_id in ["WALCL", "WTREGEN", "RRPONTSYD", "WRESBAL"] {
        assert_eq!(
            guard_write_authority(series_id)
                .expect_err("越权写入")
                .kind(),
            TreasuryErrorKind::WriteAuthorityDenied,
            "{series_id}"
        );
    }
    assert!(guard_write_authority("DGS10").is_ok());
}

/// `guard_ny_fed`：NY Fed 整体 `NO-GO`。
#[test]
fn guard_ny_fed_is_no_go() {
    for dataset_id in NY_FED_DATASETS {
        assert_eq!(
            guard_ny_fed(dataset_id).expect_err("NO-GO").kind(),
            TreasuryErrorKind::NotApplicable
        );
    }
}

/// `unit_for_validation_series`：Billions 与 Millions 不得混同。
#[test]
fn unit_risk_is_expressed() {
    assert_eq!(
        unit_for_validation_series(FredValidationSeries::Fd03),
        Unit::BillionsOfUsd
    );
    assert_eq!(
        unit_for_validation_series(FredValidationSeries::Fd02),
        Unit::MillionsOfUsd
    );
    assert_ne!(
        unit_for_validation_series(FredValidationSeries::Fd03),
        unit_for_validation_series(FredValidationSeries::Fd02)
    );
}

/// `validate_record`：金额全缺为 `Missing`，载荷错配为 `Invalid`。
#[test]
fn validate_record_rejects_incomplete_or_illegal() {
    let mut all_missing = sample_ds05();
    all_missing.payload = TreasuryPayload::DebtToPenny(treasuryx::TreasuryDebtToPenny {
        debt_held_public_amt: TreasuryAmount::Missing(TreasuryMissingReason::SourceOmitted),
        intragov_hold_amt: TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull),
        tot_pub_debt_out_amt: TreasuryAmount::Missing(TreasuryMissingReason::SourceOmitted),
    });
    assert_eq!(
        validate_record(&all_missing)
            .expect_err("无可用金额")
            .kind(),
        TreasuryErrorKind::Missing
    );

    let mut mismatch = sample_ds05();
    mismatch.payload = TreasuryPayload::CashBalance(treasuryx::TreasuryCashBalance {
        account_type: "x".into(),
        open_today_bal: TreasuryAmount::Present(1.0),
        close_today_bal: TreasuryAmount::Present(1.0),
    });
    assert_eq!(
        validate_record(&mismatch).expect_err("载荷错配").kind(),
        TreasuryErrorKind::Invalid
    );

    assert!(validate_record(&sample_ds05()).is_ok());
}

/// `TreasuryAmount::present`：缺失不得静默转 0。
#[test]
fn amount_present_never_zero_fills() {
    assert_eq!(TreasuryAmount::Present(1.5).present(), Some(1.5));
    assert_eq!(
        TreasuryAmount::Missing(TreasuryMissingReason::SourceOmitted).present(),
        None
    );
}

/// `TreasuryAmount::is_missing`：只对缺失变体为真。
#[test]
fn amount_is_missing_matches_variant() {
    assert!(TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull).is_missing());
    assert!(!TreasuryAmount::Present(0.0).is_missing());
}

/// `authorize_treasury`：证据缺失 / 空白 / live 请求一律拒绝。
#[test]
fn authorize_treasury_is_fail_closed() {
    let evidence = TreasuryAuthorizationEvidence::new(DECISION_ID, "ZoneCNH", "treasury_offline");
    match authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&evidence)) {
        TreasuryAuthorization::Authorized { scope } => {
            assert!(scope.contains("offline_fixture_only"));
            assert!(scope.contains("live 须另批"));
        }
        other => panic!("离线面应授权：{other:?}"),
    }
    assert!(matches!(
        authorize_treasury(TreasuryScope::OfflineFixtureOnly, None),
        TreasuryAuthorization::Denied { .. }
    ));
    let blank = TreasuryAuthorizationEvidence::new("", "ZoneCNH", "s");
    assert!(matches!(
        authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&blank)),
        TreasuryAuthorization::Denied { .. }
    ));
    assert!(matches!(
        authorize_treasury(TreasuryScope::LiveCollection, Some(&evidence)),
        TreasuryAuthorization::Denied { .. }
    ));
}

/// `TreasuryAuthorizationEvidence::new`：逐字段复制。
#[test]
fn authorization_evidence_new_copies_fields() {
    let evidence = TreasuryAuthorizationEvidence::new("d-1", "s-1", "scope-1");
    assert_eq!(evidence.decision_id, "d-1");
    assert_eq!(evidence.signed_by, "s-1");
    assert_eq!(evidence.scope, "scope-1");
}

/// `publication_semantics`：恒为 `(Date, Inferred, NotEligible)`。
#[test]
fn publication_semantics_is_date_inferred_not_eligible() {
    assert_eq!(
        publication_semantics(),
        (
            TimePrecision::Date,
            AvailabilityEvidence::Inferred,
            PitEligibility::NotEligible
        )
    );
}

/// `is_formal_pit_eligible`：恒为 `false`。
#[test]
fn is_formal_pit_eligible_is_false() {
    assert!(!is_formal_pit_eligible());
}

/// `parse_treasury_records`：合成信封可解析；未知字段原子失败。
#[test]
fn parse_treasury_records_rejects_unknown_field() {
    assert_eq!(
        parse_treasury_records(DS01)
            .expect("DS01 样本")
            .records
            .len(),
        2
    );
    assert_eq!(
        parse_treasury_records(DS05)
            .expect("DS05 样本")
            .records
            .len(),
        1
    );

    let unknown = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18",
        "tot_pub_debt_out_amt":1.0,"x":1}],"meta":{"count":1,"total-count":1,"total-pages":1}}"#;
    assert_eq!(
        parse_treasury_records(unknown)
            .expect_err("未知字段")
            .kind(),
        TreasuryErrorKind::SemanticallyRejected
    );
    assert_eq!(
        parse_treasury_records(CURVE).expect_err("曲线").kind(),
        TreasuryErrorKind::RoutedElsewhere
    );
}

/// 构造一条 DS05 记录（测试辅助）。
fn sample_ds05() -> TreasuryRecord {
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
            debt_held_public_amt: TreasuryAmount::Present(29_000_000_000_000.0),
            intragov_hold_amt: TreasuryAmount::Present(7_500_000_000_000.0),
            tot_pub_debt_out_amt: TreasuryAmount::Present(36_500_000_000_000.0),
        }),
    }
}
