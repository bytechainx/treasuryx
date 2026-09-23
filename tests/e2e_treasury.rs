#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
//! E2E（treasuryx）：在**真实合成夹具文件 + 真实临时文件 + 真实类型构造 + 真实守卫判定**上
//! 端到端执行 **全部** 公开接口。
//!
//! treasuryx 是纯离线库：无外部服务、无凭据、无网络、无进程级环境变量。它的 E2E 面就是：
//! 真实的离线信封 JSON（`tests/fixtures/*.json`，带 `_synthetic` 标注）、真实的临时文件
//! 往返（改写后的信封：`meta.count` 不一致 / 未知字段）、真实的类型构造与穷尽解构，以及
//! 真实执行的 fail-closed 守卫（曲线路由 / 采集面 / 写入主权 / NY Fed `NO-GO` / 授权判定）。
//!
//! 与分点单测不同，本文件的对齐对象是 `cargo +nightly public-api --simplified` 导出的
//! 完整公开面（156 条）：`fn` / `type` / `field` / `const` / `variant` 五类逐条登记在
//! [`E2E_MANIFEST`]，运行期由 `cover` 登记表核对「声明 = 实际执行」（缺一即失败）。
//!
//! **独立核对**：`scripts/verify-e2e-coverage.mjs` 会重新派生公开面与清单双向 diff，并用
//! `-C instrument-coverage` + `cargo-llvm-cov` 按函数取执行次数，断言每条公开 `fn` 的
//! count > 0；本文件内的登记表只是**声明**，不是唯一证据。
//!
//! 用例**不**需要任何凭据：临时文件名带 pid + 纳秒唯一化并在收尾删除且断言删除生效。
//! 本文件只保留**一个**顺序驱动的 `#[test]`，以消除跨用例竞态。
//!
//! ```text
//! cd /home/workspace/bytechainx/treasuryx
//! CARGO_TARGET_DIR=/home/workspace/bytechainx/.cargo/e2e-cov/treasuryx cargo test --test e2e_treasury
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use treasuryx::{
    authorize_treasury, guard_collection_dataset, guard_curve_product, guard_ny_fed,
    guard_write_authority, is_curve_product, is_formal_pit_eligible, is_validation_only_series,
    parse_treasury_records, publication_semantics, unit_for_validation_series, validate_record,
    AvailabilityEvidence, Date, FredValidationSeries, Frequency, NyFedDatasetId, Period,
    PitEligibility, TimePrecision, TreasuryAmount, TreasuryAuthorization,
    TreasuryAuthorizationEvidence, TreasuryCashBalance, TreasuryDatasetId, TreasuryDebtToPenny,
    TreasuryEnvelope, TreasuryEnvelopeMeta, TreasuryError, TreasuryErrorKind,
    TreasuryMissingReason, TreasuryPayload, TreasuryRecord, TreasuryResult, TreasuryScope, Unit,
    AUTHORIZED_SCOPE, CANONICAL_WRITE_OWNER, DECISION_ID, DRY_RUN_DECLARED_DATASETS,
    FD_VALIDATION_SERIES, LIVE_PREAUTH_CHANNEL, LIVE_REQUIRES_NEW_DECISION,
    LOSSY_SKELETON_DATASETS, NY_FED_DATASETS, NY_FED_DECISION, PHASE1_DATASETS,
    SUPERSEDED_DECISION_ID, WIRE_COLLECT_DATASETS,
};

