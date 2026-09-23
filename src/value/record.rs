//! treasuryx 的记录值对象与完整性校验入口。

use crate::error::{TreasuryError, TreasuryResult};
use crate::value::datasets::{guard_collection_dataset, TreasuryDatasetId};
use crate::value::{Frequency, Period, Unit};

/// 具名缺失原因。
///
/// 缺失 MUST 具名表达，MUST NOT 静默转 0（`source-library-contract.md` §2.1）。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreasuryMissingReason {
    /// 该字段在源记录中缺省。
    SourceOmitted,
    /// 该字段在源记录中显式为 `null`。
    ExplicitNull,
}

/// 金额槽。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TreasuryAmount {
    /// 有值。
    Present(f64),
    /// 缺失及其具名原因。
    Missing(TreasuryMissingReason),
}

impl TreasuryAmount {
    /// 取值；缺失返回 `None`（MUST NOT 转 0）。
    #[must_use]
    pub fn present(&self) -> Option<f64> {
        match self {
            Self::Present(value) => Some(*value),
            Self::Missing(_) => None,
        }
    }

    /// 是否为缺失。
    #[must_use]
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }
}

/// DS01 行：DTS 现金余额（TGA）。字段名逐字取自 `specs/adapter/treasury.md` §1.1。
#[derive(Debug, Clone, PartialEq)]
pub struct TreasuryCashBalance {
    /// 源字段 `account_type`。
    pub account_type: String,
    /// 源字段 `open_today_bal`。
    pub open_today_bal: TreasuryAmount,
    /// 源字段 `close_today_bal`。
    pub close_today_bal: TreasuryAmount,
}

/// DS05 行：Debt to the Penny。字段名逐字取自 `specs/adapter/treasury.md` §1.1。
#[derive(Debug, Clone, PartialEq)]
pub struct TreasuryDebtToPenny {
    /// 源字段 `debt_held_public_amt`。
    pub debt_held_public_amt: TreasuryAmount,
    /// 源字段 `intragov_hold_amt`。
    pub intragov_hold_amt: TreasuryAmount,
    /// 源字段 `tot_pub_debt_out_amt`。
    pub tot_pub_debt_out_amt: TreasuryAmount,
}

/// 数据集专用载荷：只覆盖本域采集面（`DS01` / `DS05`）。
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum TreasuryPayload {
    /// DS01 DTS 现金余额。
    CashBalance(TreasuryCashBalance),
    /// DS05 Debt to the Penny。
    DebtToPenny(TreasuryDebtToPenny),
}

/// 一条源记录（源事实，不含任何派生指标）。
#[derive(Debug, Clone, PartialEq)]
pub struct TreasuryRecord {
    /// 数据集 ID。
    pub dataset_id: TreasuryDatasetId,
    /// 业务期间（源字段 `record_date`，映射为单日期间）。
    pub period: Period,
    /// 源侧频率。
    pub frequency: Frequency,
    /// 源侧单位（`DS01` / `DS05` 未在清单声明，恒为 [`Unit::Unknown`]）。
    pub unit: Unit,
    /// 修订标识：清单未声明官方 revision 面，恒为 `None`，MUST NOT 伪造。
    pub revision: Option<String>,
    /// 数据集专用载荷。
    pub payload: TreasuryPayload,
}

/// Fiscal Data 信封的 `meta` 块。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreasuryEnvelopeMeta {
    /// `meta.count`。
    pub count: u64,
    /// `meta.total-count`。
    pub total_count: u64,
    /// `meta.total-pages`。
    pub total_pages: u64,
}

/// 离线解析结果：`data[]` + `meta{count, total-count, total-pages}`。
#[derive(Debug, Clone, PartialEq)]
pub struct TreasuryEnvelope {
    /// `data[]` 的解析结果。
    pub records: Vec<TreasuryRecord>,
    /// 信封元数据。
    pub meta: TreasuryEnvelopeMeta,
}

