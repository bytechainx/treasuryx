//! treasuryx 的离线解析器：把合成样本 JSON 解析为 [`TreasuryEnvelope`]。
//!
//! - 参数**只有**字符串：不接受 URL、HTTP 客户端或任何认证信息
//! - 输入形态：Fiscal Data 信封形态 —— 顶层 `_dataset` + `data[]` + `meta{count, total-count, total-pages}`
//! - 未知字段 → **原子失败**（[`TreasuryError::SemanticallyRejected`]）
//! - 重复身份（`DS01` = 数据集 + 日期 + 账户类型；`DS05` = 数据集 + 日期）→ **拒绝**，不做静默去重
//! - `_dataset` 为**合成夹具的标注字段**（真实源信封不具备），用于在离线样本中声明行归属
//! - `meta.count` == `data` 行数是**合成样本层面的自洽不变量**，不是对真实 Fiscal Data
//!   信封语义的结论（清单把分页语义登记为部分收敛 / 仍 UNKNOWN）；保留该检查只为 fail-closed

use serde_json::{Map, Value};

use crate::error::{TreasuryError, TreasuryResult};
use crate::value::{
    guard_collection_dataset, validate_record, Period, TreasuryAmount, TreasuryCashBalance,
    TreasuryDatasetId, TreasuryDebtToPenny, TreasuryEnvelope, TreasuryEnvelopeMeta,
    TreasuryMissingReason, TreasuryPayload, TreasuryRecord, Unit,
};

/// 顶层允许的键。
const TOP_LEVEL_KEYS: [&str; 5] = ["_synthetic", "_note", "_dataset", "data", "meta"];

/// `meta` 允许的键（逐字取自 `specs/adapter/treasury.md` 的信息封套形态）。
const META_KEYS: [&str; 3] = ["count", "total-count", "total-pages"];

/// DS01 行允许的键（字段名逐字取自 `specs/adapter/treasury.md` §1.1）。
const DS01_KEYS: [&str; 4] = [
    "record_date",
    "account_type",
    "open_today_bal",
    "close_today_bal",
];

/// DS05 行允许的键（字段名逐字取自 `specs/adapter/treasury.md` §1.1）。
const DS05_KEYS: [&str; 4] = [
    "record_date",
    "debt_held_public_amt",
    "intragov_hold_amt",
    "tot_pub_debt_out_amt",
];