/// 公开面清单：`(条目类别, 入口 id)`，由权威公开面派生并冻结。
///
/// 类别取值域：`fn` / `type` / `field` / `const` / `variant`。
/// 该清单由 `scripts/verify-e2e-coverage.mjs --write-manifest` 生成，MUST NOT 手抄。
const E2E_MANIFEST: &[(&str, &str)] = &[
    ("type", "TreasuryAuthorization"),
    ("variant", "TreasuryAuthorization::Authorized"),
    ("variant", "TreasuryAuthorization::Denied"),
    ("type", "TreasuryScope"),
    ("variant", "TreasuryScope::LiveCollection"),
    ("variant", "TreasuryScope::OfflineFixtureOnly"),
    ("type", "TreasuryAuthorizationEvidence"),
    ("field", "TreasuryAuthorizationEvidence::decision_id"),
    ("field", "TreasuryAuthorizationEvidence::scope"),
    ("field", "TreasuryAuthorizationEvidence::signed_by"),
    ("fn", "TreasuryAuthorizationEvidence::new"),
    ("const", "AUTHORIZED_SCOPE"),
    ("const", "DECISION_ID"),
    ("const", "LIVE_PREAUTH_CHANNEL"),
    ("const", "LIVE_REQUIRES_NEW_DECISION"),
    ("const", "SUPERSEDED_DECISION_ID"),
    ("fn", "authorize_treasury"),
    ("type", "TreasuryError"),
    ("variant", "TreasuryError::AuthorizationDenied"),
    ("variant", "TreasuryError::Invalid"),
    ("variant", "TreasuryError::Invariant"),
    ("variant", "TreasuryError::Missing"),
    ("variant", "TreasuryError::NotApplicable"),
    ("variant", "TreasuryError::RoutedElsewhere"),
    ("variant", "TreasuryError::SemanticallyRejected"),
    ("variant", "TreasuryError::WriteAuthorityDenied"),
    ("fn", "TreasuryError::is_retryable"),
    ("fn", "TreasuryError::kind"),
    ("type", "TreasuryErrorKind"),
    ("variant", "TreasuryErrorKind::AuthorizationDenied"),
    ("variant", "TreasuryErrorKind::Invalid"),
    ("variant", "TreasuryErrorKind::Invariant"),
    ("variant", "TreasuryErrorKind::Missing"),
    ("variant", "TreasuryErrorKind::NotApplicable"),
    ("variant", "TreasuryErrorKind::RoutedElsewhere"),
    ("variant", "TreasuryErrorKind::SemanticallyRejected"),
    ("variant", "TreasuryErrorKind::WriteAuthorityDenied"),
    ("type", "TreasuryResult"),
    ("fn", "parse_treasury_records"),
    ("type", "AvailabilityEvidence"),
    ("variant", "AvailabilityEvidence::Calendar"),
    ("variant", "AvailabilityEvidence::Inferred"),
    ("variant", "AvailabilityEvidence::Official"),
    ("type", "PitEligibility"),
    ("variant", "PitEligibility::Formal"),
    ("variant", "PitEligibility::NotEligible"),
    ("type", "TimePrecision"),
    ("variant", "TimePrecision::Date"),
    ("variant", "TimePrecision::Instant"),
    ("fn", "is_formal_pit_eligible"),
    ("fn", "publication_semantics"),
    ("type", "FredValidationSeries"),
    ("variant", "FredValidationSeries::Fd01"),
    ("variant", "FredValidationSeries::Fd02"),
    ("variant", "FredValidationSeries::Fd03"),
    ("variant", "FredValidationSeries::Fd04"),
    ("fn", "FredValidationSeries::canonical_write_owner"),
    ("fn", "FredValidationSeries::id"),
    ("fn", "FredValidationSeries::label"),
    ("fn", "FredValidationSeries::parse_id"),
    ("fn", "FredValidationSeries::series_id"),
    ("type", "Frequency"),
    ("variant", "Frequency::Annual"),
    ("variant", "Frequency::Daily"),
    ("variant", "Frequency::Event"),
    ("variant", "Frequency::Irregular"),
    ("variant", "Frequency::Monthly"),
    ("variant", "Frequency::Quarterly"),
    ("variant", "Frequency::Weekly"),
    ("type", "NyFedDatasetId"),
    ("variant", "NyFedDatasetId::Fr01"),
    ("variant", "NyFedDatasetId::Fr02"),
    ("variant", "NyFedDatasetId::Fr03"),
    ("fn", "NyFedDatasetId::frequency"),
    ("fn", "NyFedDatasetId::id"),
    ("fn", "NyFedDatasetId::parse_id"),
    ("type", "Period"),
    ("variant", "Period::Day"),
    ("variant", "Period::Event"),
    ("variant", "Period::Month"),
    ("variant", "Period::Quarter"),
    ("variant", "Period::Year"),
    ("fn", "Period::parse_day"),
    ("type", "TreasuryAmount"),
    ("variant", "TreasuryAmount::Missing"),
    ("variant", "TreasuryAmount::Present"),
    ("fn", "TreasuryAmount::is_missing"),
    ("fn", "TreasuryAmount::present"),
    ("type", "TreasuryDatasetId"),
    ("variant", "TreasuryDatasetId::Ds01"),
    ("variant", "TreasuryDatasetId::Ds02"),
    ("variant", "TreasuryDatasetId::Ds03"),
    ("variant", "TreasuryDatasetId::Ds04"),
    ("variant", "TreasuryDatasetId::Ds05"),
    ("variant", "TreasuryDatasetId::Ds06"),
    ("variant", "TreasuryDatasetId::Ds07"),
    ("variant", "TreasuryDatasetId::Ds08"),
    ("variant", "TreasuryDatasetId::Ds09"),
    ("variant", "TreasuryDatasetId::Ds10"),
    ("fn", "TreasuryDatasetId::frequency"),
    ("fn", "TreasuryDatasetId::id"),
    ("fn", "TreasuryDatasetId::parse_id"),
    ("type", "TreasuryMissingReason"),
    ("variant", "TreasuryMissingReason::ExplicitNull"),
    ("variant", "TreasuryMissingReason::SourceOmitted"),
    ("type", "TreasuryPayload"),
    ("variant", "TreasuryPayload::CashBalance"),
    ("variant", "TreasuryPayload::DebtToPenny"),
    ("type", "Unit"),
    ("variant", "Unit::BillionsOfUsd"),
    ("variant", "Unit::MillionsOfUsd"),
    ("variant", "Unit::Unknown"),
    ("type", "Date"),
    ("field", "Date::day"),
    ("field", "Date::month"),
    ("field", "Date::year"),
    ("fn", "Date::new"),
    ("fn", "Date::parse"),
    ("type", "TreasuryCashBalance"),
    ("field", "TreasuryCashBalance::account_type"),
    ("field", "TreasuryCashBalance::close_today_bal"),
    ("field", "TreasuryCashBalance::open_today_bal"),
    ("type", "TreasuryDebtToPenny"),
    ("field", "TreasuryDebtToPenny::debt_held_public_amt"),
    ("field", "TreasuryDebtToPenny::intragov_hold_amt"),
    ("field", "TreasuryDebtToPenny::tot_pub_debt_out_amt"),
    ("type", "TreasuryEnvelope"),
    ("field", "TreasuryEnvelope::meta"),
    ("field", "TreasuryEnvelope::records"),
    ("type", "TreasuryEnvelopeMeta"),
    ("field", "TreasuryEnvelopeMeta::count"),
    ("field", "TreasuryEnvelopeMeta::total_count"),
    ("field", "TreasuryEnvelopeMeta::total_pages"),
    ("type", "TreasuryRecord"),
    ("field", "TreasuryRecord::dataset_id"),
    ("field", "TreasuryRecord::frequency"),
    ("field", "TreasuryRecord::payload"),
    ("field", "TreasuryRecord::period"),
    ("field", "TreasuryRecord::revision"),
    ("field", "TreasuryRecord::unit"),
    ("const", "CANONICAL_WRITE_OWNER"),
    ("const", "DRY_RUN_DECLARED_DATASETS"),
    ("const", "FD_VALIDATION_SERIES"),
    ("const", "LOSSY_SKELETON_DATASETS"),
    ("const", "NY_FED_DATASETS"),
    ("const", "NY_FED_DECISION"),
    ("const", "PHASE1_DATASETS"),
    ("const", "WIRE_COLLECT_DATASETS"),
    ("fn", "guard_collection_dataset"),
    ("fn", "guard_curve_product"),
    ("fn", "guard_ny_fed"),
    ("fn", "guard_write_authority"),
    ("fn", "is_curve_product"),
    ("fn", "is_validation_only_series"),
    ("fn", "unit_for_validation_series"),
    ("fn", "validate_record"),
];

/// 覆盖登记表：只登记**真实发生**的调用/读取，不登记「计划要调用」。
mod cover {
    use std::collections::BTreeSet;
    use std::sync::{Mutex, OnceLock};

    static EXECUTED: OnceLock<Mutex<BTreeSet<(&'static str, &'static str)>>> = OnceLock::new();

    fn log() -> &'static Mutex<BTreeSet<(&'static str, &'static str)>> {
        EXECUTED.get_or_init(|| Mutex::new(BTreeSet::new()))
    }

    /// 登记一次真实执行。清单外的 `(类别, id)` 立即 panic，防止调用点与清单漂移。
    pub fn hit(kind: &'static str, id: &'static str) {
        assert!(
            super::E2E_MANIFEST
                .iter()
                .any(|(declared_kind, declared_id)| *declared_kind == kind && *declared_id == id),
            "登记了清单外的公开条目：{kind} {id}"
        );
        log().lock().expect("覆盖登记表锁中毒").insert((kind, id));
    }

    pub fn executed() -> BTreeSet<(&'static str, &'static str)> {
        log().lock().expect("覆盖登记表锁中毒").clone()
    }
}

/// 覆盖登记的简写入口（保持调用点可读）。
fn hit(kind: &'static str, id: &'static str) {
    cover::hit(kind, id);
}

/// 清单自身良构：类别取值域合法、`(类别, id)` 不重复。
fn assert_manifest_wellformed() {
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for &(kind, id) in E2E_MANIFEST {
        assert!(
            matches!(kind, "fn" | "type" | "field" | "const" | "variant"),
            "未知条目类别 {kind}（id={id}）"
        );
        assert!(seen.insert((kind, id)), "清单重复条目：{kind} {id}");
    }
    assert!(!E2E_MANIFEST.is_empty(), "清单不得为空");
}

/// 收尾断言：声明集合与执行集合必须**双向相等**。
fn assert_coverage_complete() {
    let declared: BTreeSet<(&'static str, &'static str)> = E2E_MANIFEST.iter().copied().collect();
    let executed = cover::executed();

    let missing: Vec<&(&str, &str)> = declared.difference(&executed).collect();
    let ghost: Vec<&(&str, &str)> = executed.difference(&declared).collect();

    assert!(
        missing.is_empty(),
        "以下 {} 条公开条目被声明却未执行：{missing:?}",
        missing.len()
    );
    assert!(
        ghost.is_empty(),
        "以下 {} 条执行未登记在清单：{ghost:?}",
        ghost.len()
    );
    eprintln!(
        "E2E 覆盖：{}/{} 条公开条目全部执行（treasuryx）",
        executed.len(),
        declared.len()
    );
}