/// 校验一条记录的完整性。
///
/// 检查项：采集面守卫、单位与数据集一致、期间为单日、频率与数据集一致、
/// 修订标识未伪造、载荷与数据集匹配、金额有限、至少一个金额可用。
///
/// # Errors
///
/// - 曲线产品 → [`TreasuryError::RoutedElsewhere`]；`DS04` → [`TreasuryError::SemanticallyRejected`]；
///   其余采集面外 → [`TreasuryError::NotApplicable`]
/// - 载荷与数据集不匹配 / 单位或频率不符 / 修订标识被伪造 / 金额非有限 →
///   [`TreasuryError::Invalid`]
/// - 账户类型空白 / 无任何可用金额 → [`TreasuryError::Missing`]
pub fn validate_record(record: &TreasuryRecord) -> TreasuryResult<()> {
    guard_collection_dataset(record.dataset_id)?;

    if record.unit != Unit::Unknown {
        return Err(TreasuryError::Invalid(format!(
            "DS01 / DS05 的源侧单位未在清单声明，记录单位必须为 Unknown：{}",
            record.dataset_id.id()
        )));
    }
    if !matches!(record.period, Period::Day(_)) {
        return Err(TreasuryError::Invalid(format!(
            "本域记录的业务期间必须是单日：{}",
            record.dataset_id.id()
        )));
    }
    if let Period::Day(date) = record.period {
        crate::value::Date::new(date.year, date.month, date.day)?;
    }

    if record.frequency != record.dataset_id.frequency() {
        return Err(TreasuryError::Invalid(format!(
            "频率与数据集不符：{}",
            record.dataset_id.id()
        )));
    }
    if record.revision.is_some() {
        return Err(TreasuryError::Invalid(format!(
            "清单未声明官方 revision 面，修订标识不得伪造：{}",
            record.dataset_id.id()
        )));
    }

    match (&record.payload, record.dataset_id) {
        (TreasuryPayload::CashBalance(row), TreasuryDatasetId::Ds01) => {
            if row.account_type.trim().is_empty() {
                return Err(TreasuryError::Missing(
                    "DS01 的 account_type 不得为空白".into(),
                ));
            }
            check_amounts("DS01", &[row.open_today_bal, row.close_today_bal])
        }
        (TreasuryPayload::DebtToPenny(row), TreasuryDatasetId::Ds05) => check_amounts(
            "DS05",
            &[
                row.debt_held_public_amt,
                row.intragov_hold_amt,
                row.tot_pub_debt_out_amt,
            ],
        ),
        _ => Err(TreasuryError::Invalid(format!(
            "载荷与数据集不匹配：{}",
            record.dataset_id.id()
        ))),
    }
}

