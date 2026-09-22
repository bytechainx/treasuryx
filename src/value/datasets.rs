//! treasuryx 的数据集事实与守卫：Phase 1 数据集、NY Fed 数据集、FRED 校验序列，
//! 以及曲线路由、采集面与写入主权三类守卫。
//!
//! 常量取自 `specs/adapter/treasury.md` 与 `contracts/cross-source-routing.md` §2–§3，
//! 只作**声明层**与守卫使用：本库不实现任何采集客户端。

use crate::error::{TreasuryError, TreasuryResult};
use crate::value::{Frequency, Unit};

/// 美国财政部 Fiscal Data（Phase 1）数据集 ID。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreasuryDatasetId {
    /// DS01 DTS 现金余额（TGA）。
    Ds01,
    /// DS02 DTS 存取款明细。
    Ds02,
    /// DS03 DTS 公共债务交易。
    Ds03,
    /// DS04 国债拍卖结果（**lossy skeleton map，禁作拍卖事实源**）。
    Ds04,
    /// DS05 Debt to the Penny。
    Ds05,
    /// DS06 日度收益率曲线（**路由 `yieldx`**）。
    Ds06,
    /// DS07 实际收益率（TIPS，**路由 `yieldx`**）。
    Ds07,
    /// DS08 MTS 月度收支。
    Ds08,
    /// DS09 未偿证券明细（MSPD）。
    Ds09,
    /// DS10 利息支出。
    Ds10,
}

impl TreasuryDatasetId {
    /// 清单登记形（`DS01`–`DS10`）。
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Ds01 => "DS01",
            Self::Ds02 => "DS02",
            Self::Ds03 => "DS03",
            Self::Ds04 => "DS04",
            Self::Ds05 => "DS05",
            Self::Ds06 => "DS06",
            Self::Ds07 => "DS07",
            Self::Ds08 => "DS08",
            Self::Ds09 => "DS09",
            Self::Ds10 => "DS10",
        }
    }

    /// 本数据集的源侧频率（取自 `specs/adapter/treasury.md` §1.1）。
    #[must_use]
    pub fn frequency(self) -> Frequency {
        match self {
            Self::Ds04 => Frequency::Event,
            Self::Ds08 | Self::Ds09 | Self::Ds10 => Frequency::Monthly,
            _ => Frequency::Daily,
        }
    }

    /// 解析数据集 ID，并对曲线产品 fail-closed。
    ///
    /// # Errors
    ///
    /// - `DS06` / `DS07`（曲线产品）→ [`TreasuryError::RoutedElsewhere`]
    /// - 其余未知 ID → [`TreasuryError::Invalid`]
    pub fn parse_id(text: &str) -> TreasuryResult<Self> {
        let dataset_id = match text {
            "DS01" => Self::Ds01,
            "DS02" => Self::Ds02,
            "DS03" => Self::Ds03,
            "DS04" => Self::Ds04,
            "DS05" => Self::Ds05,
            "DS06" => Self::Ds06,
            "DS07" => Self::Ds07,
            "DS08" => Self::Ds08,
            "DS09" => Self::Ds09,
            "DS10" => Self::Ds10,
            other => {
                return Err(TreasuryError::Invalid(format!("未知数据集 ID：{other}")));
            }
        };
        guard_curve_product(dataset_id)?;
        Ok(dataset_id)
    }
}

/// NY Fed Markets（Phase 2）数据集 ID。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NyFedDatasetId {
    /// FR01 ON RRP。
    Fr01,
    /// FR02 SOMA 持仓。
    Fr02,
    /// FR03 一级交易商统计。
    Fr03,
}

impl NyFedDatasetId {
    /// 清单登记形（`FR01`–`FR03`）。
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Fr01 => "FR01",
            Self::Fr02 => "FR02",
            Self::Fr03 => "FR03",
        }
    }

    /// 本数据集的源侧频率（取自 `specs/adapter/treasury.md` §1.2）。
    #[must_use]
    pub fn frequency(self) -> Frequency {
        match self {
            Self::Fr01 => Frequency::Daily,
            Self::Fr02 | Self::Fr03 => Frequency::Weekly,
        }
    }

    /// 解析 NY Fed 数据集 ID。
    ///
    /// # Errors
    ///
    /// 未知 ID → [`TreasuryError::Invalid`]。
    pub fn parse_id(text: &str) -> TreasuryResult<Self> {
        match text {
            "FR01" => Ok(Self::Fr01),
            "FR02" => Ok(Self::Fr02),
            "FR03" => Ok(Self::Fr03),
            other => Err(TreasuryError::Invalid(format!(
                "未知 NY Fed 数据集 ID：{other}"
            ))),
        }
    }
}