/// 临时文件路径：`<temp_dir>/<prefix>_<pid>_<纳秒>.json`（进程内唯一）。
fn unique_temp_file(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("系统时钟应晚于 UNIX_EPOCH")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{}_{}.json", std::process::id(), nanos))
}

/// 真实读取 `tests/fixtures/<name>`（相对 `CARGO_MANIFEST_DIR`，不用 `include_str!`）。
fn read_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读取合成夹具文件失败 {}：{error}", path.display()))
}

/// 构造一条合法 DS01 记录（供 `validate_record` 两侧复用）。
fn ds01_record() -> TreasuryRecord {
    TreasuryRecord {
        dataset_id: TreasuryDatasetId::Ds01,
        period: Period::Day(Date::new(2026, 8, 18).expect("合法日期")),
        frequency: Frequency::Daily,
        unit: Unit::Unknown,
        revision: None,
        payload: TreasuryPayload::CashBalance(TreasuryCashBalance {
            account_type: "Federal Reserve Account".into(),
            open_today_bal: TreasuryAmount::Present(830_000.0),
            close_today_bal: TreasuryAmount::Present(825_500.0),
        }),
    }
}

/// 构造一条合法 DS05 记录（供 `validate_record` 两侧复用）。
fn ds05_record() -> TreasuryRecord {
    TreasuryRecord {
        dataset_id: TreasuryDatasetId::Ds05,
        period: Period::Day(Date::new(2026, 8, 18).expect("合法日期")),
        frequency: Frequency::Daily,
        unit: Unit::Unknown,
        revision: None,
        payload: TreasuryPayload::DebtToPenny(TreasuryDebtToPenny {
            debt_held_public_amt: TreasuryAmount::Present(29_000_000_000_000.0),
            intragov_hold_amt: TreasuryAmount::Present(7_500_000_000_000.0),
            tot_pub_debt_out_amt: TreasuryAmount::Present(36_500_000_000_000.0),
        }),
    }
}

/// 断言一次授权判定落到 `Denied` 并取出理由。
fn expect_denied(result: TreasuryAuthorization) -> String {
    match result {
        TreasuryAuthorization::Denied { reason } => reason,
        other => panic!("应为 Denied，实得 {other:?}"),
    }
}

/// 阶段 1：13 个公开常量逐条取值断言（含数组长度与成员）。
fn phase_constants() {
    // —— authz 的 5 个声明层常量 ——
    hit("const", "DECISION_ID");
    assert_eq!(DECISION_ID, "TREASURY-B1-2026-08-20-OFFLINE");
    hit("const", "SUPERSEDED_DECISION_ID");
    assert_eq!(SUPERSEDED_DECISION_ID, "TREASURY-PROD-2026-08-17-approve");
    assert_ne!(DECISION_ID, SUPERSEDED_DECISION_ID, "新旧决策 ID 不得相同");
    hit("const", "AUTHORIZED_SCOPE");
    assert_eq!(AUTHORIZED_SCOPE, "offline_fixture_only");
    hit("const", "LIVE_PREAUTH_CHANNEL");
    assert!(!LIVE_PREAUTH_CHANNEL.is_empty(), "预授权通道说明不得为空");
    hit("const", "LIVE_REQUIRES_NEW_DECISION");
    assert_eq!(
        u8::from(LIVE_REQUIRES_NEW_DECISION),
        1,
        "live 恒须另批决策 ID + B2–B8 门禁"
    );

    // —— value 的 8 个常量 ——
    hit("const", "CANONICAL_WRITE_OWNER");
    assert_eq!(CANONICAL_WRITE_OWNER, "fredx");
    hit("const", "NY_FED_DECISION");
    assert_eq!(NY_FED_DECISION, "NO-GO");

    hit("const", "PHASE1_DATASETS");
    assert_eq!(PHASE1_DATASETS.len(), 10, "Phase 1 数据集全集为 10");
    let phase1_ids: Vec<&str> = PHASE1_DATASETS.iter().map(|id| id.id()).collect();
    assert_eq!(
        phase1_ids,
        ["DS01", "DS02", "DS03", "DS04", "DS05", "DS06", "DS07", "DS08", "DS09", "DS10"]
    );

    hit("const", "WIRE_COLLECT_DATASETS");
    assert_eq!(
        WIRE_COLLECT_DATASETS,
        [TreasuryDatasetId::Ds01, TreasuryDatasetId::Ds05],
        "采集面恰为 DS01 + DS05"
    );
    assert_eq!(WIRE_COLLECT_DATASETS.len(), 2);

    hit("const", "DRY_RUN_DECLARED_DATASETS");
    assert_eq!(DRY_RUN_DECLARED_DATASETS.len(), 3);
    assert!(
        DRY_RUN_DECLARED_DATASETS.contains(&TreasuryDatasetId::Ds04),
        "DS04 只出现在 dry-run 声明集"
    );
    assert!(!WIRE_COLLECT_DATASETS.contains(&TreasuryDatasetId::Ds04));

    hit("const", "LOSSY_SKELETON_DATASETS");
    assert_eq!(LOSSY_SKELETON_DATASETS, [TreasuryDatasetId::Ds04]);

    hit("const", "NY_FED_DATASETS");
    assert_eq!(NY_FED_DATASETS.len(), 3);
    let ny_fed_ids: Vec<&str> = NY_FED_DATASETS.iter().map(|id| id.id()).collect();
    assert_eq!(ny_fed_ids, ["FR01", "FR02", "FR03"]);

    hit("const", "FD_VALIDATION_SERIES");
    assert_eq!(FD_VALIDATION_SERIES.len(), 4);
    let validation_ids: Vec<&str> = FD_VALIDATION_SERIES.iter().map(|s| s.series_id()).collect();
    assert_eq!(validation_ids, ["WALCL", "WTREGEN", "RRPONTSYD", "WRESBAL"]);
}

