use crate::ids::{ActorId, LeaseId, PlanningTaskId, RequestId, RequestModerationTaskId};
use crate::moderation::{ModerationDecision, ModerationReportState, ModerationReportSubmission};
use crate::planning::{PlanningTaskRecord, mechanical_planning_task};
use crate::state::{
    ActorCapability, RequestModerationTaskState, RequestState, RequestTransition, TrustLevel,
};
use crate::validation::pii_detection::has_phone_like_number;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub type RequestIntakePayload = Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoRepairPreference {
    NoAutomatedRepair,
    RequestBoundedCodeRepair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredRequestVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestPublicStatus {
    Requested,
}

impl RequestPublicStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntakeRejectionReason {
    MissingRequiredField,
    UnknownField,
    InvalidField,
    UnsupportedMvpValue,
    UnsafeText,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestWorkflowError {
    #[error("request intake rejected: {reason:?} at {field_path}")]
    Rejected {
        reason: IntakeRejectionReason,
        field_path: String,
    },
    #[error("request moderation rejected: {reason}")]
    ModerationRejected { reason: &'static str },
    #[error("request workflow conflict: {reason}")]
    Conflict { reason: &'static str },
    #[error(transparent)]
    Transition(#[from] crate::error::TransitionError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeContext {
    pub request_id: RequestId,
    pub moderation_task_id: RequestModerationTaskId,
    pub planning_task_id: PlanningTaskId,
    pub scope_id: String,
    pub created_by_actor_id: ActorId,
    pub now: String,
    pub default_auto_repair_preference: AutoRepairPreference,
    pub default_visibility: StoredRequestVisibility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRequest {
    pub request_id: RequestId,
    pub scope_id: String,
    pub created_by_actor_id: ActorId,
    pub created_at: String,
    pub state: RequestState,
    pub title: String,
    pub subject: String,
    pub topic: String,
    pub age_range: String,
    pub language: String,
    pub lesson_duration_minutes: u16,
    pub desired_artifacts: Vec<String>,
    pub constraints: Vec<String>,
    pub license_preference: String,
    pub visibility: StoredRequestVisibility,
    pub forbidden_content_acknowledged: bool,
    pub auto_repair_preference: AutoRepairPreference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestModerationTaskRecord {
    pub task_id: RequestModerationTaskId,
    pub request_id: RequestId,
    pub scope_id: String,
    pub state: RequestModerationTaskState,
    pub task_type: &'static str,
    pub input_refs: Vec<String>,
    pub required_output_schema: &'static str,
    pub allowed_outputs: Vec<&'static str>,
    pub forbidden_outputs: Vec<&'static str>,
    pub required_capabilities: Vec<ActorCapability>,
    pub minimum_runner_trust_level: TrustLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestIntakeOutcome {
    pub request: StoredRequest,
    pub public_status: RequestPublicStatus,
    pub moderation_task: RequestModerationTaskRecord,
    pub planning_task: Option<PlanningTaskRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestModerationContext {
    pub request_id: RequestId,
    pub moderation_task_id: RequestModerationTaskId,
    pub planning_task_id: PlanningTaskId,
    pub moderator_actor_id: ActorId,
    pub lease_id: LeaseId,
    pub claim_token_hash: String,
    pub scope_id: String,
    pub lease_active: bool,
    pub actor_scope_matches: bool,
    pub actor_can_moderate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationApplicationOutcome {
    pub request_state: RequestState,
    pub report_state: ModerationReportState,
    pub planning_task: Option<PlanningTaskRecord>,
}

pub fn accept_request_intake(
    payload: RequestIntakePayload,
    context: IntakeContext,
) -> Result<RequestIntakeOutcome, RequestWorkflowError> {
    let object = payload.as_object().ok_or(RequestWorkflowError::Rejected {
        reason: IntakeRejectionReason::InvalidField,
        field_path: "/".to_owned(),
    })?;
    reject_unknown_fields(object.keys().map(String::as_str))?;

    let title = required_limited_string(object, "title", 1, 120)?;
    let subject = required_enum(object, "subject", &["physics"])?;
    let topic = required_topic(object)?;
    let age_range = required_enum(object, "age_range", &["14-16"])?;
    let language = required_enum(object, "language", &["en"])?;
    let lesson_duration_minutes = required_duration_minutes(object)?;
    let desired_artifacts = required_desired_artifacts(object)?;
    let constraints = optional_constraints(object)?;
    let license_preference = required_enum(object, "license_preference", &["CC-BY-4.0"])?;
    let visibility = optional_visibility(object, context.default_visibility)?;
    let auto_repair_preference =
        optional_auto_repair_preference(object, context.default_auto_repair_preference)?;
    require_true(object, "forbidden_content_acknowledged")?;

    for (value, field_path) in [
        (&title, "/title"),
        (&subject, "/subject"),
        (&topic, "/topic"),
        (&age_range, "/age_range"),
        (&language, "/language"),
        (&license_preference, "/license_preference"),
    ] {
        reject_unsafe_text(value, field_path)?;
    }
    for (index, value) in desired_artifacts.iter().enumerate() {
        reject_unsafe_text(value, &format!("/desired_artifacts/{index}"))?;
    }
    for (index, value) in constraints.iter().enumerate() {
        reject_unsafe_text(value, &format!("/constraints/{index}"))?;
    }

    let request_state =
        RequestState::Requested.transition(RequestTransition::DeterministicIntakePassed)?;
    let request = StoredRequest {
        request_id: context.request_id.clone(),
        scope_id: context.scope_id.clone(),
        created_by_actor_id: context.created_by_actor_id,
        created_at: context.now,
        state: request_state,
        title,
        subject,
        topic,
        age_range,
        language,
        lesson_duration_minutes,
        desired_artifacts,
        constraints,
        license_preference,
        visibility,
        forbidden_content_acknowledged: true,
        auto_repair_preference,
    };
    let moderation_task = RequestModerationTaskRecord {
        task_id: context.moderation_task_id,
        request_id: context.request_id,
        scope_id: context.scope_id,
        state: RequestModerationTaskState::Open,
        task_type: "moderate_request",
        input_refs: vec![request.request_id.to_string()],
        required_output_schema: "request_moderation_report.schema.json",
        allowed_outputs: vec!["request_moderation_report"],
        forbidden_outputs: vec![
            "arbitrary_prompt",
            "provider_raw_response",
            "provider_credentials",
            "student_grading_task",
        ],
        required_capabilities: vec![
            ActorCapability::ContentModeration,
            ActorCapability::AgeAppropriatenessClassification,
            ActorCapability::StructuredJsonOutput,
        ],
        minimum_runner_trust_level: TrustLevel::ModerationCandidate,
    };

    Ok(RequestIntakeOutcome {
        request,
        public_status: RequestPublicStatus::Requested,
        moderation_task,
        planning_task: None,
    })
}

pub fn apply_moderation_report(
    request: &StoredRequest,
    context: RequestModerationContext,
    report: ModerationReportSubmission,
) -> Result<ModerationApplicationOutcome, RequestWorkflowError> {
    if !context.lease_active || !context.actor_scope_matches || !context.actor_can_moderate {
        return Err(RequestWorkflowError::ModerationRejected {
            reason: "moderation_context_not_authorized",
        });
    }
    if context.request_id != request.request_id
        || context.scope_id != request.scope_id
        || report.request_id != request.request_id
        || report.request_moderation_task_id != context.moderation_task_id
        || report.lease_id != context.lease_id
        || crate::state::Lease::claim_token_hash(&report.claim_token) != context.claim_token_hash
    {
        return Err(RequestWorkflowError::ModerationRejected {
            reason: "moderation_lineage_mismatch",
        });
    }
    if request.state != RequestState::ModerationPending || !report.is_consistent() {
        return Err(RequestWorkflowError::ModerationRejected {
            reason: "moderation_report_inconsistent",
        });
    }

    match report.decision {
        ModerationDecision::AllowMvpPlanning => {
            let moderation_passed = request
                .state
                .transition(RequestTransition::ModerationAllowsPlanning)?;
            let planning_open =
                moderation_passed.transition(RequestTransition::CreatePlanningTask)?;
            Ok(ModerationApplicationOutcome {
                request_state: planning_open,
                report_state: ModerationReportState::Accepted,
                planning_task: Some(mechanical_planning_task(
                    context.planning_task_id,
                    request.request_id.clone(),
                    request.scope_id.clone(),
                )),
            })
        }
        ModerationDecision::RejectRequest => Ok(ModerationApplicationOutcome {
            request_state: request
                .state
                .transition(RequestTransition::ModerationRejects)?,
            report_state: ModerationReportState::Accepted,
            planning_task: None,
        }),
        ModerationDecision::QuarantineRequest => Ok(ModerationApplicationOutcome {
            request_state: request
                .state
                .transition(RequestTransition::ModerationQuarantines)?,
            report_state: ModerationReportState::Accepted,
            planning_task: None,
        }),
    }
}

fn reject_unknown_fields<'a>(
    mut keys: impl Iterator<Item = &'a str>,
) -> Result<(), RequestWorkflowError> {
    let allowed = [
        "title",
        "subject",
        "topic",
        "age_range",
        "language",
        "lesson_duration_minutes",
        "desired_artifacts",
        "constraints",
        "license_preference",
        "visibility",
        "forbidden_content_acknowledged",
        "auto_repair_preference",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if keys.find(|key| !allowed.contains(key)).is_some() {
        return Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::UnknownField,
            field_path: "/unknown_field".to_owned(),
        });
    }
    Ok(())
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<String, RequestWorkflowError> {
    object
        .get(field)
        .ok_or(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::MissingRequiredField,
            field_path: format!("/{field}"),
        })?
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: format!("/{field}"),
        })
}

fn required_limited_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
    min: usize,
    max: usize,
) -> Result<String, RequestWorkflowError> {
    let value = required_string(object, field)?;
    let scalar_count = value.trim().chars().count();
    if scalar_count < min || scalar_count > max {
        return Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: format!("/{field}"),
        });
    }
    Ok(value.trim().to_owned())
}

fn required_enum(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
    allowed: &[&str],
) -> Result<String, RequestWorkflowError> {
    let value = required_limited_string(object, field, 1, 80)?;
    if allowed.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: format!("/{field}"),
        })
    }
}

