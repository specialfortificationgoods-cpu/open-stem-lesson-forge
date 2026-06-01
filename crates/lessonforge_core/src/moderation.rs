use crate::ids::{LeaseId, RequestId, RequestModerationReportId, RequestModerationTaskId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationKind {
    DummyFixture,
    ProviderBacked,
    HumanCuratorFixture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationDecision {
    AllowMvpPlanning,
    RejectRequest,
    QuarantineRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationCategory {
    None,
    Sexual,
    SexualMinors,
    Violence,
    SelfHarm,
    Hate,
    Harassment,
    Illicit,
    Weapons,
    Privacy,
    AgeInappropriate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
#[serde(rename_all = "snake_case")]
pub enum ModerationSafeReason {
    ModerationAllowed,
    ModerationRejectedSexual,
    ModerationRejectedSexualMinors,
    ModerationRejectedViolence,
    ModerationRejectedSelfHarm,
    ModerationRejectedHateOrHarassment,
    ModerationRejectedIllicit,
    ModerationRejectedWeapons,
    ModerationRejectedPrivacy,
    ModerationRejectedAgeInappropriate,
    ModerationQuarantineReviewNeeded,
    ModerationSchemaInvalid,
    ModerationLineageMismatch,
    ModerationStaleLease,
    ModerationDeterministicHeuristicOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationReportState {
    Submitted,
    Accepted,
    Rejected,
}

impl ModerationReportState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationReportSubmission {
    pub request_moderation_report_id: RequestModerationReportId,
    pub request_moderation_task_id: RequestModerationTaskId,
    pub request_id: RequestId,
    pub lease_id: LeaseId,
    pub claim_token: String,
    pub moderation_kind: ModerationKind,
    pub decision: ModerationDecision,
    pub category_flags: Vec<ModerationCategory>,
    pub safe_reason_codes: Vec<ModerationSafeReason>,
}

impl ModerationReportSubmission {
    pub fn is_consistent(&self) -> bool {
        let contains_none = self.category_flags.contains(&ModerationCategory::None);
        if contains_none && self.category_flags.len() != 1 {
            return false;
        }

        match self.decision {
            ModerationDecision::AllowMvpPlanning => {
                self.category_flags == [ModerationCategory::None]
                    && self.safe_reason_codes == [ModerationSafeReason::ModerationAllowed]
            }
            ModerationDecision::RejectRequest | ModerationDecision::QuarantineRequest => {
                !contains_none
                    && !self.category_flags.is_empty()
                    && categories_are_unique(&self.category_flags)
                    && reasons_are_unique(&self.safe_reason_codes)
                    && self.category_flags.iter().all(|category| {
                        matching_reason_present(*category, self.decision, &self.safe_reason_codes)
                    })
                    && self.safe_reason_codes.iter().all(|reason| {
                        reason_matches_flagged_category(
                            *reason,
                            self.decision,
                            &self.category_flags,
                        )
                    })
            }
        }
    }
}

fn categories_are_unique(categories: &[ModerationCategory]) -> bool {
    let mut seen = Vec::with_capacity(categories.len());
    for category in categories {
        if seen.contains(category) {
            return false;
        }
        seen.push(*category);
    }
    true
}

fn reasons_are_unique(reasons: &[ModerationSafeReason]) -> bool {
    let mut seen = Vec::with_capacity(reasons.len());
    for reason in reasons {
        if seen.contains(reason) {
            return false;
        }
        seen.push(*reason);
    }
    true
}

fn matching_reason_present(
    category: ModerationCategory,
    decision: ModerationDecision,
    reasons: &[ModerationSafeReason],
) -> bool {
    // Quarantine is a safe terminal review path rather than a category-specific
    // rejection, so the general quarantine reason deliberately satisfies every
    // flagged category for quarantine decisions only.
    if decision == ModerationDecision::QuarantineRequest
        && reasons.contains(&ModerationSafeReason::ModerationQuarantineReviewNeeded)
    {
        return true;
    }
    let expected = match category {
        ModerationCategory::None => ModerationSafeReason::ModerationAllowed,
        ModerationCategory::Sexual => ModerationSafeReason::ModerationRejectedSexual,
        ModerationCategory::SexualMinors => ModerationSafeReason::ModerationRejectedSexualMinors,
        ModerationCategory::Violence => ModerationSafeReason::ModerationRejectedViolence,
        ModerationCategory::SelfHarm => ModerationSafeReason::ModerationRejectedSelfHarm,
        ModerationCategory::Hate | ModerationCategory::Harassment => {
            ModerationSafeReason::ModerationRejectedHateOrHarassment
        }
        ModerationCategory::Illicit => ModerationSafeReason::ModerationRejectedIllicit,
        ModerationCategory::Weapons => ModerationSafeReason::ModerationRejectedWeapons,
        ModerationCategory::Privacy => ModerationSafeReason::ModerationRejectedPrivacy,
        ModerationCategory::AgeInappropriate => {
            ModerationSafeReason::ModerationRejectedAgeInappropriate
        }
    };
    reasons.contains(&expected)
}

fn reason_matches_flagged_category(
    reason: ModerationSafeReason,
    decision: ModerationDecision,
    categories: &[ModerationCategory],
) -> bool {
    // Keep this symmetric with matching_reason_present: the general quarantine
    // reason is valid for any flagged category only when the decision is
    // QuarantineRequest.
    if decision == ModerationDecision::QuarantineRequest
        && reason == ModerationSafeReason::ModerationQuarantineReviewNeeded
    {
        return true;
    }
    categories
        .iter()
        .any(|category| matching_reason_present(*category, decision, &[reason]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarantine_review_reason_short_circuits_all_flagged_categories() {
        let flagged_categories = [
            ModerationCategory::Sexual,
            ModerationCategory::SexualMinors,
            ModerationCategory::Violence,
            ModerationCategory::SelfHarm,
            ModerationCategory::Hate,
            ModerationCategory::Harassment,
            ModerationCategory::Illicit,
            ModerationCategory::Weapons,
            ModerationCategory::Privacy,
            ModerationCategory::AgeInappropriate,
        ];

        for category in flagged_categories {
            assert!(matching_reason_present(
                category,
                ModerationDecision::QuarantineRequest,
                &[ModerationSafeReason::ModerationQuarantineReviewNeeded],
            ));
            assert!(reason_matches_flagged_category(
                ModerationSafeReason::ModerationQuarantineReviewNeeded,
                ModerationDecision::QuarantineRequest,
                &[category],
            ));
        }
    }

    #[test]
    fn reject_request_still_requires_category_specific_reasons() {
        assert!(!matching_reason_present(
            ModerationCategory::Violence,
            ModerationDecision::RejectRequest,
            &[ModerationSafeReason::ModerationQuarantineReviewNeeded],
        ));
        assert!(!reason_matches_flagged_category(
            ModerationSafeReason::ModerationQuarantineReviewNeeded,
            ModerationDecision::RejectRequest,
            &[ModerationCategory::Violence],
        ));
        assert!(matching_reason_present(
            ModerationCategory::Violence,
            ModerationDecision::RejectRequest,
            &[ModerationSafeReason::ModerationRejectedViolence],
        ));
    }
}