/// 金额集合校验：全部有限，且至少一个可用。
fn check_amounts(dataset: &str, amounts: &[TreasuryAmount]) -> TreasuryResult<()> {
    let mut available = 0usize;
    for amount in amounts {
        match amount.present() {
            Some(value) => {
                if !value.is_finite() {
                    return Err(TreasuryError::Invalid(format!(
                        "{dataset} 的金额必须是有限值"
                    )));
                }
                available += 1;
            }
            None => continue,
        }
    }
    if available == 0 {
        return Err(TreasuryError::Missing(format!(
            "{dataset} 的记录没有任何可用金额"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Date;

    fn ds01_record() -> TreasuryRecord {
        TreasuryRecord {
            dataset_id: TreasuryDatasetId::Ds01,
            period: Period::Day(Date {
                year: 2026,
                month: 8,
                day: 18,
            }),
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
            payload: TreasuryPayload::DebtToPenny(TreasuryDebtToPenny {
                debt_held_public_amt: TreasuryAmount::Present(29_000_000_000_000.0),
                intragov_hold_amt: TreasuryAmount::Present(7_500_000_000_000.0),
                tot_pub_debt_out_amt: TreasuryAmount::Present(36_500_000_000_000.0),
            }),
        }
    }

    #[test]
    fn validate_accepts_both_collection_face_datasets() {
        assert!(validate_record(&ds01_record()).is_ok());
        assert!(validate_record(&ds05_record()).is_ok());
    }

    #[test]
    fn validate_rejects_payload_dataset_mismatch() {
        let mut record = ds01_record();
        record.payload = ds05_record().payload;
        assert_eq!(
            validate_record(&record).expect_err("载荷不匹配").kind(),
            crate::error::TreasuryErrorKind::Invalid
        );
    }

    #[test]
    fn validate_rejects_curve_and_non_face_datasets() {
        let mut curve = ds01_record();
        curve.dataset_id = TreasuryDatasetId::Ds06;
        assert_eq!(
            validate_record(&curve).expect_err("曲线").kind(),
            crate::error::TreasuryErrorKind::RoutedElsewhere
        );

        let mut lossy = ds01_record();
        lossy.dataset_id = TreasuryDatasetId::Ds04;
        assert_eq!(
            validate_record(&lossy).expect_err("lossy").kind(),
            crate::error::TreasuryErrorKind::SemanticallyRejected
        );

        let mut outside = ds01_record();
        outside.dataset_id = TreasuryDatasetId::Ds08;
        outside.frequency = Frequency::Monthly;
        assert_eq!(
            validate_record(&outside).expect_err("采集面外").kind(),
            crate::error::TreasuryErrorKind::NotApplicable
        );
    }

    #[test]
    fn validate_rejects_non_finite_fake_revision_and_blank_account() {
        let mut nan = ds05_record();
        nan.payload = TreasuryPayload::DebtToPenny(TreasuryDebtToPenny {
            debt_held_public_amt: TreasuryAmount::Present(f64::NAN),
            intragov_hold_amt: TreasuryAmount::Missing(TreasuryMissingReason::SourceOmitted),
            tot_pub_debt_out_amt: TreasuryAmount::Missing(TreasuryMissingReason::SourceOmitted),
        });
        assert!(validate_record(&nan).is_err());

        let mut fake_revision = ds01_record();
        fake_revision.revision = Some("1".into());
        assert!(validate_record(&fake_revision).is_err());

        let mut blank = ds01_record();
        blank.payload = TreasuryPayload::CashBalance(TreasuryCashBalance {
            account_type: "  ".into(),
            open_today_bal: TreasuryAmount::Present(1.0),
            close_today_bal: TreasuryAmount::Present(1.0),
        });
        assert_eq!(
            validate_record(&blank).expect_err("账户类型空白").kind(),
            crate::error::TreasuryErrorKind::Missing
        );
    }

    #[test]
    fn all_missing_amounts_is_a_named_missing_not_zero() {
        let mut record = ds05_record();
        record.payload = TreasuryPayload::DebtToPenny(TreasuryDebtToPenny {
            debt_held_public_amt: TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull),
            intragov_hold_amt: TreasuryAmount::Missing(TreasuryMissingReason::SourceOmitted),
            tot_pub_debt_out_amt: TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull),
        });
        assert_eq!(
            validate_record(&record).expect_err("无可用金额").kind(),
            crate::error::TreasuryErrorKind::Missing
        );
        assert_eq!(
            TreasuryAmount::Missing(TreasuryMissingReason::ExplicitNull).present(),
            None
        );
        assert!(TreasuryAmount::Missing(TreasuryMissingReason::SourceOmitted).is_missing());
    }

    #[test]
    fn validation_rechecks_public_period_dates() {
        for date in [
            Date {
                year: 2026,
                month: 2,
                day: 30,
            },
            Date {
                year: 2026,
                month: 0,
                day: 1,
            },
            Date {
                year: 2026,
                month: 12,
                day: 0,
            },
        ] {
            let mut value = ds01_record();
            value.period = Period::Day(date);
            assert!(validate_record(&value).is_err());
        }
        for period in [
            Period::Month {
                year: 2026,
                month: 2,
            },
            Period::Quarter {
                year: 2026,
                quarter: 1,
            },
            Period::Year(2026),
            Period::Event {
                date: Date::new(2026, 2, 1).unwrap(),
            },
        ] {
            let mut value = ds01_record();
            value.period = period;
            assert!(validate_record(&value).is_err());
        }
        let mut value = ds01_record();
        value.period = Period::Day(Date::new(2024, 2, 29).unwrap());
        assert!(validate_record(&value).is_ok());
    }
}
