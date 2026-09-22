#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
//! SDD 规格对照（特性 005）：把 `docs/标准.md` 的每个 `##` 章节转成可执行断言。
//!
//! 章节与断言函数须与 `docs/标准.md` 的 `##` 章节 1:1（检查器按标题逐字比对）。
//!
//! // SPEC-MAP: S-1 | 1. 需求标准 | assert_requirements_standard
//! // SPEC-MAP: S-2 | 2. 设计标准 | assert_design_standard
//! // SPEC-MAP: S-3 | 3. 数据集与采集面 | assert_datasets_and_collection_face
//! // SPEC-MAP: S-4 | 4. 曲线路由 | assert_curve_routing
//! // SPEC-MAP: S-5 | 5. 写入主权与校验面 | assert_write_authority_and_validation_surface
//! // SPEC-MAP: S-6 | 6. NY Fed 与 DS04 | assert_ny_fed_and_ds04
//! // SPEC-MAP: S-7 | 7. 解析与合成夹具 | assert_parse_and_synthetic_fixtures
//! // SPEC-MAP: S-8 | 8. publication 语义 | assert_publication_semantics
//! // SPEC-MAP: S-9 | 9. 非目标与门禁 | assert_non_goals_and_gates

use treasuryx::{
    authorize_treasury, guard_collection_dataset, guard_curve_product, guard_ny_fed,
    guard_write_authority, is_curve_product, is_formal_pit_eligible, is_validation_only_series,
    parse_treasury_records, publication_semantics, unit_for_validation_series, validate_record,
    AvailabilityEvidence, Date, FredValidationSeries, Frequency, NyFedDatasetId, Period,
    PitEligibility, TimePrecision, TreasuryAmount, TreasuryAuthorization,
    TreasuryAuthorizationEvidence, TreasuryDatasetId, TreasuryErrorKind, TreasuryMissingReason,
    TreasuryPayload, TreasuryRecord, TreasuryScope, Unit, CANONICAL_WRITE_OWNER,
    DRY_RUN_DECLARED_DATASETS, FD_VALIDATION_SERIES, LOSSY_SKELETON_DATASETS, NY_FED_DATASETS,
    NY_FED_DECISION, PHASE1_DATASETS, WIRE_COLLECT_DATASETS,
};

/// S-1：数据集常量、守卫、解析器、授权判定与 publication 语义五类能力均可达。
#[test]
fn assert_requirements_standard() {
    // 三个 ID 枚举均可用。
    assert_eq!(
        TreasuryDatasetId::parse_id("DS01").expect("合法"),
        TreasuryDatasetId::Ds01
    );
    assert_eq!(
        NyFedDatasetId::parse_id("FR01").expect("合法"),
        NyFedDatasetId::Fr01
    );
    assert_eq!(
        FredValidationSeries::parse_id("FD01").expect("合法"),
        FredValidationSeries::Fd01
    );

    // 记录值对象与信封可解析。
    let envelope =
        parse_treasury_records(include_str!("fixtures/ds05_envelope.json")).expect("合成样本");
    assert_eq!(envelope.records.len(), 1);

    // 守卫与会话判定可达。
    assert!(guard_collection_dataset(TreasuryDatasetId::Ds05).is_ok());
    assert!(matches!(
        authorize_treasury(TreasuryScope::OfflineFixtureOnly, None),
        TreasuryAuthorization::Denied { .. }
    ));

    // 错误统一为 TreasuryError，8 个变体各有语义。
    for error in [
        treasuryx::TreasuryError::Invalid("x".into()),
        treasuryx::TreasuryError::Missing("x".into()),
        treasuryx::TreasuryError::AuthorizationDenied("x".into()),
        treasuryx::TreasuryError::RoutedElsewhere("x".into()),
        treasuryx::TreasuryError::WriteAuthorityDenied("x".into()),
        treasuryx::TreasuryError::SemanticallyRejected("x".into()),
        treasuryx::TreasuryError::NotApplicable("x".into()),
        treasuryx::TreasuryError::Invariant("x".into()),
    ] {
        assert!(!error.to_string().is_empty(), "错误须可展示：{error:?}");
    }
}

/// S-2：模块切分（error / value / value::datasets / value::record / authz / pit / parse）
/// 体现在公开路径上，依赖方向单向。
#[test]
fn assert_design_standard() {
    // value 面：基础值对象经 crate 根可达。
    assert_eq!(
        Date::new(2026, 8, 18).expect("合法"),
        Date {
            year: 2026,
            month: 8,
            day: 18
        }
    );
    assert_eq!(Frequency::Daily, Frequency::Daily);

    // datasets 面：常量与守卫经 crate 根可达。
    assert_eq!(PHASE1_DATASETS.len(), 10);
    assert!(is_curve_product(TreasuryDatasetId::Ds07));

    // record 面：记录与校验入口经 crate 根可达。
    assert!(validate_record(&ds05_record()).is_ok());

    // authz / pit / parse 面。
    assert_eq!(
        TreasuryAuthorizationEvidence::new("d", "s", "sc").scope,
        "sc"
    );
    assert_eq!(publication_semantics().0, TimePrecision::Date);
    assert!(parse_treasury_records(include_str!("fixtures/ds01_envelope.json")).is_ok());
}

