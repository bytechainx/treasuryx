//! treasuryx 的授权判定：fail-closed 的只读结论。
//!
//! 判定 MUST NOT 改写清单的原登记值；`Denied` MUST 携带可读理由。
//! 本模块判定的是「该源的这个范围是否被授权访问」，**不是**「本库是否生产就绪」。

/// Owner 签核决策 ID（声明层常量，不是运行时凭据）。
pub const DECISION_ID: &str = "TREASURY-B1-2026-08-20-OFFLINE";

/// 本 v2 签核覆盖的旧决策 ID（仅作历史记录；旧签绑定 R-B 前旧树，已失效）。
pub const SUPERSEDED_DECISION_ID: &str = "TREASURY-PROD-2026-08-17-approve";

/// 本库的授权范围：仅 offline fixture。
pub const AUTHORIZED_SCOPE: &str = "offline_fixture_only";

/// 授权通道：**仅为未来 live 阶段预授权**，当前不实现任何 live / HTTP 客户端。
pub const LIVE_PREAUTH_CHANNEL: &str = "Treasury Fiscal Data 官方 API（未来 live 阶段预授权通道）";

/// live 采集是否需要另批决策 ID + B2–B8 门禁（恒为 `true`）。
pub const LIVE_REQUIRES_NEW_DECISION: bool = true;

/// 请求的授权范围。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryScope {
    /// 离线 fixture（本库的唯一授权面）。
    OfflineFixtureOnly,
    /// 联网采集（本库不实现；本签不覆盖，须另批决策 ID）。
    LiveCollection,
}

/// 授权判定输入：Owner 签核文件中的可核对字段（只读快照）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryAuthorizationEvidence {
    /// 决策 ID。
    pub decision_id: String,
    /// 签署者。
    pub signed_by: String,
    /// 该签核覆盖的范围。
    pub scope: String,
}

impl TreasuryAuthorizationEvidence {
    /// 构造证据快照（不做判定）。
    #[must_use]
    pub fn new(decision_id: &str, signed_by: &str, scope: &str) -> Self {
        Self {
            decision_id: decision_id.into(),
            signed_by: signed_by.into(),
            scope: scope.into(),
        }
    }
}

/// 授权判定结果。
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreasuryAuthorization {
    /// 证据有效且覆盖本次请求的范围与有效期。
    Authorized {
        /// 被覆盖的源 / 端点 / 模式 / 用途 / 有效期。
        scope: String,
    },
    /// 证据缺失 / 过期 / 签署者不明 / 覆盖范围不明。
    Denied {
        /// 可读的拒绝理由。
        reason: String,
    },
}