/// 解析合成样本 JSON 为 Fiscal Data 信封。
///
/// # Examples
///
/// ```
/// use treasuryx::parse_treasury_records;
///
/// let sample = r#"{"_dataset":"DS05",
///     "data":[{"record_date":"2026-08-18","tot_pub_debt_out_amt":36500000000000.0}],
///     "meta":{"count":1,"total-count":1,"total-pages":1}}"#;
/// let envelope = parse_treasury_records(sample)?;
/// assert_eq!(envelope.records.len(), 1);
/// # Ok::<(), treasuryx::TreasuryError>(())
/// ```
///
/// # Errors
///
/// - 非合法 JSON / 顶层或条目不是对象 / 字段类型不符 → [`TreasuryError::Invalid`]
/// - 缺少 `_dataset` / `data` / `meta` 或必需字段 / 无任何可用金额 → [`TreasuryError::Missing`]
/// - 出现未知字段 → [`TreasuryError::SemanticallyRejected`]
/// - 曲线产品 `DS06` / `DS07` → [`TreasuryError::RoutedElsewhere`]
/// - 采集面外的数据集 → [`TreasuryError::NotApplicable`]
/// - `DS04`（lossy skeleton map）→ [`TreasuryError::SemanticallyRejected`]
pub fn parse_treasury_records(input: &str) -> TreasuryResult<TreasuryEnvelope> {
    let root: Value = serde_json::from_str(input)
        .map_err(|error| TreasuryError::Invalid(format!("样本不是合法 JSON：{error}")))?;
    let Some(object) = root.as_object() else {
        return Err(TreasuryError::Invalid("样本顶层必须是 JSON 对象".into()));
    };
    check_keys(object, &TOP_LEVEL_KEYS, "顶层")?;

    let dataset_id = TreasuryDatasetId::parse_id(&required_str(object, "顶层", "_dataset")?)?;
    guard_collection_dataset(dataset_id)?;

    let meta = parse_meta(object.get("meta"))?;

    let Some(items) = object.get("data") else {
        return Err(TreasuryError::Missing("缺少顶层必需项 data".into()));
    };
    let Some(items) = items.as_array() else {
        return Err(TreasuryError::Invalid("data 必须是数组".into()));
    };

    let allowed: &[&str] = match dataset_id {
        TreasuryDatasetId::Ds01 => &DS01_KEYS,
        _ => &DS05_KEYS,
    };

    let mut records: Vec<TreasuryRecord> = Vec::with_capacity(items.len());
    let mut seen: Vec<String> = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let scope = format!("data[{index}]");
        let Some(entry) = item.as_object() else {
            return Err(TreasuryError::Invalid(format!("{scope} 必须是 JSON 对象")));
        };
        check_keys(entry, allowed, &scope)?;

        let record_date = required_str(entry, &scope, "record_date")?;
        let period = Period::parse_day(&record_date)?;

        let (identity, payload) = match dataset_id {
            TreasuryDatasetId::Ds01 => {
                let account_type = required_str(entry, &scope, "account_type")?;
                (
                    format!("{}|{record_date}|{account_type}", dataset_id.id()),
                    TreasuryPayload::CashBalance(TreasuryCashBalance {
                        account_type,
                        open_today_bal: amount(entry, &scope, "open_today_bal")?,
                        close_today_bal: amount(entry, &scope, "close_today_bal")?,
                    }),
                )
            }
            _ => (
                format!("{}|{record_date}", dataset_id.id()),
                TreasuryPayload::DebtToPenny(TreasuryDebtToPenny {
                    debt_held_public_amt: amount(entry, &scope, "debt_held_public_amt")?,
                    intragov_hold_amt: amount(entry, &scope, "intragov_hold_amt")?,
                    tot_pub_debt_out_amt: amount(entry, &scope, "tot_pub_debt_out_amt")?,
                }),
            ),
        };

        if seen.contains(&identity) {
            return Err(TreasuryError::Invalid(format!("重复身份：{identity}")));
        }
        seen.push(identity);

        let record = TreasuryRecord {
            dataset_id,
            period,
            frequency: dataset_id.frequency(),
            unit: Unit::Unknown,
            revision: None,
            payload,
        };
        validate_record(&record)?;
        records.push(record);
    }

    // 合成样本层面的自洽不变量（不是对真实 Fiscal Data 信封语义的结论）；
    // 保留该检查的理由是 fail-closed：不满足即拒绝，失败面安全。
    if meta.count != records.len() as u64 {
        return Err(TreasuryError::Invalid(format!(
            "meta.count 与 data 行数不一致：{} vs {}",
            meta.count,
            records.len()
        )));
    }

    Ok(TreasuryEnvelope { records, meta })
}

/// 解析 `meta` 块。
fn parse_meta(raw: Option<&Value>) -> TreasuryResult<TreasuryEnvelopeMeta> {
    let Some(raw) = raw else {
        return Err(TreasuryError::Missing("缺少顶层必需项 meta".into()));
    };
    let Some(object) = raw.as_object() else {
        return Err(TreasuryError::Invalid("meta 必须是 JSON 对象".into()));
    };
    check_keys(object, &META_KEYS, "meta")?;
    Ok(TreasuryEnvelopeMeta {
        count: required_u64(object, "meta", "count")?,
        total_count: required_u64(object, "meta", "total-count")?,
        total_pages: required_u64(object, "meta", "total-pages")?,
    })
}

/// 字段白名单校验；命中白名单外的键即原子失败。
fn check_keys(object: &Map<String, Value>, allowed: &[&str], scope: &str) -> TreasuryResult<()> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(TreasuryError::SemanticallyRejected(format!(
                "{scope} 未知字段：{key}"
            )));
        }
    }
    Ok(())
}

/// 取必填字符串字段。
fn required_str(object: &Map<String, Value>, scope: &str, key: &str) -> TreasuryResult<String> {
    let Some(raw) = object.get(key) else {
        return Err(TreasuryError::Missing(format!("{scope} 缺少字段 {key}")));
    };
    let Some(text) = raw.as_str() else {
        return Err(TreasuryError::Invalid(format!(
            "{scope} 的 {key} 必须是字符串"
        )));
    };
    if text.trim().is_empty() {
        return Err(TreasuryError::Invalid(format!(
            "{scope} 的 {key} 不得为空白"
        )));
    }
    Ok(text.to_string())
}

