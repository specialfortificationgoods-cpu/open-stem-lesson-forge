use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdError {
    #[error("id for {entity} must start with prefix {prefix}")]
    InvalidPrefix {
        entity: &'static str,
        prefix: &'static str,
    },
    #[error("id for {entity} must have a non-empty safe suffix")]
    EmptySuffix { entity: &'static str },
    #[error("id for {entity} contains unsafe data")]
    UnsafeContent { entity: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid {entity} transition from {from} using {action}")]
pub struct TransitionError {
    pub entity: &'static str,
    pub from: &'static str,
    pub action: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LeaseError {
    #[error("lease is not active")]
    NotActive,
    #[error("lease actor mismatch")]
    ActorMismatch,
    #[error("lease entity mismatch")]
    EntityMismatch,
    #[error("lease is expired")]
    Expired,
    #[error("lease cannot expire before expires_at")]
    NotExpired,
    #[error("lease action is not allowed from current state")]
    InvalidState,
    #[error("lease submission requires idempotency key, payload digest, and result id")]
    MissingSubmissionRecord,
    #[error("lease claim token mismatch")]
    ClaimTokenMismatch,
    #[error("lease idempotency key was replayed with a different payload")]
    IdempotencyConflict,
}