fn required_topic(object: &serde_json::Map<String, Value>) -> Result<String, RequestWorkflowError> {
    let value = required_limited_string(object, "topic", 1, 80)?;
    if value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        Ok(value)
    } else {
        Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: "/topic".to_owned(),
        })
    }
}

fn required_u16(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<u16, RequestWorkflowError> {
    let value = object
        .get(field)
        .ok_or(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::MissingRequiredField,
            field_path: format!("/{field}"),
        })?
        .as_u64()
        .ok_or(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: format!("/{field}"),
        })?;
    u16::try_from(value).map_err(|_| RequestWorkflowError::Rejected {
        reason: IntakeRejectionReason::InvalidField,
        field_path: format!("/{field}"),
    })
}

fn required_duration_minutes(
    object: &serde_json::Map<String, Value>,
) -> Result<u16, RequestWorkflowError> {
    let minutes = required_u16(object, "lesson_duration_minutes")?;
    if (15..=120).contains(&minutes) {
        Ok(minutes)
    } else {
        Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: "/lesson_duration_minutes".to_owned(),
        })
    }
}

fn required_desired_artifacts(
    object: &serde_json::Map<String, Value>,
) -> Result<Vec<String>, RequestWorkflowError> {
    let values = required_string_array(object, "desired_artifacts")?;
    if values.is_empty() || values.len() > 8 {
        return Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: "/desired_artifacts".to_owned(),
        });
    }
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !matches!(
            value.as_str(),
            "worksheet" | "answer_key" | "python_checker" | "teacher_notes"
        ) || !seen.insert(value.clone())
        {
            return Err(RequestWorkflowError::Rejected {
                reason: IntakeRejectionReason::InvalidField,
                field_path: format!("/desired_artifacts/{index}"),
            });
        }
    }
    let canonical = vec![
        "worksheet".to_owned(),
        "answer_key".to_owned(),
        "python_checker".to_owned(),
        "teacher_notes".to_owned(),
    ];
    Ok(canonical
        .into_iter()
        .filter(|artifact| seen.contains(artifact))
        .collect())
}