/// FRED 交叉校验序列（Phase 2 · FD01–FD04）。
///
/// 本域是**永久校验面**：权威写入归 [`CANONICAL_WRITE_OWNER`]，本域只对账告警。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FredValidationSeries {
    /// FD01。
    Fd01,
    /// FD02。
    Fd02,
    /// FD03。
    Fd03,
    /// FD04。
    Fd04,
}

impl FredValidationSeries {
    /// 清单登记形（`FD01`–`FD04`）。
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Fd01 => "FD01",
            Self::Fd02 => "FD02",
            Self::Fd03 => "FD03",
            Self::Fd04 => "FD04",
        }
    }

    /// FRED series ID（`WALCL` / `WTREGEN` / `RRPONTSYD` / `WRESBAL`）。
    #[must_use]
    pub fn series_id(self) -> &'static str {
        match self {
            Self::Fd01 => "WALCL",
            Self::Fd02 => "WTREGEN",
            Self::Fd03 => "RRPONTSYD",
            Self::Fd04 => "WRESBAL",
        }
    }

    /// 中文含义（取自 `specs/adapter/treasury.md` §1.3）。
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Fd01 => "美联储总资产",
            Self::Fd02 => "TGA 周频交叉",
            Self::Fd03 => "ON RRP",
            Self::Fd04 => "银行准备金",
        }
    }

    /// 该序列的权威写入方（恒为 [`CANONICAL_WRITE_OWNER`]）。
    #[must_use]
    pub fn canonical_write_owner(self) -> &'static str {
        CANONICAL_WRITE_OWNER
    }

    /// 解析校验序列 ID。
    ///
    /// # Errors
    ///
    /// 未知 ID → [`TreasuryError::Invalid`]。
    pub fn parse_id(text: &str) -> TreasuryResult<Self> {
        match text {
            "FD01" => Ok(Self::Fd01),
            "FD02" => Ok(Self::Fd02),
            "FD03" => Ok(Self::Fd03),
            "FD04" => Ok(Self::Fd04),
            other => Err(TreasuryError::Invalid(format!("未知校验序列 ID：{other}"))),
        }
    }
}

/// Phase 1 数据集全集（`DS01`–`DS10`）。
pub const PHASE1_DATASETS: [TreasuryDatasetId; 10] = [
    TreasuryDatasetId::Ds01,
    TreasuryDatasetId::Ds02,
    TreasuryDatasetId::Ds03,
    TreasuryDatasetId::Ds04,
    TreasuryDatasetId::Ds05,
    TreasuryDatasetId::Ds06,
    TreasuryDatasetId::Ds07,
    TreasuryDatasetId::Ds08,
    TreasuryDatasetId::Ds09,
    TreasuryDatasetId::Ds10,
];

/// 本域的**采集面**（最小集）：`DS01` + `DS05`。
pub const WIRE_COLLECT_DATASETS: [TreasuryDatasetId; 2] =
    [TreasuryDatasetId::Ds01, TreasuryDatasetId::Ds05];

/// 已接 dry-run 声明的数据集（`--wire-dataset DS01|DS04|DS05`）。**dry-run ≠ live**。
pub const DRY_RUN_DECLARED_DATASETS: [TreasuryDatasetId; 3] = [
    TreasuryDatasetId::Ds01,
    TreasuryDatasetId::Ds04,
    TreasuryDatasetId::Ds05,
];

/// `lossy skeleton map` 数据集：**禁止作拍卖事实源**，不进采集常量。
pub const LOSSY_SKELETON_DATASETS: [TreasuryDatasetId; 1] = [TreasuryDatasetId::Ds04];

/// NY Fed Markets 数据集全集（Phase 2）。
pub const NY_FED_DATASETS: [NyFedDatasetId; 3] = [
    NyFedDatasetId::Fr01,
    NyFedDatasetId::Fr02,
    NyFedDatasetId::Fr03,
];

/// FRED 交叉校验序列全集（Phase 2）。
pub const FD_VALIDATION_SERIES: [FredValidationSeries; 4] = [
    FredValidationSeries::Fd01,
    FredValidationSeries::Fd02,
    FredValidationSeries::Fd03,
    FredValidationSeries::Fd04,
];

/// 权威写入方（决策 `WALCL-AUTHORITY-2026-08-20-A`）。
pub const CANONICAL_WRITE_OWNER: &str = "fredx";

