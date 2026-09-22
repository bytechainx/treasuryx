//! treasuryx 的基础值对象：日期、期间、频率与源侧单位。
//!
//! 本模块只表达「身份」层面的源事实：不做时区、不做算术、不做单位换算。
//! 数据集标识与守卫见 [`crate::value::datasets`]，记录本体与校验入口见
//! [`crate::value::record`]。

mod datasets;
mod record;

pub use datasets::{
    guard_collection_dataset, guard_curve_product, guard_ny_fed, guard_write_authority,
    is_curve_product, is_validation_only_series, unit_for_validation_series, FredValidationSeries,
    NyFedDatasetId, TreasuryDatasetId, CANONICAL_WRITE_OWNER, DRY_RUN_DECLARED_DATASETS,
    FD_VALIDATION_SERIES, LOSSY_SKELETON_DATASETS, NY_FED_DATASETS, NY_FED_DECISION,
    PHASE1_DATASETS, WIRE_COLLECT_DATASETS,
};
pub use record::{
    validate_record, TreasuryAmount, TreasuryCashBalance, TreasuryDebtToPenny, TreasuryEnvelope,
    TreasuryEnvelopeMeta, TreasuryMissingReason, TreasuryPayload, TreasuryRecord,
};

use crate::error::{TreasuryError, TreasuryResult};

/// 业务日期：只表达「年 / 月 / 日」的身份，不带时区、不做算术、不做格式化。
///
/// 本层只需要期间的**身份**（见 `source-library-contract.md` §2.2），
/// 因此不引入 `chrono` / `time`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    /// 年。
    pub year: i16,
    /// 月（1–12）。
    pub month: u8,
    /// 日（按月份与闰年校验）。
    pub day: u8,
}

impl Date {
    /// 构造并校验一个日期：月必须 1–12，日必须落在该月与闰年规则允许的范围内。
    ///
    /// # Errors
    ///
    /// 月或日超出取值域时返回 [`TreasuryError::Invalid`]。
    pub fn new(year: i16, month: u8, day: u8) -> TreasuryResult<Self> {
        if !(1..=12).contains(&month) {
            return Err(TreasuryError::Invalid(format!("月份超出 1–12：{month}")));
        }
        let limit = days_in_month(year, month);
        if day == 0 || day > limit {
            return Err(TreasuryError::Invalid(format!(
                "日超出该月范围：{year}-{month:02} 的上限为 {limit}，实际 {day}"
            )));
        }
        Ok(Self { year, month, day })
    }

    /// 严格解析 `YYYY-MM-DD`（月与日必须两位补零）。
    ///
    /// 拒绝 `2026-2-3`、`2026/02/03`、`2026-02-03T00:00:00` 与任何非数字字符。
    ///
    /// # Errors
    ///
    /// 形态不合法或日期取值超出取值域时返回 [`TreasuryError::Invalid`]。
    pub fn parse(input: &str) -> TreasuryResult<Self> {
        let bytes = input.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(TreasuryError::Invalid(format!(
                "日期必须是严格的 YYYY-MM-DD（两位补零）：{input}"
            )));
        }
        let year = literal_digits(&bytes[0..4])
            .ok_or_else(|| TreasuryError::Invalid(format!("年份部分非十进制数字：{input}")))?;
        let month = literal_digits(&bytes[5..7])
            .ok_or_else(|| TreasuryError::Invalid(format!("月份部分非十进制数字：{input}")))?;
        let day = literal_digits(&bytes[8..10])
            .ok_or_else(|| TreasuryError::Invalid(format!("日部分非十进制数字：{input}")))?;
        let year = i16::try_from(year)
            .map_err(|_| TreasuryError::Invalid(format!("年份超出 i16 取值域：{input}")))?;
        let month = u8::try_from(month)
            .map_err(|_| TreasuryError::Invalid(format!("月份超出 u8 取值域：{input}")))?;
        let day = u8::try_from(day)
            .map_err(|_| TreasuryError::Invalid(format!("日超出 u8 取值域：{input}")))?;
        Self::new(year, month, day)
    }
}

/// 解析定长十进制字段；任何非数字字符都判失败。
fn literal_digits(slice: &[u8]) -> Option<u32> {
    let mut value: u32 = 0;
    for byte in slice {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u32::from(byte - b'0');
    }
    Some(value)
}

