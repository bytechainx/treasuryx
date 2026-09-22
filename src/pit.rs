//! treasuryx 的 publication 语义：时间精度、可得性证据层与正式 PIT 资格。
//!
//! 本源的 publication 时刻属**推断层**：Fiscal Data（DS01–DS05 日更数据集）
//! 无官方发布时刻字段，亦无 Owner 签批发布日历。

/// 时间精度：源只给日期还是给出时刻。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimePrecision {
    /// 只有日期。
    Date,
    /// 有明确时刻。
    Instant,
}

/// 可得性证据层：官方字段 > 日历 > 推断。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityEvidence {
    /// 官方字段。
    Official,
    /// 发布日历。
    Calendar,
    /// 推断。
    Inferred,
}

/// 正式 PIT 资格。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitEligibility {
    /// 可进正式 PIT。
    Formal,
    /// 不可进正式 PIT。
    NotEligible,
}

/// 本源的 publication 语义三元组，恒为 `(Date, Inferred, NotEligible)`。
///
/// MUST NOT 补造 `00:00 UTC` 之类时刻把 `Date` 伪装成 `Instant`。
#[must_use]
pub fn publication_semantics() -> (TimePrecision, AvailabilityEvidence, PitEligibility) {
    (
        TimePrecision::Date,
        AvailabilityEvidence::Inferred,
        PitEligibility::NotEligible,
    )
}

/// 是否可进正式 PIT。恒为 `false`（证据层为推断，PIT-AVAIL-001）。
#[must_use]
pub fn is_formal_pit_eligible() -> bool {
    let (_, _, eligibility) = publication_semantics();
    matches!(eligibility, PitEligibility::Formal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_semantics_is_pinned_to_date_inferred_not_eligible() {
        assert_eq!(
            publication_semantics(),
            (
                TimePrecision::Date,
                AvailabilityEvidence::Inferred,
                PitEligibility::NotEligible
            )
        );
    }

    #[test]
    fn formal_pit_eligibility_is_false() {
        assert!(!is_formal_pit_eligible(), "推断层不得进入正式 PIT");
    }
}