/// S-3：Phase 1 数据集全集、采集面与频率。
#[test]
fn assert_datasets_and_collection_face() {
    let ids: Vec<&str> = PHASE1_DATASETS.iter().map(|id| id.id()).collect();
    assert_eq!(
        ids,
        ["DS01", "DS02", "DS03", "DS04", "DS05", "DS06", "DS07", "DS08", "DS09", "DS10"]
    );
    assert_eq!(
        WIRE_COLLECT_DATASETS,
        [TreasuryDatasetId::Ds01, TreasuryDatasetId::Ds05]
    );
    assert_eq!(TreasuryDatasetId::Ds01.frequency(), Frequency::Daily);
    assert_eq!(TreasuryDatasetId::Ds04.frequency(), Frequency::Event);
    assert_eq!(TreasuryDatasetId::Ds10.frequency(), Frequency::Monthly);
    for dataset_id in PHASE1_DATASETS {
        let result = guard_collection_dataset(dataset_id);
        match dataset_id {
            TreasuryDatasetId::Ds01 | TreasuryDatasetId::Ds05 => assert!(result.is_ok()),
            TreasuryDatasetId::Ds06 | TreasuryDatasetId::Ds07 => assert_eq!(
                result.expect_err("曲线").kind(),
                TreasuryErrorKind::RoutedElsewhere
            ),
            TreasuryDatasetId::Ds04 => assert_eq!(
                result.expect_err("lossy").kind(),
                TreasuryErrorKind::SemanticallyRejected
            ),
            _ => assert_eq!(
                result.expect_err("采集面外").kind(),
                TreasuryErrorKind::NotApplicable
            ),
        }
    }
}

/// S-4：曲线产品一律拒绝并路由 `yieldx`，且解析层 fail-closed。
#[test]
fn assert_curve_routing() {
    for curve in [TreasuryDatasetId::Ds06, TreasuryDatasetId::Ds07] {
        assert!(is_curve_product(curve));
        assert_eq!(
            guard_curve_product(curve).expect_err("曲线").kind(),
            TreasuryErrorKind::RoutedElsewhere
        );
        assert_eq!(
            TreasuryDatasetId::parse_id(curve.id())
                .expect_err("parse_id 必须 fail-closed")
                .kind(),
            TreasuryErrorKind::RoutedElsewhere
        );
    }
    assert_eq!(
        parse_treasury_records(include_str!("fixtures/curve_envelope.json"))
            .expect_err("曲线样本必须拒绝")
            .kind(),
        TreasuryErrorKind::RoutedElsewhere
    );
}

/// S-5：写入主权归 `fredx`，本域只作永久校验面；单位不得混同。
#[test]
fn assert_write_authority_and_validation_surface() {
    assert_eq!(CANONICAL_WRITE_OWNER, "fredx");
    assert_eq!(FD_VALIDATION_SERIES.len(), 4);
    for series in FD_VALIDATION_SERIES {
        let series_id = series.series_id();
        assert!(is_validation_only_series(series_id));
        assert_eq!(series.canonical_write_owner(), "fredx");
        assert_eq!(
            guard_write_authority(series_id)
                .expect_err("越权写入")
                .kind(),
            TreasuryErrorKind::WriteAuthorityDenied,
            "{series_id}"
        );
    }
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
        unit_for_validation_series(FredValidationSeries::Fd02),
        "Billions 与 Millions 不得混同"
    );
}

/// S-6：NY Fed 整体 `NO-GO`；`DS04` 为 lossy skeleton 且不进采集常量。
#[test]
fn assert_ny_fed_and_ds04() {
    assert_eq!(NY_FED_DECISION, "NO-GO");
    assert_eq!(NY_FED_DATASETS.len(), 3);
    for dataset_id in NY_FED_DATASETS {
        assert_eq!(
            guard_ny_fed(dataset_id).expect_err("NO-GO").kind(),
            TreasuryErrorKind::NotApplicable
        );
    }

    assert_eq!(LOSSY_SKELETON_DATASETS, [TreasuryDatasetId::Ds04]);
    assert_eq!(
        DRY_RUN_DECLARED_DATASETS,
        [
            TreasuryDatasetId::Ds01,
            TreasuryDatasetId::Ds04,
            TreasuryDatasetId::Ds05
        ]
    );
    assert!(!WIRE_COLLECT_DATASETS.contains(&TreasuryDatasetId::Ds04));
    assert_eq!(
        guard_collection_dataset(TreasuryDatasetId::Ds04)
            .expect_err("lossy")
            .kind(),
        TreasuryErrorKind::SemanticallyRejected
    );
}

