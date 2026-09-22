//! treasuryx 的错误分类与错误类型。

/// 错误分类：按「调用方应如何反应」划分。
///
/// 禁止用字符串匹配替代对本枚举的匹配。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreasuryErrorKind {
    /// 输入形态或取值非法（调用方应修数据，重试无意义）。
    Invalid,
    /// 缺少必需项。
    Missing,
    /// 授权判定未通过（fail-closed 落点）。
    AuthorizationDenied,
    /// 该产品不属于本域，已被路由规则拒绝。
    RoutedElsewhere,
    /// 越权写入（权威写入归他域）。
    WriteAuthorityDenied,
    /// 结构可解析但语义不被接受（如近义非同 ID）。
    SemanticallyRejected,
    /// 尚未实现的规划能力。
    NotApplicable,
    /// 不变量被破坏（库内 bug 的信号）。
    Invariant,
}

/// treasuryx 错误。保留可区分的分类与来源链。
///
/// 错误消息一律为简体中文，且不回显原始样本正文、凭据或整行配置源码。
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum TreasuryError {
    /// 输入非法。
    #[error("输入非法：{0}")]
    Invalid(String),
    /// 缺少必需项。
    #[error("缺少必需项：{0}")]
    Missing(String),
    /// 授权判定未通过。
    #[error("授权判定未通过：{0}")]
    AuthorizationDenied(String),
    /// 产品不属于本域。
    #[error("不属于本域，已路由他处：{0}")]
    RoutedElsewhere(String),
    /// 越权写入。
    #[error("越权写入被拒绝：{0}")]
    WriteAuthorityDenied(String),
    /// 语义拒绝。
    #[error("语义不被接受：{0}")]
    SemanticallyRejected(String),
    /// 规划能力未实现。
    #[error("规划能力尚未实现：{0}")]
    NotApplicable(String),
    /// 不变量被破坏。
    #[error("不变量被破坏：{0}")]
    Invariant(String),
}

impl TreasuryError {
    /// 分类，供调用方按「如何反应」分流。
    #[must_use]
    pub fn kind(&self) -> TreasuryErrorKind {
        match self {
            Self::Invalid(_) => TreasuryErrorKind::Invalid,
            Self::Missing(_) => TreasuryErrorKind::Missing,
            Self::AuthorizationDenied(_) => TreasuryErrorKind::AuthorizationDenied,
            Self::RoutedElsewhere(_) => TreasuryErrorKind::RoutedElsewhere,
            Self::WriteAuthorityDenied(_) => TreasuryErrorKind::WriteAuthorityDenied,
            Self::SemanticallyRejected(_) => TreasuryErrorKind::SemanticallyRejected,
            Self::NotApplicable(_) => TreasuryErrorKind::NotApplicable,
            Self::Invariant(_) => TreasuryErrorKind::Invariant,
        }
    }

    /// 是否值得重试。本层无网络，除 `Invariant` 外一律 `false`。
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(self.kind(), TreasuryErrorKind::Invariant)
    }
}

/// 本 crate 统一结果别名。
pub type TreasuryResult<T> = Result<T, TreasuryError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_maps_every_variant_distinctly() {
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
            assert_eq!(error.kind(), expected, "分类不匹配：{error:?}");
        }
    }

    #[test]
    fn only_invariant_is_retryable() {
        assert!(TreasuryError::Invariant("x".into()).is_retryable());
        for error in [
            TreasuryError::Invalid("x".into()),
            TreasuryError::Missing("x".into()),
            TreasuryError::AuthorizationDenied("x".into()),
            TreasuryError::RoutedElsewhere("x".into()),
            TreasuryError::WriteAuthorityDenied("x".into()),
            TreasuryError::SemanticallyRejected("x".into()),
            TreasuryError::NotApplicable("x".into()),
        ] {
            assert!(!error.is_retryable(), "本层无网络，不应可重试：{error:?}");
        }
    }

    #[test]
    fn display_is_non_empty_and_keeps_the_reason() {
        let error = TreasuryError::WriteAuthorityDenied("WALCL 权威写入归 fredx".into());
        let text = error.to_string();
        assert!(!text.is_empty());
        assert!(text.contains("WALCL 权威写入归 fredx"), "实际：{text}");
    }

    #[test]
    fn four_kinds_are_distinguishable_without_string_matching() {
        let kinds = [
            TreasuryErrorKind::AuthorizationDenied,
            TreasuryErrorKind::RoutedElsewhere,
            TreasuryErrorKind::WriteAuthorityDenied,
            TreasuryErrorKind::SemanticallyRejected,
        ];
        for (i, left) in kinds.iter().enumerate() {
            for right in kinds.iter().skip(i + 1) {
                assert_ne!(left, right, "分类必须互不相等");
            }
        }
    }
}
