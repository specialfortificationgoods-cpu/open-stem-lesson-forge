use crate::error::IdError;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! typed_id {
    ($name:ident, $entity:literal, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub const PREFIX: &'static str = $prefix;

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<&str> for $name {
            type Error = IdError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                validate_id($entity, Self::PREFIX, value)?;
                Ok(Self(value.to_owned()))
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                validate_id($entity, Self::PREFIX, &value)?;
                Ok(Self(value))
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

typed_id!(RequestId, "request", "req_");
typed_id!(
    RequestModerationTaskId,
    "request_moderation_task",
    "rmtask_"
);
typed_id!(
    RequestModerationReportId,
    "request_moderation_report",
    "rmreport_"
);
typed_id!(PlanningTaskId, "planning_task", "ptask_");
typed_id!(ProposedTaskGraphId, "proposed_task_graph", "plan_");
typed_id!(PlanVerificationTaskId, "plan_verification_task", "pvtask_");
typed_id!(PlanVerificationId, "plan_verification", "pverify_");
typed_id!(PromotionDecisionId, "promotion_decision", "promo_");
typed_id!(WorkPacketId, "work_packet", "wp_");
typed_id!(LeaseId, "lease", "lease_");
typed_id!(ArtifactId, "artifact", "art_");
typed_id!(ValidationReportId, "validation_report", "vreport_");
typed_id!(ReviewTaskId, "review_task", "rtask_");
typed_id!(ReviewId, "review", "review_");
typed_id!(FindingId, "finding", "finding_");
typed_id!(ActorId, "actor", "actor_");
typed_id!(StateTransitionEventId, "state_transition_event", "event_");
typed_id!(PublicArtifactLabelId, "public_artifact_label", "plabel_");

fn validate_id(entity: &'static str, prefix: &'static str, value: &str) -> Result<(), IdError> {
    let Some(suffix) = value.strip_prefix(prefix) else {
        return Err(IdError::InvalidPrefix { entity, prefix });
    };
    if suffix.is_empty() {
        return Err(IdError::EmptySuffix { entity });
    }
    if suffix_has_unsafe_content(suffix) {
        return Err(IdError::UnsafeContent { entity });
    }
    Ok(())
}

fn suffix_has_unsafe_content(suffix: &str) -> bool {
    const MAX_ID_SUFFIX_LEN: usize = 96;
    const FORBIDDEN_SUFFIX_PARTS: &[&str] = &[
        "api_key",
        "cookie",
        "credentials",
        "credential",
        "email",
        "home",
        "key",
        "local_path",
        "oauth",
        "password",
        "provider",
        "secret",
        "student",
        "tmp",
        "token",
        "users",
    ];
    const FORBIDDEN_COMPACT_SUFFIX_PARTS: &[&str] = &[
        "accesstoken",
        "apikey",
        "authtoken",
        "bearertoken",
        "clientsecret",
        "idtoken",
        "jwttoken",
        "refreshtoken",
        "secretkey",
        "sessiontoken",
    ];

    let lowercase = suffix.to_ascii_lowercase();
    suffix.len() > MAX_ID_SUFFIX_LEN
        || lowercase.starts_with("sk-")
        || lowercase.starts_with("sk_")
        || lowercase.contains("-sk-")
        || lowercase.contains("-sk_")
        || lowercase.contains("_sk-")
        || lowercase.contains("_sk_")
        || suffix_has_forbidden_compact_alias(&lowercase, FORBIDDEN_COMPACT_SUFFIX_PARTS)
        || suffix_has_forbidden_segment(&lowercase, FORBIDDEN_SUFFIX_PARTS)
        || !suffix.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
}

fn suffix_has_forbidden_segment(lowercase: &str, forbidden_parts: &[&str]) -> bool {
    forbidden_parts.iter().any(|forbidden| {
        lowercase == *forbidden
            || lowercase
                .strip_prefix(forbidden)
                .is_some_and(|tail| tail.starts_with(['_', '-']))
            || lowercase
                .strip_suffix(forbidden)
                .is_some_and(|head| head.ends_with(['_', '-']))
            || lowercase.contains(&format!("_{forbidden}_"))
            || lowercase.contains(&format!("_{forbidden}-"))
            || lowercase.contains(&format!("-{forbidden}_"))
            || lowercase.contains(&format!("-{forbidden}-"))
    })
}

fn suffix_has_forbidden_compact_alias(lowercase: &str, forbidden_parts: &[&str]) -> bool {
    forbidden_parts
        .iter()
        .any(|forbidden| lowercase.contains(forbidden))
}