/// NY Fed Markets 的决策结论（整体 `NO-GO`）。
pub const NY_FED_DECISION: &str = "NO-GO";

/// 该数据集是否为曲线产品（`DS06` / `DS07`）。
#[must_use]
pub fn is_curve_product(dataset_id: TreasuryDatasetId) -> bool {
    matches!(
        dataset_id,
        TreasuryDatasetId::Ds06 | TreasuryDatasetId::Ds07
    )
}

/// 该 series ID 是否属于 FD01–FD04 永久校验面。
#[must_use]
pub fn is_validation_only_series(series_id: &str) -> bool {
    FD_VALIDATION_SERIES
        .iter()
        .any(|series| series.series_id() == series_id)
}

/// 守卫：曲线产品不得作为普通观测进入本域，必须路由 `yieldx`。
///
/// # Errors
///
/// `DS06` / `DS07` → [`TreasuryError::RoutedElsewhere`]；其余放行。
pub fn guard_curve_product(dataset_id: TreasuryDatasetId) -> TreasuryResult<()> {
    if is_curve_product(dataset_id) {
        return Err(TreasuryError::RoutedElsewhere(format!(
            "{} 是曲线产品，路由 yieldx，不得洗白为普通观测",
            dataset_id.id()
        )));
    }
    Ok(())
}

/// 守卫：该数据集是否可进入本域采集面（`DS01` + `DS05`）。
///
/// # Errors
///
/// - `DS06` / `DS07` → [`TreasuryError::RoutedElsewhere`]
/// - `DS04` → [`TreasuryError::SemanticallyRejected`]（lossy skeleton map，禁作拍卖事实源）
/// - `DS02` / `DS03` / `DS08` / `DS09` / `DS10` → [`TreasuryError::NotApplicable`]（Phase-1 外）
pub fn guard_collection_dataset(dataset_id: TreasuryDatasetId) -> TreasuryResult<()> {
    guard_curve_product(dataset_id)?;
    match dataset_id {
        TreasuryDatasetId::Ds01 | TreasuryDatasetId::Ds05 => Ok(()),
        TreasuryDatasetId::Ds04 => Err(TreasuryError::SemanticallyRejected(
            "DS04 仅为 lossy skeleton map 的 dry-run 声明，禁作拍卖事实源".into(),
        )),
        other => Err(TreasuryError::NotApplicable(format!(
            "{} 不在本域采集面内（采集面 = DS01 + DS05）",
            other.id()
        ))),
    }
}

/// 守卫：FD01–FD04 的权威写入归 `fredx`，本域尝试写入权威键须被拒绝。
///
/// # Errors
///
/// 命中 [`is_validation_only_series`] 时返回 [`TreasuryError::WriteAuthorityDenied`]。
pub fn guard_write_authority(series_id: &str) -> TreasuryResult<()> {
    if is_validation_only_series(series_id) {
        return Err(TreasuryError::WriteAuthorityDenied(format!(
            "{series_id} 的权威写入归 {CANONICAL_WRITE_OWNER}；本域是永久校验面，只对账告警，\
不覆盖权威值、不双写 canonical key"
        )));
    }
    Ok(())
}

/// 守卫：NY Fed Markets 整体 `NO-GO`，任何数据集都不得进入采集面。
///
/// # Errors
///
/// 恒返回 [`TreasuryError::NotApplicable`]。
pub fn guard_ny_fed(dataset_id: NyFedDatasetId) -> TreasuryResult<()> {
    Err(TreasuryError::NotApplicable(format!(
        "{}：NY Fed Markets 整体 {}，不进本域采集面",
        dataset_id.id(),
        NY_FED_DECISION
    )))
}

