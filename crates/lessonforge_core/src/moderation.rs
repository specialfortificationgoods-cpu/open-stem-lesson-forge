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
                    && self
                        .category_flags
                        .iter()
                        .all(|category| matching_reason_present(*category, &self.safe_reason_codes))
            }
        }
    }
}

fn matching_reason_present(category: ModerationCategory, reasons: &[ModerationSafeReason]) -> bool {
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