/// S-7：字段白名单原子失败、日期严格、重复身份拒绝、缺失具名、信封自洽。
#[test]
fn assert_parse_and_synthetic_fixtures() {
    let sample = include_str!("fixtures/ds05_envelope.json");
    let envelope = parse_treasury_records(sample).expect("合成样本可解析");
    assert_eq!(envelope.records.len(), 1);
    assert!(
        sample.contains("\"_synthetic\": true"),
        "夹具必须自带合成标注"
    );
    assert!(sample.contains("不构成任何证据"));

    // 未知字段 → 原子失败。
    let unknown = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18",
        "tot_pub_debt_out_amt":1.0,"x":1}],"meta":{"count":1,"total-count":1,"total-pages":1}}"#;
    assert_eq!(
        parse_treasury_records(unknown)
            .expect_err("未知字段")
            .kind(),
        TreasuryErrorKind::SemanticallyRejected
    );
    // 日期不严格。
    let bad_date = r#"{"_dataset":"DS05","data":[{"record_date":"2026-8-18",
        "tot_pub_debt_out_amt":1.0}],"meta":{"count":1,"total-count":1,"total-pages":1}}"#;
    assert!(parse_treasury_records(bad_date).is_err());
    // 重复身份 → 拒绝。
    let duplicate = r#"{"_dataset":"DS05","data":[
        {"record_date":"2026-08-18","tot_pub_debt_out_amt":1.0},
        {"record_date":"2026-08-18","tot_pub_debt_out_amt":2.0}],
        "meta":{"count":2,"total-count":2,"total-pages":1}}"#;
    assert_eq!(
        parse_treasury_records(duplicate)
            .expect_err("重复身份")
            .kind(),
        TreasuryErrorKind::Invalid
    );
    // 信封自洽：count 与行数不符 → 拒绝。
    let mismatch = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18",
        "tot_pub_debt_out_amt":1.0}],"meta":{"count":2,"total-count":2,"total-pages":1}}"#;
    assert!(parse_treasury_records(mismatch).is_err());
    // 缺失具名，不转 0。
    assert_eq!(
        TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull).present(),
        None
    );
}

/// S-8：publication 三元组恒为 `(Date, Inferred, NotEligible)`。
#[test]
fn assert_publication_semantics() {
    assert_eq!(
        publication_semantics(),
        (
            TimePrecision::Date,
            AvailabilityEvidence::Inferred,
            PitEligibility::NotEligible
        )
    );
    assert!(!is_formal_pit_eligible());
}

/// S-9：非目标与门禁——不联网、不读凭据、不依赖兄弟仓、不写权威键。
#[test]
fn assert_non_goals_and_gates() {
    // 解析器只吃字符串。
    let envelope =
        parse_treasury_records(include_str!("fixtures/ds01_envelope.json")).expect("可解析");
    assert_eq!(envelope.records.len(), 2);

    // 单 crate 独立：无 path 依赖，且依赖段只允许白名单三项。
    let manifest = include_str!("../Cargo.toml");
    assert!(!manifest.contains("path = \"../"), "不得有跨仓 path 依赖");
    let deps: Vec<&str> = manifest
        .lines()
        .skip_while(|line| *line != "[dependencies]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(
        deps.len(),
        3,
        "依赖段只允许 thiserror / serde / serde_json，实际：{deps:?}"
    );

    // 不写权威键：四个校验序列全部拒绝。
    for series_id in ["WALCL", "WTREGEN", "RRPONTSYD", "WRESBAL"] {
        assert!(guard_write_authority(series_id).is_err());
    }
    // 零端点字面量 / 零凭据读取：**运行期递归枚举** `src/` 下全部 `.rs`
    // （以 `CARGO_MANIFEST_DIR` 为根，不依赖 cwd；日后新增 src 文件自动纳入，不用手写清单）。
    let sources = collect_src_sources();
    assert!(!sources.is_empty(), "src/ 下必须至少有一个 .rs 源文件");
    for (path, text) in &sources {
        for (needle, why) in [
            ("http://", "不得含 http:// 端点字面量"),
            ("https://", "不得含 https:// 端点字面量"),
            ("env::var", "不得读取环境变量"),
            ("from_env", "不得读取环境变量/凭据"),
        ] {
            assert!(
                !text.contains(needle),
                "{} 含 {needle:?}：{why}",
                path.display()
            );
        }
    }

    assert_eq!(envelope.records[0].revision, None, "不得伪造修订标识");
}

/// 递归枚举 `src/` 下全部 `.rs` 的源码文本（不依赖 cwd，新增文件自动纳入）。
fn collect_src_sources() -> Vec<(std::path::PathBuf, String)> {
    /// 递归收集 `.rs` 路径。
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension() == Some(std::ffi::OsStr::new("rs")) {
                out.push(path);
            }
        }
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("src 源码可读");
            (path, text)
        })
        .collect()
}

/// 构造一条 DS05 记录（测试辅助）。
fn ds05_record() -> TreasuryRecord {
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