/// 阶段 2：基础值对象面（`Date` / `Period` / `Frequency` / `Unit` / `TreasuryAmount` /
/// `TreasuryMissingReason` / `TreasuryPayload`）——变体逐个构造、字段穷尽解构。
fn phase_value_types() {
    // —— Date：构造 / 解析 / 闰年边界 + 3 个公开字段穷尽解构 ——
    hit("type", "Date");
    hit("fn", "Date::new");
    let date = Date::new(2026, 8, 18).expect("合法日期");
    let Date { year, month, day } = date;
    hit("field", "Date::year");
    assert_eq!(year, 2026);
    hit("field", "Date::month");
    assert_eq!(month, 8);
    hit("field", "Date::day");
    assert_eq!(day, 18);
    assert!(Date::new(2026, 13, 1).is_err(), "13 月必须被拒绝");
    assert!(Date::new(2026, 0, 1).is_err(), "0 月必须被拒绝");
    assert!(Date::new(2026, 2, 29).is_err(), "2026 非闰年");
    assert!(Date::new(2024, 2, 29).is_ok(), "2024 是闰年");
    assert!(Date::new(1900, 2, 29).is_err(), "1900 非闰年");
    assert!(Date::new(2000, 2, 29).is_ok(), "2000 是闰年");
    assert!(Date::new(2026, 1, 0).is_err(), "0 日必须被拒绝");
    assert!(Date::new(2026, 4, 31).is_err(), "4 月只有 30 天");

    hit("fn", "Date::parse");
    assert_eq!(Date::parse("2026-08-18").expect("严格 ISO"), date);
    assert!(Date::parse("2024-02-29").is_ok(), "闰年 2 月 29 日合法");
    for bad in [
        "2026-8-18",
        "2026/08/18",
        "2026-08-18T00:00:00",
        "2026-08-8",
        "2026-008-18",
        "2026-08-1a",
        "2026-08-18 ",
        "",
    ] {
        assert!(Date::parse(bad).is_err(), "必须拒绝：{bad}");
    }

    // —— Period：5 个变体 ——
    hit("type", "Period");
    hit("variant", "Period::Day");
    let day_period = Period::Day(date);
    hit("variant", "Period::Month");
    let month_period = Period::Month {
        year: 2026,
        month: 8,
    };
    hit("variant", "Period::Quarter");
    let quarter_period = Period::Quarter {
        year: 2026,
        quarter: 3,
    };
    hit("variant", "Period::Year");
    let year_period = Period::Year(2026);
    hit("variant", "Period::Event");
    let event_period = Period::Event { date };
    let periods = [
        day_period,
        month_period,
        quarter_period,
        year_period,
        event_period,
    ];
    let distinct: BTreeSet<String> = periods.iter().map(|p| format!("{p:?}")).collect();
    assert_eq!(distinct.len(), 5, "5 个期间变体必须互异");

    hit("fn", "Period::parse_day");
    let parsed_day = Period::parse_day("2026-08-18").expect("合法日");
    assert!(
        matches!(parsed_day, Period::Day(_)),
        "parse_day 必须是单日期间"
    );
    assert_eq!(parsed_day, day_period);
    assert!(
        Period::parse_day("2026-8-18").is_err(),
        "非严格 ISO 必须拒绝"
    );

    // —— Frequency：7 个变体 ——
    hit("type", "Frequency");
    hit("variant", "Frequency::Daily");
    let daily = Frequency::Daily;
    hit("variant", "Frequency::Weekly");
    let weekly = Frequency::Weekly;
    hit("variant", "Frequency::Monthly");
    let monthly = Frequency::Monthly;
    hit("variant", "Frequency::Quarterly");
    let quarterly = Frequency::Quarterly;
    hit("variant", "Frequency::Annual");
    let annual = Frequency::Annual;
    hit("variant", "Frequency::Event");
    let event = Frequency::Event;
    hit("variant", "Frequency::Irregular");
    let irregular = Frequency::Irregular;
    let frequencies = [daily, weekly, monthly, quarterly, annual, event, irregular];
    let distinct: BTreeSet<String> = frequencies.iter().map(|f| format!("{f:?}")).collect();
    assert_eq!(distinct.len(), 7, "频率取值域恰为 7 个");

    // —— Unit：3 个变体，Billions 与 Millions 不得混同 ——
    hit("type", "Unit");
    hit("variant", "Unit::Unknown");
    let unknown_unit = Unit::Unknown;
    hit("variant", "Unit::BillionsOfUsd");
    let billions = Unit::BillionsOfUsd;
    hit("variant", "Unit::MillionsOfUsd");
    let millions = Unit::MillionsOfUsd;
    assert_ne!(billions, millions, "量级不同不得混同");
    assert_ne!(unknown_unit, millions);

    // —— TreasuryMissingReason：2 个具名缺失原因 ——
    hit("type", "TreasuryMissingReason");
    hit("variant", "TreasuryMissingReason::SourceOmitted");
    let omitted = TreasuryMissingReason::SourceOmitted;
    hit("variant", "TreasuryMissingReason::ExplicitNull");
    let explicit_null = TreasuryMissingReason::ExplicitNull;
    assert_ne!(omitted, explicit_null);

    // —— TreasuryAmount：2 个变体 + present / is_missing ——
    hit("type", "TreasuryAmount");
    hit("variant", "TreasuryAmount::Present");
    let present = TreasuryAmount::Present(1.5);
    hit("variant", "TreasuryAmount::Missing");
    let missing = TreasuryAmount::Missing(omitted);
    hit("fn", "TreasuryAmount::present");
    assert_eq!(present.present(), Some(1.5));
    assert_eq!(missing.present(), None, "缺失不得静默转 0");
    hit("fn", "TreasuryAmount::is_missing");
    assert!(missing.is_missing());
    assert!(!present.is_missing());
    assert_eq!(TreasuryAmount::Present(0.0).present(), Some(0.0));

    // —— TreasuryPayload / TreasuryCashBalance / TreasuryDebtToPenny：类型面 ——
    hit("type", "TreasuryPayload");
    hit("variant", "TreasuryPayload::CashBalance");
    let cash = TreasuryPayload::CashBalance(TreasuryCashBalance {
        account_type: "Federal Reserve Account".into(),
        open_today_bal: present,
        close_today_bal: missing,
    });
    hit("variant", "TreasuryPayload::DebtToPenny");
    let debt = TreasuryPayload::DebtToPenny(TreasuryDebtToPenny {
        debt_held_public_amt: present,
        intragov_hold_amt: missing,
        tot_pub_debt_out_amt: present,
    });
    hit("type", "TreasuryCashBalance");
    hit("type", "TreasuryDebtToPenny");
    assert_ne!(cash, debt);

    // —— TreasuryRecord：类型 + 6 个公开字段穷尽解构（不写 `..`）——
    hit("type", "TreasuryRecord");
    let record = ds01_record();
    let TreasuryRecord {
        dataset_id,
        period,
        frequency,
        unit,
        revision,
        payload,
    } = record;
    hit("field", "TreasuryRecord::dataset_id");
    assert_eq!(dataset_id, TreasuryDatasetId::Ds01);
    hit("field", "TreasuryRecord::period");
    assert_eq!(period, Period::Day(date));
    hit("field", "TreasuryRecord::frequency");
    assert_eq!(frequency, Frequency::Daily);
    hit("field", "TreasuryRecord::unit");
    assert_eq!(unit, Unit::Unknown);
    hit("field", "TreasuryRecord::revision");
    assert!(revision.is_none(), "清单未声明 revision 面，不得伪造");
    hit("field", "TreasuryRecord::payload");
    match payload {
        TreasuryPayload::CashBalance(row) => {
            assert_eq!(row.open_today_bal.present(), Some(830_000.0))
        }
        other => panic!("DS01 载荷应为 CashBalance，实得 {other:?}"),
    }
}