fn optional_constraints(
    object: &serde_json::Map<String, Value>,
) -> Result<Vec<String>, RequestWorkflowError> {
    let Some(value) = object.get("constraints") else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or(RequestWorkflowError::Rejected {
        reason: IntakeRejectionReason::InvalidField,
        field_path: "/constraints".to_owned(),
    })?;
    if values.len() > 12 {
        return Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: "/constraints".to_owned(),
        });
    }
    let mut constraints = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let Some(text) = value.as_str() else {
            return Err(RequestWorkflowError::Rejected {
                reason: IntakeRejectionReason::InvalidField,
                field_path: format!("/constraints/{index}"),
            });
        };
        let trimmed = text.trim();
        let len = trimmed.chars().count();
        if len == 0 || len > 240 {
            return Err(RequestWorkflowError::Rejected {
                reason: IntakeRejectionReason::InvalidField,
                field_path: format!("/constraints/{index}"),
            });
        }
        constraints.push(trimmed.to_owned());
    }
    Ok(constraints)
}

fn required_string_array(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, RequestWorkflowError> {
    let values = object
        .get(field)
        .ok_or(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::MissingRequiredField,
            field_path: format!("/{field}"),
        })?
        .as_array()
        .ok_or(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: format!("/{field}"),
        })?;
    if values.is_empty() {
        return Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: format!("/{field}"),
        });
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .filter(|text| !text.trim().is_empty())
                .map(ToOwned::to_owned)
                .ok_or(RequestWorkflowError::Rejected {
                    reason: IntakeRejectionReason::InvalidField,
                    field_path: format!("/{field}/{index}"),
                })
        })
        .collect()
}