/// fail-closed 的授权判定。
///
/// 范围逐字匹配：`offline_fixture_only` 来自 Owner 签核的 product_boundary，
/// `treasury_offline` 为既有离线证据别名；其他源范围和带前后空白者不接受。
///
/// 证据缺失、决策 ID 空白、签署者空白或覆盖范围空白 → **一律** `Denied`；
/// 请求 `LiveCollection` → **一律** `Denied`（须另批决策 ID + B2–B8 门禁）。
/// MUST NOT 默认放行。
#[must_use]
pub fn authorize_treasury(
    scope: TreasuryScope,
    evidence: Option<&TreasuryAuthorizationEvidence>,
) -> TreasuryAuthorization {
    if matches!(scope, TreasuryScope::LiveCollection) {
        return TreasuryAuthorization::Denied {
            reason: format!(
                "live 采集须另批决策 ID + B2–B8 门禁；{DECISION_ID} 不构成 live 授权，\
本库亦不实现 live / HTTP 客户端"
            ),
        };
    }
    let Some(evidence) = evidence else {
        return TreasuryAuthorization::Denied {
            reason: "缺少授权证据（fail-closed）".into(),
        };
    };
    if evidence.decision_id.trim().is_empty() {
        return TreasuryAuthorization::Denied {
            reason: "证据缺少决策 ID（签署者不明）".into(),
        };
    }
    if evidence.signed_by.trim().is_empty() {
        return TreasuryAuthorization::Denied {
            reason: "证据缺少签署者（签署者不明）".into(),
        };
    }
    if !matches!(
        evidence.scope.as_str(),
        "offline_fixture_only" | "treasury_offline"
    ) {
        return TreasuryAuthorization::Denied {
            reason: "证据范围未覆盖本次离线请求（范围未知或不匹配）".into(),
        };
    }
    TreasuryAuthorization::Authorized {
        scope: format!(
            "{AUTHORIZED_SCOPE}（decision_id={}，签署者={}；授权通道仅为未来 live 阶段预授权，\
live 须另批）",
            evidence.decision_id, evidence.signed_by
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> TreasuryAuthorizationEvidence {
        TreasuryAuthorizationEvidence::new(DECISION_ID, "ZoneCNH", "treasury_offline")
    }

    #[test]
    fn authorized_scope_is_explicitly_offline_only() {
        match authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&evidence())) {
            TreasuryAuthorization::Authorized { scope } => {
                assert!(scope.contains(AUTHORIZED_SCOPE), "实际：{scope}");
                assert!(scope.contains("live 须另批"), "必须显式声明仅覆盖离线范围");
                assert!(!scope.contains("production"), "不得声称生产就绪：{scope}");
            }
            other => panic!("离线面应授权：{other:?}"),
        }
    }

    #[test]
    fn missing_evidence_is_denied() {
        assert!(matches!(
            authorize_treasury(TreasuryScope::OfflineFixtureOnly, None),
            TreasuryAuthorization::Denied { .. }
        ));
    }

    #[test]
    fn live_collection_is_denied_even_with_valid_evidence() {
        let verdict = authorize_treasury(TreasuryScope::LiveCollection, Some(&evidence()));
        match verdict {
            TreasuryAuthorization::Denied { reason } => {
                assert!(reason.contains("另批"), "实际：{reason}");
            }
            other => panic!("live 必须拒绝：{other:?}"),
        }
        assert_eq!(
            u8::from(LIVE_REQUIRES_NEW_DECISION),
            1,
            "live 恒须另批决策 ID + B2–B8 门禁"
        );
    }

    #[test]
    fn blank_evidence_fields_are_denied() {
        for blank in ["", "   "] {
            let blank_id = TreasuryAuthorizationEvidence::new(blank, "ZoneCNH", "s");
            assert!(matches!(
                authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&blank_id)),
                TreasuryAuthorization::Denied { .. }
            ));
            let blank_signer = TreasuryAuthorizationEvidence::new(DECISION_ID, blank, "s");
            assert!(matches!(
                authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&blank_signer)),
                TreasuryAuthorization::Denied { .. }
            ));
            let blank_scope = TreasuryAuthorizationEvidence::new(DECISION_ID, "ZoneCNH", blank);
            assert!(matches!(
                authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&blank_scope)),
                TreasuryAuthorization::Denied { .. }
            ));
        }
    }

    #[test]
    fn declaration_constants_match_the_manifest() {
        assert_eq!(DECISION_ID, "TREASURY-B1-2026-08-20-OFFLINE");
        assert_eq!(SUPERSEDED_DECISION_ID, "TREASURY-PROD-2026-08-17-approve");
        assert!(!LIVE_PREAUTH_CHANNEL.is_empty());
    }

    #[test]
    fn authorization_checks_scope_coverage() {
        for scope in [
            "unknown",
            "unrelated_source_only",
            "live_only",
            " offline_fixture_only ",
            " domain_yahoo_release",
            "domain_yahoo_release ",
            "treasury_offline-extra",
        ] {
            let e = TreasuryAuthorizationEvidence::new(DECISION_ID, "ZoneCNH", scope);
            assert!(matches!(
                authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&e)),
                TreasuryAuthorization::Denied { .. }
            ));
        }
        for scope in ["offline_fixture_only", "treasury_offline"] {
            let e = TreasuryAuthorizationEvidence::new(DECISION_ID, "ZoneCNH", scope);
            assert!(matches!(
                authorize_treasury(TreasuryScope::OfflineFixtureOnly, Some(&e)),
                TreasuryAuthorization::Authorized { .. }
            ));
        }
    }
}