/// 校验序列的源侧单位。
///
/// `FD04`（`WRESBAL`）未在 `cross-source-routing.md` §4 的单位风险条中登记，故记
/// [`Unit::Unknown`]——本层不得由序列名推断单位。
#[must_use]
pub fn unit_for_validation_series(series: FredValidationSeries) -> Unit {
    match series {
        FredValidationSeries::Fd01 | FredValidationSeries::Fd02 => Unit::MillionsOfUsd,
        FredValidationSeries::Fd03 => Unit::BillionsOfUsd,
        FredValidationSeries::Fd04 => Unit::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_catalog_and_collection_face_match_the_manifest() {
        assert_eq!(PHASE1_DATASETS.len(), 10);
        assert_eq!(
            WIRE_COLLECT_DATASETS,
            [TreasuryDatasetId::Ds01, TreasuryDatasetId::Ds05]
        );
        assert_eq!(NY_FED_DATASETS.len(), 3);
        assert_eq!(FD_VALIDATION_SERIES.len(), 4);
        assert_eq!(CANONICAL_WRITE_OWNER, "fredx");
        assert_eq!(NY_FED_DECISION, "NO-GO");
        // DS04 只出现在 dry-run 声明集，且被登记为 lossy skeleton。
        assert_eq!(LOSSY_SKELETON_DATASETS, [TreasuryDatasetId::Ds04]);
        assert!(DRY_RUN_DECLARED_DATASETS.contains(&TreasuryDatasetId::Ds04));
        assert!(!WIRE_COLLECT_DATASETS.contains(&TreasuryDatasetId::Ds04));
    }

    #[test]
    fn parse_id_fail_closes_on_curve_and_unknown() {
        assert_eq!(
            TreasuryDatasetId::parse_id("DS01").expect("合法 ID"),
            TreasuryDatasetId::Ds01
        );
        for curve in ["DS06", "DS07"] {
            assert_eq!(
                TreasuryDatasetId::parse_id(curve)
                    .expect_err("曲线必须路由")
                    .kind(),
                crate::error::TreasuryErrorKind::RoutedElsewhere,
                "{curve}"
            );
        }
        assert_eq!(
            TreasuryDatasetId::parse_id("DS99")
                .expect_err("未知 ID")
                .kind(),
            crate::error::TreasuryErrorKind::Invalid
        );
        assert_eq!(
            NyFedDatasetId::parse_id("FR99")
                .expect_err("未知 ID")
                .kind(),
            crate::error::TreasuryErrorKind::Invalid
        );
        assert!(FredValidationSeries::parse_id("FD05").is_err());
    }

    #[test]
    fn collection_guard_covers_all_ten_datasets() {
        for dataset_id in PHASE1_DATASETS {
            let result = guard_collection_dataset(dataset_id);
            match dataset_id {
                TreasuryDatasetId::Ds01 | TreasuryDatasetId::Ds05 => assert!(result.is_ok()),
                TreasuryDatasetId::Ds06 | TreasuryDatasetId::Ds07 => assert_eq!(
                    result.expect_err("曲线").kind(),
                    crate::error::TreasuryErrorKind::RoutedElsewhere
                ),
                TreasuryDatasetId::Ds04 => assert_eq!(
                    result.expect_err("lossy").kind(),
                    crate::error::TreasuryErrorKind::SemanticallyRejected
                ),
                _ => assert_eq!(
                    result.expect_err("Phase-1 外").kind(),
                    crate::error::TreasuryErrorKind::NotApplicable
                ),
            }
        }
    }

    #[test]
    fn write_authority_guard_rejects_every_validation_series() {
        for series in FD_VALIDATION_SERIES {
            let series_id = series.series_id();
            assert!(is_validation_only_series(series_id));
            assert_eq!(
                guard_write_authority(series_id)
                    .expect_err("越权写入必须拒绝")
                    .kind(),
                crate::error::TreasuryErrorKind::WriteAuthorityDenied,
                "{series_id}"
            );
            assert_eq!(series.canonical_write_owner(), "fredx");
            assert!(!series.label().is_empty());
        }
        // 非校验序列不受本守卫约束。
        assert!(guard_write_authority("DGS10").is_ok());
        assert!(!is_validation_only_series("WALCLX"));
    }

    #[test]
    fn ny_fed_is_no_go_for_every_dataset() {
        for dataset_id in NY_FED_DATASETS {
            assert_eq!(
                guard_ny_fed(dataset_id).expect_err("NY Fed NO-GO").kind(),
                crate::error::TreasuryErrorKind::NotApplicable,
                "{dataset_id:?}"
            );
        }
    }

    #[test]
    fn unit_risk_entry_is_expressed() {
        assert_eq!(
            unit_for_validation_series(FredValidationSeries::Fd03),
            Unit::BillionsOfUsd
        );
        assert_eq!(
            unit_for_validation_series(FredValidationSeries::Fd01),
            Unit::MillionsOfUsd
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

    #[test]
    fn dataset_frequencies_match_the_manifest() {
        assert_eq!(TreasuryDatasetId::Ds01.frequency(), Frequency::Daily);
        assert_eq!(TreasuryDatasetId::Ds04.frequency(), Frequency::Event);
        assert_eq!(TreasuryDatasetId::Ds08.frequency(), Frequency::Monthly);
        assert_eq!(NyFedDatasetId::Fr01.frequency(), Frequency::Daily);
        assert_eq!(NyFedDatasetId::Fr02.frequency(), Frequency::Weekly);
    }
}