fn require_true(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<(), RequestWorkflowError> {
    if object.get(field).and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    Err(RequestWorkflowError::Rejected {
        reason: IntakeRejectionReason::InvalidField,
        field_path: format!("/{field}"),
    })
}

fn optional_visibility(
    object: &serde_json::Map<String, Value>,
    _default: StoredRequestVisibility,
) -> Result<StoredRequestVisibility, RequestWorkflowError> {
    match object.get("visibility") {
        None => Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::MissingRequiredField,
            field_path: "/visibility".to_owned(),
        }),
        Some(Value::String(value)) if value == "public" => Ok(StoredRequestVisibility::Public),
        Some(Value::String(value)) if value == "private" => Ok(StoredRequestVisibility::Private),
        Some(_) => Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: "/visibility".to_owned(),
        }),
    }
}

fn optional_auto_repair_preference(
    object: &serde_json::Map<String, Value>,
    default: AutoRepairPreference,
) -> Result<AutoRepairPreference, RequestWorkflowError> {
    match object.get("auto_repair_preference") {
        None => Ok(default),
        Some(Value::String(value)) if value == "no_automated_repair" => {
            Ok(AutoRepairPreference::NoAutomatedRepair)
        }
        Some(Value::String(value)) if value == "request_bounded_code_repair" => {
            Ok(AutoRepairPreference::RequestBoundedCodeRepair)
        }
        Some(Value::String(_)) => Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::UnsupportedMvpValue,
            field_path: "/auto_repair_preference".to_owned(),
        }),
        Some(_) => Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::InvalidField,
            field_path: "/auto_repair_preference".to_owned(),
        }),
    }
}

fn reject_unsafe_text(value: &str, field_path: &str) -> Result<(), RequestWorkflowError> {
    let lowercase = value.to_ascii_lowercase();
    let looks_like_email = value.contains('@') && value.contains('.');
    let looks_like_url = lowercase.contains("://")
        || lowercase.starts_with("http:")
        || lowercase.starts_with("https:");
    let looks_like_local_path = lowercase.contains("/users/")
        || lowercase.contains("\\users\\")
        || lowercase.contains("/home/")
        || lowercase.contains("\\home\\")
        || lowercase.contains("/etc/")
        || lowercase.contains("/private/")
        || lowercase.contains("/var/")
        || lowercase.contains("/tmp/")
        || lowercase.contains("~/")
        || lowercase.contains("../")
        || lowercase.contains("..\\")
        || contains_windows_absolute_path(&lowercase)
        || lowercase.contains("\\\\");
    let looks_like_secret = contains_secret_key_prefix(&lowercase)
        || lowercase.contains("api key")
        || lowercase.contains("api_key")
        || lowercase.contains("secret")
        || lowercase.contains("provider endpoint")
        || lowercase.contains("provider key");
    // Request text is free-form STEM prose; reject only very long digit runs here to
    // avoid treating ordinary numeric examples as identifiers.
    let looks_like_long_digit_id = has_long_digit_run(value, 15);
    let looks_like_phone = has_phone_like_number(value, &lowercase);
    if looks_like_email
        || looks_like_url
        || looks_like_local_path
        || looks_like_secret
        || looks_like_long_digit_id
        || looks_like_phone
    {
        return Err(RequestWorkflowError::Rejected {
            reason: IntakeRejectionReason::UnsafeText,
            field_path: field_path.to_owned(),
        });
    }
    Ok(())
}

fn contains_secret_key_prefix(lowercase: &str) -> bool {
    ["sk-", "sk_"]
        .iter()
        .any(|prefix| contains_delimited_secret_key_prefix(lowercase, prefix))
}

fn contains_windows_absolute_path(lowercase: &str) -> bool {
    let bytes = lowercase.as_bytes();
    bytes
        .windows(3)
        .any(|window| window[0].is_ascii_alphabetic() && window[1] == b':' && window[2] == b'\\')
}

fn contains_delimited_secret_key_prefix(lowercase: &str, prefix: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative_start) = lowercase[search_start..].find(prefix) {
        let start = search_start.saturating_add(relative_start);
        let end = start.saturating_add(prefix.len());
        let before_is_token = lowercase[..start]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');
        if !before_is_token && secret_key_tail_is_plausible(&lowercase[end..]) {
            return true;
        }
        search_start = end;
    }
    false
}

fn secret_key_tail_is_plausible(tail: &str) -> bool {
    tail.chars()
        .take_while(|character| {
            character.is_ascii_alphanumeric() || *character == '-' || *character == '_'
        })
        .count()
        >= 4
}

fn has_long_digit_run(value: &str, minimum_run: usize) -> bool {
    let mut run = 0;
    for character in value.chars() {
        if character.is_ascii_digit() {
            run += 1;
            if run >= minimum_run {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}