/// 闰年判定（公历规则）。
fn is_leap_year(year: i16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// 某年某月的天数；月份越界时返回 0。
fn days_in_month(year: i16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// 业务期间：本层只需要「日 / 月 / 季 / 年 / 事件」的身份。
///
/// `Period` MUST NOT 被 `String` 顶替（`source-library-contract.md` §2.2）。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Period {
    /// 单日。
    Day(Date),
    /// 单月。
    Month {
        /// 年。
        year: i16,
        /// 月（1–12）。
        month: u8,
    },
    /// 单季。
    Quarter {
        /// 年。
        year: i16,
        /// 季（1–4）。
        quarter: u8,
    },
    /// 单年。
    Year(i16),
    /// 事件时点（源只给日期）。
    Event {
        /// 事件日期。
        date: Date,
    },
}

impl Period {
    /// 按「单日」解析 `YYYY-MM-DD`。
    ///
    /// # Errors
    ///
    /// 形态不合法或日期取值超出取值域时返回 [`TreasuryError::Invalid`]。
    pub fn parse_day(input: &str) -> TreasuryResult<Self> {
        Ok(Self::Day(Date::parse(input)?))
    }
}

/// 源侧频率。取值域与 `source-library-contract.md` §2.1 一致。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Frequency {
    /// 日频。
    Daily,
    /// 周频。
    Weekly,
    /// 月频。
    Monthly,
    /// 季频。
    Quarterly,
    /// 年频。
    Annual,
    /// 事件驱动。
    Event,
    /// 不规则。
    Irregular,
}

/// 源侧单位。
///
/// 清单只登记了两处单位风险（`cross-source-routing.md` §4）：
/// `RRPONTSYD` 为 **Billions**，而 `WALCL` / `WTREGEN` 为 **Millions**。
/// DS01 / DS05 的源侧单位在清单中**未声明**，故记 [`Unit::Unknown`]——
/// 本层不得由字段名推断单位，也不得做任何换算。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    /// 源侧单位未在清单中声明（DS01 / DS05 属此）。
    Unknown,
    /// 十亿美元（`RRPONTSYD` 源单位）。
    BillionsOfUsd,
    /// 百万美元（`WALCL` / `WTREGEN` 源单位）。
    MillionsOfUsd,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_accepts_strict_iso_only() {
        assert_eq!(
            Date::parse("2026-08-18").expect("合法日期"),
            Date {
                year: 2026,
                month: 8,
                day: 18
            }
        );
        for bad in [
            "2026-8-18",
            "2026/08/18",
            "2026-08-18T00:00:00",
            "2026-08-8",
            "2026-008-18",
            "",
            "2026-08-18 ",
            "2026-08-1a",
        ] {
            assert!(Date::parse(bad).is_err(), "{bad} 必须被拒绝");
        }
    }

    #[test]
    fn date_validates_month_day_and_leap_year() {
        assert!(Date::new(2026, 13, 1).is_err());
        assert!(Date::new(2026, 0, 1).is_err());
        assert!(Date::new(2026, 2, 29).is_err(), "2026 不是闰年");
        assert!(Date::new(2000, 2, 29).is_ok(), "2000 是闰年");
        assert!(Date::new(1900, 2, 29).is_err(), "1900 不是闰年");
        assert!(Date::new(2024, 2, 29).is_ok());
        assert!(Date::new(2026, 4, 31).is_err());
        assert!(Date::new(2026, 1, 0).is_err());
    }

    #[test]
    fn period_parse_day_is_a_day_period() {
        assert!(matches!(
            Period::parse_day("2026-08-18").expect("合法日"),
            Period::Day(_)
        ));
        assert!(Period::parse_day("2026-8-18").is_err());
    }

    #[test]
    fn frequency_and_unit_carry_the_manifest_values() {
        let values = [
            Frequency::Daily,
            Frequency::Weekly,
            Frequency::Monthly,
            Frequency::Quarterly,
            Frequency::Annual,
            Frequency::Event,
            Frequency::Irregular,
        ];
        assert_eq!(values.len(), 7, "频率取值域为 7 个");
        assert_ne!(Unit::BillionsOfUsd, Unit::MillionsOfUsd, "单位不得混同");
    }
}