/// 阶段 3：`validate_record` 的成功与**全部**拒绝分支。
fn phase_validate_record() {
    hit("fn", "validate_record");

    assert!(
        validate_record(&ds01_record()).is_ok(),
        "合法 DS01 必须通过"
    );
    assert!(
        validate_record(&ds05_record()).is_ok(),
        "合法 DS05 必须通过"
    );

    // 载荷与数据集不匹配 → Invalid。
    let mut mismatch = ds01_record();
    mismatch.payload = ds05_record().payload;
    assert_eq!(
        validate_record(&mismatch).expect_err("载荷错配").kind(),
        TreasuryErrorKind::Invalid
    );

    // 曲线产品 → RoutedElsewhere。
    let mut curve = ds01_record();
    curve.dataset_id = TreasuryDatasetId::Ds06;
    assert_eq!(
        validate_record(&curve).expect_err("曲线").kind(),
        TreasuryErrorKind::RoutedElsewhere
    );

    // DS04 lossy skeleton → SemanticallyRejected。
    let mut lossy = ds01_record();
    lossy.dataset_id = TreasuryDatasetId::Ds04;
    assert_eq!(
        validate_record(&lossy).expect_err("lossy").kind(),
        TreasuryErrorKind::SemanticallyRejected
    );

    // 采集面外 → NotApplicable。
    let mut outside = ds01_record();
    outside.dataset_id = TreasuryDatasetId::Ds08;
    outside.frequency = Frequency::Monthly;
    assert_eq!(
        validate_record(&outside).expect_err("采集面外").kind(),
        TreasuryErrorKind::NotApplicable
    );

    // 单位漂移（非 Unknown）→ Invalid。
    let mut wrong_unit = ds01_record();
    wrong_unit.unit = Unit::MillionsOfUsd;
    assert_eq!(
        validate_record(&wrong_unit).expect_err("单位漂移").kind(),
        TreasuryErrorKind::Invalid
    );

    // 期间非单日 → Invalid。
    let mut wrong_period = ds01_record();
    wrong_period.period = Period::Month {
        year: 2026,
        month: 8,
    };
    assert_eq!(
        validate_record(&wrong_period).expect_err("非单日").kind(),
        TreasuryErrorKind::Invalid
    );

    // 频率与数据集不符 → Invalid。
    let mut wrong_frequency = ds01_record();
    wrong_frequency.frequency = Frequency::Weekly;
    assert_eq!(
        validate_record(&wrong_frequency)
            .expect_err("频率漂移")
            .kind(),
        TreasuryErrorKind::Invalid
    );

    // 伪造 revision → Invalid。
    let mut fake_revision = ds01_record();
    fake_revision.revision = Some("1".into());
    assert_eq!(
        validate_record(&fake_revision)
            .expect_err("伪造修订标识")
            .kind(),
        TreasuryErrorKind::Invalid
    );

    // 非有限金额 → Invalid。
    let mut non_finite = ds05_record();
    non_finite.payload = TreasuryPayload::DebtToPenny(TreasuryDebtToPenny {
        debt_held_public_amt: TreasuryAmount::Present(f64::NAN),
        intragov_hold_amt: TreasuryAmount::Missing(TreasuryMissingReason::SourceOmitted),
        tot_pub_debt_out_amt: TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull),
    });
    assert_eq!(
        validate_record(&non_finite).expect_err("NaN").kind(),
        TreasuryErrorKind::Invalid
    );

    // 账户类型空白 → Missing。
    let mut blank_account = ds01_record();
    blank_account.payload = TreasuryPayload::CashBalance(TreasuryCashBalance {
        account_type: "  ".into(),
        open_today_bal: TreasuryAmount::Present(1.0),
        close_today_bal: TreasuryAmount::Present(1.0),
    });
    assert_eq!(
        validate_record(&blank_account)
            .expect_err("空白账户类型")
            .kind(),
        TreasuryErrorKind::Missing
    );

    // 金额全缺 → Missing（不得静默补 0）。
    let mut all_missing = ds05_record();
    all_missing.payload = TreasuryPayload::DebtToPenny(TreasuryDebtToPenny {
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
}

/// 阶段 4：错误面（`TreasuryError` 8 变体 + `TreasuryErrorKind` 8 变体 + `TreasuryResult`）。
fn phase_error_types() {
    hit("type", "TreasuryError");
    let cases: [(&'static str, TreasuryError, TreasuryErrorKind, bool); 8] = [
        (
            "TreasuryError::Invalid",
            TreasuryError::Invalid("e2e".into()),
            TreasuryErrorKind::Invalid,
            false,
        ),
        (
            "TreasuryError::Missing",
            TreasuryError::Missing("e2e".into()),
            TreasuryErrorKind::Missing,
            false,
        ),
        (
            "TreasuryError::AuthorizationDenied",
            TreasuryError::AuthorizationDenied("e2e".into()),
            TreasuryErrorKind::AuthorizationDenied,
            false,
        ),
        (
            "TreasuryError::RoutedElsewhere",
            TreasuryError::RoutedElsewhere("e2e".into()),
            TreasuryErrorKind::RoutedElsewhere,
            false,
        ),
        (
            "TreasuryError::WriteAuthorityDenied",
            TreasuryError::WriteAuthorityDenied("e2e".into()),
            TreasuryErrorKind::WriteAuthorityDenied,
            false,
        ),
        (
            "TreasuryError::SemanticallyRejected",
            TreasuryError::SemanticallyRejected("e2e".into()),
            TreasuryErrorKind::SemanticallyRejected,
            false,
        ),
        (
            "TreasuryError::NotApplicable",
            TreasuryError::NotApplicable("e2e".into()),
            TreasuryErrorKind::NotApplicable,
            false,
        ),
        (
            "TreasuryError::Invariant",
            TreasuryError::Invariant("e2e".into()),
            TreasuryErrorKind::Invariant,
            true,
        ),
    ];
    for (variant, error, kind, retryable) in cases {
        hit("variant", variant);
        hit("fn", "TreasuryError::kind");
        assert_eq!(error.kind(), kind, "{variant} 的分类不符");
        hit("fn", "TreasuryError::is_retryable");
        assert_eq!(
            error.is_retryable(),
            retryable,
            "{variant} 的可重试分类不符"
        );
        assert!(
            !error.to_string().is_empty(),
            "{variant} 的 Display 不得为空"
        );
    }

    hit("type", "TreasuryErrorKind");
    let kinds: [(&'static str, TreasuryErrorKind); 8] = [
        ("TreasuryErrorKind::Invalid", TreasuryErrorKind::Invalid),
        ("TreasuryErrorKind::Missing", TreasuryErrorKind::Missing),
        (
            "TreasuryErrorKind::AuthorizationDenied",
            TreasuryErrorKind::AuthorizationDenied,
        ),
        (
            "TreasuryErrorKind::RoutedElsewhere",
            TreasuryErrorKind::RoutedElsewhere,
        ),
        (
            "TreasuryErrorKind::WriteAuthorityDenied",
            TreasuryErrorKind::WriteAuthorityDenied,
        ),
        (
            "TreasuryErrorKind::SemanticallyRejected",
            TreasuryErrorKind::SemanticallyRejected,
        ),
        (
            "TreasuryErrorKind::NotApplicable",
            TreasuryErrorKind::NotApplicable,
        ),
        ("TreasuryErrorKind::Invariant", TreasuryErrorKind::Invariant),
    ];
    let mut seen = BTreeSet::new();
    for (id, kind) in kinds {
        hit("variant", id);
        assert!(seen.insert(format!("{kind:?}")), "{id} 与其它变体重复");
    }
    assert_eq!(seen.len(), 8);

    hit("type", "TreasuryResult");
    let ok: TreasuryResult<u8> = Ok(7);
    match ok {
        Ok(value) => assert_eq!(value, 7),
        Err(error) => panic!("必须是 Ok 分支，实得 {error}"),
    }
    let err: TreasuryResult<u8> = Err(TreasuryError::Invalid("e2e".into()));
    assert!(err.is_err());
}

/// 阶段 5：publication 语义三元组（`TimePrecision` / `AvailabilityEvidence` /
/// `PitEligibility`）。
fn phase_pit() {
    hit("type", "TimePrecision");
    hit("variant", "TimePrecision::Date");
    hit("variant", "TimePrecision::Instant");
    assert_ne!(TimePrecision::Date, TimePrecision::Instant);

    hit("type", "AvailabilityEvidence");
    hit("variant", "AvailabilityEvidence::Official");
    hit("variant", "AvailabilityEvidence::Calendar");
    hit("variant", "AvailabilityEvidence::Inferred");
    let layers = [
        AvailabilityEvidence::Official,
        AvailabilityEvidence::Calendar,
        AvailabilityEvidence::Inferred,
    ];
    let distinct: BTreeSet<String> = layers.iter().map(|l| format!("{l:?}")).collect();
    assert_eq!(distinct.len(), 3, "3 层证据必须互异");

    hit("type", "PitEligibility");
    hit("variant", "PitEligibility::Formal");
    hit("variant", "PitEligibility::NotEligible");
    assert_ne!(PitEligibility::Formal, PitEligibility::NotEligible);

    hit("fn", "publication_semantics");
    let semantics = publication_semantics();
    assert_eq!(
        semantics,
        (
            TimePrecision::Date,
            AvailabilityEvidence::Inferred,
            PitEligibility::NotEligible
        ),
        "本源的 publication 语义三元组必须钉死"
    );
    assert_ne!(
        semantics.0,
        TimePrecision::Instant,
        "MUST NOT 升格为 Instant"
    );
    assert_eq!(semantics, publication_semantics(), "三元组必须是常量");

    hit("fn", "is_formal_pit_eligible");
    assert!(!is_formal_pit_eligible(), "推断层不得进入正式 PIT");
}

/// 阶段 6：数据集事实面与四类守卫（曲线路由 / 采集面 / 写入主权 / NY Fed `NO-GO`）。
fn phase_datasets() {
    // —— TreasuryDatasetId：10 个变体 + id / frequency / parse_id ——
    hit("type", "TreasuryDatasetId");
    let datasets = [
        (
            "TreasuryDatasetId::Ds01",
            TreasuryDatasetId::Ds01,
            "DS01",
            Frequency::Daily,
        ),
        (
            "TreasuryDatasetId::Ds02",
            TreasuryDatasetId::Ds02,
            "DS02",
            Frequency::Daily,
        ),
        (
            "TreasuryDatasetId::Ds03",
            TreasuryDatasetId::Ds03,
            "DS03",
            Frequency::Daily,
        ),
        (
            "TreasuryDatasetId::Ds04",
            TreasuryDatasetId::Ds04,
            "DS04",
            Frequency::Event,
        ),
        (
            "TreasuryDatasetId::Ds05",
            TreasuryDatasetId::Ds05,
            "DS05",
            Frequency::Daily,
        ),
        (
            "TreasuryDatasetId::Ds06",
            TreasuryDatasetId::Ds06,
            "DS06",
            Frequency::Daily,
        ),
        (
            "TreasuryDatasetId::Ds07",
            TreasuryDatasetId::Ds07,
            "DS07",
            Frequency::Daily,
        ),
        (
            "TreasuryDatasetId::Ds08",
            TreasuryDatasetId::Ds08,
            "DS08",
            Frequency::Monthly,
        ),
        (
            "TreasuryDatasetId::Ds09",
            TreasuryDatasetId::Ds09,
            "DS09",
            Frequency::Monthly,
        ),
        (
            "TreasuryDatasetId::Ds10",
            TreasuryDatasetId::Ds10,
            "DS10",
            Frequency::Monthly,
        ),
    ];
    for (id, dataset, literal, frequency) in datasets {
        hit("variant", id);
        hit("fn", "TreasuryDatasetId::id");
        assert_eq!(dataset.id(), literal, "{id} 的登记形不符");
        hit("fn", "TreasuryDatasetId::frequency");
        assert_eq!(dataset.frequency(), frequency, "{id} 的频率不符");
    }

    hit("fn", "TreasuryDatasetId::parse_id");
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
            .expect_err("未知 ID")
            .kind(),
        TreasuryErrorKind::Invalid
    );

    // —— NyFedDatasetId：3 个变体 + id / frequency / parse_id ——
    hit("type", "NyFedDatasetId");
    let ny_fed = [
        (
            "NyFedDatasetId::Fr01",
            NyFedDatasetId::Fr01,
            "FR01",
            Frequency::Daily,
        ),
        (
            "NyFedDatasetId::Fr02",
            NyFedDatasetId::Fr02,
            "FR02",
            Frequency::Weekly,
        ),
        (
            "NyFedDatasetId::Fr03",
            NyFedDatasetId::Fr03,
            "FR03",
            Frequency::Weekly,
        ),
    ];
    for (id, dataset, literal, frequency) in ny_fed {
        hit("variant", id);
        hit("fn", "NyFedDatasetId::id");
        assert_eq!(dataset.id(), literal, "{id} 的登记形不符");
        hit("fn", "NyFedDatasetId::frequency");
        assert_eq!(dataset.frequency(), frequency, "{id} 的频率不符");
    }
    hit("fn", "NyFedDatasetId::parse_id");
    assert_eq!(
        NyFedDatasetId::parse_id("FR02").expect("合法 ID"),
        NyFedDatasetId::Fr02
    );
    assert_eq!(
        NyFedDatasetId::parse_id("FR99")
            .expect_err("未知 ID")
            .kind(),
        TreasuryErrorKind::Invalid
    );

    // —— FredValidationSeries：4 个变体 + 4 个访问器 + parse_id ——
    hit("type", "FredValidationSeries");
    let validation = [
        (
            "FredValidationSeries::Fd01",
            FredValidationSeries::Fd01,
            "FD01",
            "WALCL",
            "美联储总资产",
        ),
        (
            "FredValidationSeries::Fd02",
            FredValidationSeries::Fd02,
            "FD02",
            "WTREGEN",
            "TGA 周频交叉",
        ),
        (
            "FredValidationSeries::Fd03",
            FredValidationSeries::Fd03,
            "FD03",
            "RRPONTSYD",
            "ON RRP",
        ),
        (
            "FredValidationSeries::Fd04",
            FredValidationSeries::Fd04,
            "FD04",
            "WRESBAL",
            "银行准备金",
        ),
    ];
    for (id, series, literal, series_id, label) in validation {
        hit("variant", id);
        hit("fn", "FredValidationSeries::id");
        assert_eq!(series.id(), literal, "{id} 的登记形不符");
        hit("fn", "FredValidationSeries::series_id");
        assert_eq!(series.series_id(), series_id, "{id} 的 series 不符");
        hit("fn", "FredValidationSeries::label");
        assert_eq!(series.label(), label, "{id} 的中文含义不符");
        hit("fn", "FredValidationSeries::canonical_write_owner");
        assert_eq!(series.canonical_write_owner(), CANONICAL_WRITE_OWNER);
    }
    hit("fn", "FredValidationSeries::parse_id");
    assert_eq!(
        FredValidationSeries::parse_id("FD03").expect("合法 ID"),
        FredValidationSeries::Fd03
    );
    assert_eq!(
        FredValidationSeries::parse_id("FD05")
            .expect_err("未知 ID")
            .kind(),
        TreasuryErrorKind::Invalid
    );

    // —— is_curve_product ——
    hit("fn", "is_curve_product");
    assert!(is_curve_product(TreasuryDatasetId::Ds06));
    assert!(is_curve_product(TreasuryDatasetId::Ds07));
    assert!(!is_curve_product(TreasuryDatasetId::Ds05));

    // —— is_validation_only_series ——
    hit("fn", "is_validation_only_series");
    for series_id in ["WALCL", "WTREGEN", "RRPONTSYD", "WRESBAL"] {
        assert!(
            is_validation_only_series(series_id),
            "{series_id} 属永久校验面"
        );
    }
    assert!(!is_validation_only_series("DGS10"));
    assert!(!is_validation_only_series("WALCLX"), "不做前缀匹配");

    // —— guard_curve_product ——
    hit("fn", "guard_curve_product");
    assert_eq!(
        guard_curve_product(TreasuryDatasetId::Ds06)
            .expect_err("曲线必须路由")
            .kind(),
        TreasuryErrorKind::RoutedElsewhere
    );
    assert!(guard_curve_product(TreasuryDatasetId::Ds01).is_ok());

    // —— guard_collection_dataset：10 个数据集的合法/拒绝两侧 ——
    hit("fn", "guard_collection_dataset");
    for dataset in PHASE1_DATASETS {
        let result = guard_collection_dataset(dataset);
        match dataset {
            TreasuryDatasetId::Ds01 | TreasuryDatasetId::Ds05 => {
                assert!(result.is_ok(), "{} 应在采集面内", dataset.id());
            }
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

    // —— guard_write_authority：4 个校验序列拒绝，非校验序列放行 ——
    hit("fn", "guard_write_authority");
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

    // —— guard_ny_fed：整体 NO-GO ——
    hit("fn", "guard_ny_fed");
    for dataset in NY_FED_DATASETS {
        assert_eq!(
            guard_ny_fed(dataset).expect_err("NY Fed NO-GO").kind(),
            TreasuryErrorKind::NotApplicable,
            "{dataset:?}"
        );
    }

    // —— unit_for_validation_series：单位风险条 ——
    hit("fn", "unit_for_validation_series");
    assert_eq!(
        unit_for_validation_series(FredValidationSeries::Fd01),
        Unit::MillionsOfUsd
    );
    assert_eq!(
        unit_for_validation_series(FredValidationSeries::Fd02),
        Unit::MillionsOfUsd
    );
    assert_eq!(
        unit_for_validation_series(FredValidationSeries::Fd03),
        Unit::BillionsOfUsd
    );
    assert_eq!(
        unit_for_validation_series(FredValidationSeries::Fd04),
        Unit::Unknown,
        "WRESBAL 未登记单位，不得推断"
    );
    assert_ne!(
        unit_for_validation_series(FredValidationSeries::Fd03),
        unit_for_validation_series(FredValidationSeries::Fd01),
        "Billions 与 Millions 不得混同"
    );
}

/// 阶段 7：授权面（证据字段穷尽解构 + `authorize_treasury` 的授权与**全部**拒绝分支）。
fn phase_authz() {
    hit("type", "TreasuryScope");
    hit("variant", "TreasuryScope::OfflineFixtureOnly");
    hit("variant", "TreasuryScope::LiveCollection");
    assert_ne!(
        TreasuryScope::OfflineFixtureOnly,
        TreasuryScope::LiveCollection
    );

    hit("type", "TreasuryAuthorization");
    hit("variant", "TreasuryAuthorization::Authorized");
    let authorized = TreasuryAuthorization::Authorized {
        scope: "scope".to_owned(),
    };
    hit("variant", "TreasuryAuthorization::Denied");
    let denied = TreasuryAuthorization::Denied {
        reason: "reason".to_owned(),
    };
    assert_ne!(authorized, denied);
    // 结构体变体字段 `scope` / `reason`：读一次即可（不在 `field` 提取口径内）。
    match &authorized {
        TreasuryAuthorization::Authorized { scope } => assert_eq!(scope, "scope"),
        other => panic!("应为 Authorized，实得 {other:?}"),
    }
    match &denied {
        TreasuryAuthorization::Denied { reason } => assert_eq!(reason, "reason"),
        other => panic!("应为 Denied，实得 {other:?}"),
    }

    // —— TreasuryAuthorizationEvidence：类型 + new + 3 个公开字段穷尽解构 ——
    hit("type", "TreasuryAuthorizationEvidence");
    hit("fn", "TreasuryAuthorizationEvidence::new");
    let evidence = TreasuryAuthorizationEvidence::new(DECISION_ID, "ZoneCNH", "treasury_offline");
    let TreasuryAuthorizationEvidence {
        decision_id,
        signed_by,
        scope,
    } = evidence.clone();
    hit("field", "TreasuryAuthorizationEvidence::decision_id");
    assert_eq!(decision_id, DECISION_ID);
    hit("field", "TreasuryAuthorizationEvidence::signed_by");
    assert_eq!(signed_by, "ZoneCNH");
    hit("field", "TreasuryAuthorizationEvidence::scope");
    assert_eq!(scope, "treasury_offline");

    // —— authorize_treasury：授权路径 ——
    hit("fn", "authorize_treasury");
    match authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&evidence)) {
        TreasuryAuthorization::Authorized { scope } => {
            assert!(scope.contains(AUTHORIZED_SCOPE), "实际：{scope}");
            assert!(scope.contains("live 须另批"), "必须显式声明仅覆盖离线范围");
            assert!(!scope.contains("production"), "不得声称生产就绪：{scope}");
        }
        other => panic!("离线面应授权，实得 {other:?}"),
    }

    // —— 拒绝分支：缺证据 / 空 decision_id / 空 signed_by / 空 scope / LiveCollection ——
    let reason = expect_denied(authorize_treasury(TreasuryScope::OfflineFixtureOnly, None));
    assert!(reason.contains("缺少授权证据"), "实际：{reason}");

    for blank in ["", "   "] {
        let blank_id = TreasuryAuthorizationEvidence::new(blank, "ZoneCNH", "treasury_offline");
        let reason = expect_denied(authorize_treasury(
            TreasuryScope::OfflineFixtureOnly,
            Some(&blank_id),
        ));
        assert!(reason.contains("决策 ID"), "实际：{reason}");

        let blank_signer =
            TreasuryAuthorizationEvidence::new(DECISION_ID, blank, "treasury_offline");
        let reason = expect_denied(authorize_treasury(
            TreasuryScope::OfflineFixtureOnly,
            Some(&blank_signer),
        ));
        assert!(reason.contains("签署者"), "实际：{reason}");

        let blank_scope = TreasuryAuthorizationEvidence::new(DECISION_ID, "ZoneCNH", blank);
        let reason = expect_denied(authorize_treasury(
            TreasuryScope::OfflineFixtureOnly,
            Some(&blank_scope),
        ));
        assert!(reason.contains("覆盖范围"), "实际：{reason}");
    }

    // LiveCollection：即便证据有效也一律拒绝。
    let reason = expect_denied(authorize_treasury(
        TreasuryScope::LiveCollection,
        Some(&evidence),
    ));
    assert!(reason.contains("另批"), "live 必须另批，实际：{reason}");
    assert_eq!(u8::from(LIVE_REQUIRES_NEW_DECISION), 1);
}

/// 阶段 8：真实离线解析面（真实夹具文件 + 真实临时文件，成功与拒绝路径都落在磁盘上）。
fn phase_parse_and_files() {
    hit("fn", "parse_treasury_records");

    // —— 真实夹具文件读取：DS01 ——
    let ds01_text = read_fixture("ds01_envelope.json");
    let ds01 = parse_treasury_records(&ds01_text).expect("DS01 合成夹具必须可解析");
    hit("type", "TreasuryEnvelope");
    let TreasuryEnvelope { records, meta } = ds01;
    hit("field", "TreasuryEnvelope::records");
    assert_eq!(records.len(), 2);
    hit("field", "TreasuryEnvelope::meta");
    hit("type", "TreasuryEnvelopeMeta");
    let TreasuryEnvelopeMeta {
        count,
        total_count,
        total_pages,
    } = meta;
    hit("field", "TreasuryEnvelopeMeta::count");
    assert_eq!(count, 2);
    hit("field", "TreasuryEnvelopeMeta::total_count");
    assert_eq!(total_count, 16_538, "total-count 允许字符串形态");
    hit("field", "TreasuryEnvelopeMeta::total_pages");
    assert_eq!(total_pages, 166);

    let mut ds01_records = records.into_iter();
    let first = ds01_records.next().expect("必须有第 1 条记录");
    let TreasuryRecord {
        dataset_id,
        period,
        frequency,
        unit,
        revision,
        payload,
    } = first;
    hit("field", "TreasuryRecord::dataset_id");
    assert_eq!(dataset_id, TreasuryDatasetId::Ds01);
    hit("field", "TreasuryRecord::period");
    assert_eq!(period, Period::parse_day("2026-08-18").expect("合法日"));
    hit("field", "TreasuryRecord::frequency");
    assert_eq!(frequency, Frequency::Daily);
    hit("field", "TreasuryRecord::unit");
    assert_eq!(unit, Unit::Unknown);
    hit("field", "TreasuryRecord::revision");
    assert!(revision.is_none());
    hit("field", "TreasuryRecord::payload");
    match payload {
        TreasuryPayload::CashBalance(row) => {
            hit("variant", "TreasuryPayload::CashBalance");
            hit("type", "TreasuryCashBalance");
            let TreasuryCashBalance {
                account_type,
                open_today_bal,
                close_today_bal,
            } = row;
            hit("field", "TreasuryCashBalance::account_type");
            assert_eq!(account_type, "Federal Reserve Account");
            hit("field", "TreasuryCashBalance::open_today_bal");
            assert_eq!(open_today_bal.present(), Some(830_000.0));
            hit("field", "TreasuryCashBalance::close_today_bal");
            assert_eq!(close_today_bal.present(), Some(825_500.0));
        }
        other => panic!("DS01 载荷应为 CashBalance，实得 {other:?}"),
    }

    let second = ds01_records.next().expect("必须有第 2 条记录");
    match second.payload {
        TreasuryPayload::CashBalance(row) => {
            assert!(row.open_today_bal.is_missing(), "显式 null 必须是具名缺失");
            assert_eq!(row.open_today_bal.present(), None, "缺失不得折算为 0");
            assert_eq!(
                row.open_today_bal,
                TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull)
            );
            assert_eq!(row.close_today_bal.present(), Some(780_000.0));
        }
        other => panic!("DS01 载荷应为 CashBalance，实得 {other:?}"),
    }

    // —— 真实夹具文件读取：DS05 ——
    let ds05_text = read_fixture("ds05_envelope.json");
    let ds05 = parse_treasury_records(&ds05_text).expect("DS05 合成夹具必须可解析");
    assert_eq!(ds05.records.len(), 1);
    let TreasuryEnvelope { records, meta } = ds05;
    assert_eq!(meta.count, 1);
    match records.into_iter().next().expect("必须有记录").payload {
        TreasuryPayload::DebtToPenny(row) => {
            hit("variant", "TreasuryPayload::DebtToPenny");
            hit("type", "TreasuryDebtToPenny");
            let TreasuryDebtToPenny {
                debt_held_public_amt,
                intragov_hold_amt,
                tot_pub_debt_out_amt,
            } = row;
            hit("field", "TreasuryDebtToPenny::debt_held_public_amt");
            assert_eq!(debt_held_public_amt.present(), Some(29_000_000_000_000.0));
            hit("field", "TreasuryDebtToPenny::intragov_hold_amt");
            assert_eq!(intragov_hold_amt.present(), Some(7_500_000_000_000.0));
            hit("field", "TreasuryDebtToPenny::tot_pub_debt_out_amt");
            assert_eq!(tot_pub_debt_out_amt.present(), Some(36_500_000_000_000.0));
        }
        other => panic!("DS05 载荷应为 DebtToPenny，实得 {other:?}"),
    }

    // —— 真实夹具文件读取：曲线产品 fail-closed ——
    let curve_text = read_fixture("curve_envelope.json");
    assert_eq!(
        parse_treasury_records(&curve_text)
            .expect_err("曲线夹具必须被路由")
            .kind(),
        TreasuryErrorKind::RoutedElsewhere
    );

    // —— 真实临时文件：合法信封往返 ——
    let valid_path = unique_temp_file("treasuryx_e2e_valid");
    std::fs::write(&valid_path, &ds05_text).expect("写临时文件必须成功");
    let read_back = std::fs::read_to_string(&valid_path).expect("读回临时文件必须成功");
    assert_eq!(read_back, ds05_text, "读回内容必须与写入一致");
    let reparsed = parse_treasury_records(&read_back).expect("临时文件中的信封必须可解析");
    assert_eq!(reparsed.records.len(), 1);
    std::fs::remove_file(&valid_path).expect("删除临时文件必须成功");
    assert!(!valid_path.exists(), "删除后临时文件不得残留");

    // —— 真实临时文件：改写 meta.count 使不一致 → Invalid ——
    let mismatched = ds05_text.replace("\"count\": 1,", "\"count\": 2,");
    assert_ne!(mismatched, ds05_text, "变异必须真正命中 meta.count");
    let mismatch_path = unique_temp_file("treasuryx_e2e_mismatch");
    std::fs::write(&mismatch_path, &mismatched).expect("写临时文件必须成功");
    let mismatch_text = std::fs::read_to_string(&mismatch_path).expect("读回临时文件必须成功");
    assert_eq!(
        parse_treasury_records(&mismatch_text)
            .expect_err("meta.count 不一致必须拒绝")
            .kind(),
        TreasuryErrorKind::Invalid
    );
    std::fs::remove_file(&mismatch_path).expect("删除临时文件必须成功");
    assert!(!mismatch_path.exists(), "删除后临时文件不得残留");

    // —— 真实临时文件：注入未知字段 → SemanticallyRejected（原子失败）——
    let unknown = ds05_text.replace(
        "\"record_date\": \"2026-08-18\",",
        "\"record_date\": \"2026-08-18\", \"extra\": true,",
    );
    assert_ne!(unknown, ds05_text, "变异必须真正命中行首字段");
    let unknown_path = unique_temp_file("treasuryx_e2e_unknown");
    std::fs::write(&unknown_path, &unknown).expect("写临时文件必须成功");
    let unknown_text = std::fs::read_to_string(&unknown_path).expect("读回临时文件必须成功");
    let unknown_err = parse_treasury_records(&unknown_text).expect_err("未知字段必须原子失败");
    assert_eq!(unknown_err.kind(), TreasuryErrorKind::SemanticallyRejected);
    let unknown_message = unknown_err.to_string();
    assert!(
        unknown_message.contains("未知字段"),
        "拒绝理由应指明未知字段：{unknown_message}"
    );
    assert!(
        !unknown_message.contains("_dataset") && !unknown_message.contains("36500000000000"),
        "错误消息不得回显信封正文：{unknown_message}"
    );
    std::fs::remove_file(&unknown_path).expect("删除临时文件必须成功");
    assert!(!unknown_path.exists(), "删除后临时文件不得残留");
}

/// 单一驱动用例：treasuryx 无进程级共享状态，但保持单驱动更易归因。
#[test]
fn e2e_treasury_all_public_api() {
    assert_manifest_wellformed();
    phase_constants();
    phase_value_types();
    phase_validate_record();
    phase_error_types();
    phase_pit();
    phase_datasets();
    phase_authz();
    phase_parse_and_files();
    assert_coverage_complete();
}