/// 取必填非负整数字段（接受 JSON 数字或纯数字字符串）。
fn required_u64(object: &Map<String, Value>, scope: &str, key: &str) -> TreasuryResult<u64> {
    let Some(raw) = object.get(key) else {
        return Err(TreasuryError::Missing(format!("{scope} 缺少字段 {key}")));
    };
    if let Some(number) = raw.as_u64() {
        return Ok(number);
    }
    if let Some(text) = raw.as_str() {
        if let Ok(number) = text.parse::<u64>() {
            return Ok(number);
        }
    }
    Err(TreasuryError::Invalid(format!(
        "{scope} 的 {key} 必须是非负整数"
    )))
}

/// 取金额槽：缺省 → `SourceOmitted`；`null` → `ExplicitNull`；数字或数值字符串 → 有值。
fn amount(object: &Map<String, Value>, scope: &str, key: &str) -> TreasuryResult<TreasuryAmount> {
    let Some(raw) = object.get(key) else {
        return Ok(TreasuryAmount::Missing(
            TreasuryMissingReason::SourceOmitted,
        ));
    };
    if raw.is_null() {
        return Ok(TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull));
    }
    let value = if let Some(number) = raw.as_f64() {
        number
    } else if let Some(text) = raw.as_str() {
        text.parse::<f64>()
            .map_err(|_| TreasuryError::Invalid(format!("{scope} 的 {key} 必须可解析为数值")))?
    } else {
        return Err(TreasuryError::Invalid(format!(
            "{scope} 的 {key} 必须是数值、数值字符串或 null"
        )));
    };
    if !value.is_finite() {
        return Err(TreasuryError::Invalid(format!(
            "{scope} 的 {key} 必须是有限数值"
        )));
    }
    Ok(TreasuryAmount::Present(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DS01_SAMPLE: &str = r#"{
        "_synthetic": true,
        "_note": "合成样本",
        "_dataset": "DS01",
        "data": [
            {"record_date":"2026-08-18","account_type":"Federal Reserve Account",
             "open_today_bal":"830000","close_today_bal":825500},
            {"record_date":"2026-08-18","account_type":"Operating Cash Balance",
             "open_today_bal":null,"close_today_bal":"780000"}
        ],
        "meta": {"count": 2, "total-count": "16538", "total-pages": 166}
    }"#;

    const DS05_SAMPLE: &str = r#"{
        "_dataset": "DS05",
        "data": [
            {"record_date":"2026-08-18","debt_held_public_amt":29000000000000.0,
             "intragov_hold_amt":7500000000000.0,"tot_pub_debt_out_amt":36500000000000.0}
        ],
        "meta": {"count": 1, "total-count": 1, "total-pages": 1}
    }"#;

    #[test]
    fn parses_both_collection_face_datasets() {
        let ds01 = parse_treasury_records(DS01_SAMPLE).expect("DS01 合成样本可解析");
        assert_eq!(ds01.records.len(), 2);
        assert_eq!(ds01.meta.total_pages, 166);
        assert_eq!(ds01.records[0].dataset_id, TreasuryDatasetId::Ds01);
        assert_eq!(ds01.records[1].unit, Unit::Unknown);

        let ds05 = parse_treasury_records(DS05_SAMPLE).expect("DS05 合成样本可解析");
        assert_eq!(ds05.records.len(), 1);
        assert_eq!(ds05.records[0].frequency, crate::value::Frequency::Daily);
        assert_eq!(ds05.records[0].revision, None);
    }

    #[test]
    fn rejects_unknown_field_atomically() {
        let unknown_row = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18",
            "tot_pub_debt_out_amt":1.0,"extra":true}],"meta":{"count":1,"total-count":1,"total-pages":1}}"#;
        assert_eq!(
            parse_treasury_records(unknown_row)
                .expect_err("未知行字段")
                .kind(),
            crate::error::TreasuryErrorKind::SemanticallyRejected
        );

        let unknown_top = r#"{"_dataset":"DS05","data":[],"meta":{"count":0,"total-count":0,
            "total-pages":0},"extra":1}"#;
        assert_eq!(
            parse_treasury_records(unknown_top)
                .expect_err("未知顶层字段")
                .kind(),
            crate::error::TreasuryErrorKind::SemanticallyRejected
        );
    }

    #[test]
    fn rejects_missing_required_fields() {
        let no_dataset = r#"{"data":[],"meta":{"count":0,"total-count":0,"total-pages":0}}"#;
        assert_eq!(
            parse_treasury_records(no_dataset)
                .expect_err("缺 _dataset")
                .kind(),
            crate::error::TreasuryErrorKind::Missing
        );
        let no_meta = r#"{"_dataset":"DS05","data":[]}"#;
        assert_eq!(
            parse_treasury_records(no_meta).expect_err("缺 meta").kind(),
            crate::error::TreasuryErrorKind::Missing
        );
        let no_date = r#"{"_dataset":"DS05","data":[{"tot_pub_debt_out_amt":1.0}],
            "meta":{"count":1,"total-count":1,"total-pages":1}}"#;
        assert_eq!(
            parse_treasury_records(no_date)
                .expect_err("缺 record_date")
                .kind(),
            crate::error::TreasuryErrorKind::Missing
        );
    }

    #[test]
    fn rejects_illegal_date_and_duplicate_identity() {
        let bad_date = r#"{"_dataset":"DS05","data":[{"record_date":"2026-8-18",
            "tot_pub_debt_out_amt":1.0}],"meta":{"count":1,"total-count":1,"total-pages":1}}"#;
        assert!(parse_treasury_records(bad_date).is_err());

        let duplicate = r#"{"_dataset":"DS05","data":[
            {"record_date":"2026-08-18","tot_pub_debt_out_amt":1.0},
            {"record_date":"2026-08-18","tot_pub_debt_out_amt":2.0}],
            "meta":{"count":2,"total-count":2,"total-pages":1}}"#;
        assert_eq!(
            parse_treasury_records(duplicate)
                .expect_err("重复身份")
                .kind(),
            crate::error::TreasuryErrorKind::Invalid
        );
    }

    #[test]
    fn rejects_curve_and_non_collection_datasets() {
        let curve =
            r#"{"_dataset":"DS06","data":[],"meta":{"count":0,"total-count":0,"total-pages":0}}"#;
        assert_eq!(
            parse_treasury_records(curve).expect_err("曲线").kind(),
            crate::error::TreasuryErrorKind::RoutedElsewhere
        );

        let lossy =
            r#"{"_dataset":"DS04","data":[],"meta":{"count":0,"total-count":0,"total-pages":0}}"#;
        assert_eq!(
            parse_treasury_records(lossy)
                .expect_err("DS04 lossy")
                .kind(),
            crate::error::TreasuryErrorKind::SemanticallyRejected
        );

        let outside =
            r#"{"_dataset":"DS08","data":[],"meta":{"count":0,"total-count":0,"total-pages":0}}"#;
        assert_eq!(
            parse_treasury_records(outside)
                .expect_err("采集面外")
                .kind(),
            crate::error::TreasuryErrorKind::NotApplicable
        );

        let unknown =
            r#"{"_dataset":"DS99","data":[],"meta":{"count":0,"total-count":0,"total-pages":0}}"#;
        assert_eq!(
            parse_treasury_records(unknown).expect_err("未知 ID").kind(),
            crate::error::TreasuryErrorKind::Invalid
        );
    }

    #[test]
    fn rejects_meta_count_mismatch_and_bad_shapes() {
        let mismatch = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18",
            "tot_pub_debt_out_amt":1.0}],"meta":{"count":2,"total-count":2,"total-pages":1}}"#;
        assert!(parse_treasury_records(mismatch).is_err());

        assert!(parse_treasury_records("[]").is_err());
        assert!(parse_treasury_records("not json").is_err());

        let bad_amount = r#"{"_dataset":"DS05","data":[{"record_date":"2026-08-18",
            "tot_pub_debt_out_amt":"not-a-number"}],"meta":{"count":1,"total-count":1,"total-pages":1}}"#;
        assert!(parse_treasury_records(bad_amount).is_err());
    }
}
