# 005. Core Data Model and State Machine Spec

Status: Passed adversarial review  
Roadmap: `docs/specs/000-spec-roadmap.md`  
Depends on:

- `docs/specs/001-mvp-slice-acceptance-tests.md`
- `docs/specs/002-security-privacy-abuse-resistance-checklist.md`
- `docs/specs/003-architecture-stack-no-inference-boundary.md`
- `docs/specs/004-data-classification-redaction-logging-no-leak.md`

## Purpose

This spec defines the durable central-core entities, identifier rules, state machines, transition authority, and provenance links for the Rust MVP.

The central rule is strict: runner and reviewer submissions may provide evidence, claims, or requested next states, but central durable state changes only through deterministic state-transition rules.

In this spec, task-like entities use aggregate availability state. Individual claim lifecycle is represented by `Lease`. A lease may expire, release, or be consumed without making the parent task permanently `expired` or `released`; the parent returns to `open` when retry is allowed.

## Entity Inventory

The MVP central core stores these durable entity types:

- `Request`
- `RequestModerationTask`
- `RequestModerationReport`
- `PlanningTask`
- `ProposedTaskGraph`
- `PlanVerificationTask`
- `PlanVerification`
- `PromotionDecision`
- `WorkPacket`
- `Lease`
- `Artifact`
- `ValidationReport`
- `ReviewTask`
- `Review`
- `Finding`
- `RunnerCapabilitySummary`
- `Actor`
- `StateTransitionEvent`
- `PublicArtifactLabel`

## Identifier Rules

Identifiers are opaque strings with type-specific prefixes:

| Entity | Prefix example |
|---|---|
| Request | `req_` |
| RequestModerationTask | `rmtask_` |
| RequestModerationReport | `rmreport_` |
| PlanningTask | `ptask_` |
| ProposedTaskGraph | `plan_` |
| PlanVerificationTask | `pvtask_` |
| PlanVerification | `pverify_` |
| PromotionDecision | `promo_` |
| WorkPacket | `wp_` |
| Lease | `lease_` |
| Artifact | `art_` |
| ValidationReport | `vreport_` |
| ReviewTask | `rtask_` |
| Review | `review_` |
| Finding | `finding_` |
| Actor | `actor_` |
| StateTransitionEvent | `event_` |
| PublicArtifactLabel | `plabel_` |

Rules:

- IDs are immutable after creation.
- IDs are not authorization tokens.
- IDs must not encode secrets, local paths, provider details, emails, or student data.
- Fixtures may use deterministic human-readable IDs.
- Production IDs may use UUID/ULID-style random IDs selected during implementation.

## Shared Metadata

Every durable entity has:

- `id`
- `scope_id`
- `schema_version`
- `created_at`
- `updated_at`
- `created_by_actor_id` where applicable
- `updated_by_actor_id` where applicable
- `state`

Every state-changing entity records transition history through `StateTransitionEvent`; entities do not need to embed full history.

`scope_id` is a deterministic authorization and partition key. The MVP may use a single default scope, but every actor, request, task, proposal, artifact, review, and event must still carry the field so later multi-tenant or classroom/project partitioning does not require changing authority rules.

## Wire-to-Core Mapping

External fixture and JSON-schema field names are allowed to be domain-specific. The core stores normalized actor and state names.

| Wire/API field or value | Core field or value |
|---|---|
| `request.state=requested` | `Request.state=requested` |
| `planner_runner_id` | `ProposedTaskGraph.planner_actor_id` |
| `verifier_runner_id` | `PlanVerification.verifier_actor_id` |
| `reviewer_id` | `Review.reviewer_actor_id`, bound from authenticated review context |
| Submitted `status`, `outcome`, `recommended_next_state`, `status_claim` | Evidence or inert claim only; never direct central state |

Schema fixtures and Rust persisted types must test this mapping so accepted JSON examples cannot drift from central domain types.

## Actor Model

Actor types:

- `public_requester`
- `authenticated_requester`
- `runner`
- `verifier_runner`
- `human_reviewer`
- `runner_operator`
- `curator`
- `admin`
- `system_core`
- `system_validator`

Actor records contain:

- `actor_id`
- `scope_id`
- `actor_type`
- `display_name_public` optional and redacted.
- `trust_level`
- `capabilities`
- `status`
- `owned_runner_actor_ids` optional for `runner_operator`.

Actor records must not contain provider credentials, full runner config, local auth paths, cookies, or exact private quotas.

## Capability and Trust

Capability and trust are separate.

Capability answers what an actor claims or is configured to do:

- `request_interpretation`
- `content_moderation`
- `age_appropriateness_classification`
- `task_decomposition`
- `plan_consistency_review`
- `artifact_generation`
- `artifact_validation`
- `human_subject_review`
- `human_pedagogy_review`

Trust answers what central policy permits actor output to influence:

- `untrusted`
- `runner_candidate`
- `moderation_candidate`
- `planner_candidate`
- `verifier_candidate`
- `reviewer_candidate`
- `reviewer_approved`
- `curator`
- `admin`
- `system`

A capability never grants trust by itself. A trust level never implies provider capability by itself.

## Request

Purpose: public or contributor-submitted educational request.

Core fields:

- `request_id`
- `title`
- `subject`
- `topic`
- `age_range`
- `language`
- `lesson_duration_minutes`
- `desired_artifacts`
- `constraints`
- `license_preference`
- `visibility`
- `auto_repair_preference`
- `forbidden_content_acknowledged`
- `state`

States:

- `requested`
- `moderation_pending`
- `moderation_passed`
- `rejected`
- `quarantined`
- `planning_open`
- `planning_in_progress`
- `planning_failed`
- `plan_proposed`
- `decomposed`
- `artifact_drafted`
- `machine_validated`
- `peer_reviewed`
- `deprecated`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `requested` | requester | Required fields present. |
| `requested` | `rejected` | system_core | Deterministic intake rejection. |
| `requested` | `quarantined` | system_core or curator | Deterministic abuse/safety trigger. |
| `requested` | `moderation_pending` | system_core | Deterministic intake passes and request moderation task created. |
| `moderation_pending` | `rejected` | system_core | Accepted moderation report denies the request. |
| `moderation_pending` | `quarantined` | system_core | Accepted moderation report or deterministic trigger requires quarantine. |
| `moderation_pending` | `moderation_passed` | system_core | Accepted moderation report allows MVP planning. |
| `moderation_passed` | `planning_open` | system_core | Planning task created after moderation gate. |
| `planning_open` | `planning_in_progress` | system_core | Planning task lease is active. |
| `planning_open` or `planning_in_progress` | `planning_failed` | system_core | Planning task exhausted deterministic retry policy without an accepted proposal; safe reason code required. |
| `planning_in_progress` | `plan_proposed` | system_core | At least one proposal stored. |
| `plan_proposed` | `decomposed` | system_core | Promotion decision accepted and work packets materialized. |
| `decomposed` | `artifact_drafted` | system_core | Selected MVP artifact bundle draft accepted. |
| `artifact_drafted` | `machine_validated` | system_core | Selected MVP artifact bundle passed trusted deterministic validation. |
| `machine_validated` | `peer_reviewed` | system_core | Selected MVP artifact bundle passed required human approval gates. |
| any non-terminal including `planning_failed` | `deprecated` | curator or admin | Safe reason code provided. |

Request state is an aggregate summary. It does not replace proposal, work packet, artifact, or review state.

For the MVP, a request has one selected promoted proposal and one selected artifact bundle containing the worksheet, answer key, Python checker, and teacher notes. `artifact_drafted`, `machine_validated`, and `peer_reviewed` mean that selected bundle reached the corresponding artifact state. After the MVP, aggregate request states must be defined as deterministic reductions over all required artifacts and review gates before they can be generalized.

## RequestModerationTask

Purpose: leaseable task asking a moderation-capable runner or non-core moderation service to classify an accepted request before planning begins.

Core fields:

- `request_moderation_task_id`
- `request_id`
- `scope_id`
- `required_output_schema=request_moderation_report.schema.json`
- `minimum_runner_trust_level=moderation_candidate`
- `state`

States:

- `open`
- `claimed`
- `completed`
- `cancelled`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `open` | system_core | Request deterministic intake passed. |
| `open` | `claimed` | system_core | Eligible moderation actor receives active lease. |
| `claimed` | `completed` | system_core | Accepted moderation report stored and request state reduced. |
| `open` or `claimed` | `cancelled` | system_core or curator/admin | Request rejected, quarantined, or deprecated. |

Moderation task state does not authorize planning by itself. Only an accepted `RequestModerationReport` with `decision=allow_mvp_planning` can move the request to `moderation_passed`.

## RequestModerationReport

Purpose: safe moderation evidence for request intake.

Core fields:

- `request_moderation_report_id`
- `request_moderation_task_id`
- `request_id`
- `moderator_actor_id`
- `moderation_kind`
- `decision`
- `category_flags`
- `safe_reason_codes`
- `state`

Allowed values:

- `moderation_kind`: `dummy_fixture`, `provider_backed`, `human_curator_fixture`
- `decision`: `allow_mvp_planning`, `reject_request`, `quarantine_request`
- `category_flags`: closed enum set from the moderation schema, not raw provider labels.
- `safe_reason_codes`: closed safe-code enum only.

Moderation category enum:

- `none`
- `sexual`
- `sexual_minors`
- `violence`
- `self_harm`
- `hate`
- `harassment`
- `illicit`
- `weapons`
- `privacy`
- `age_inappropriate`

Moderation safe reason enum:

- `moderation_allowed`
- `moderation_rejected_sexual`
- `moderation_rejected_sexual_minors`
- `moderation_rejected_violence`
- `moderation_rejected_self_harm`
- `moderation_rejected_hate_or_harassment`
- `moderation_rejected_illicit`
- `moderation_rejected_weapons`
- `moderation_rejected_privacy`
- `moderation_rejected_age_inappropriate`
- `moderation_quarantine_review_needed`
- `moderation_schema_invalid`
- `moderation_lineage_mismatch`
- `moderation_stale_lease`
- `moderation_deterministic_heuristic_override`

States:

- `submitted`
- `accepted`
- `rejected`

Rules:

- Reports are lease-bound to the request moderation task.
- Body-submitted actor IDs and request IDs are checked against authenticated context and lease lineage.
- Raw provider responses, prompts, provider account data, local paths, and secrets are forbidden.
- `category_flags=["none"]` is valid only with `decision=allow_mvp_planning`.
- `none` is mutually exclusive with every other category.
- `allow_mvp_planning` requires `category_flags=["none"]` and `safe_reason_codes=["moderation_allowed"]`.
- `reject_request` or `quarantine_request` requires at least one non-`none` category and matching safe reason code.
- Free-form or provider-derived reason text is rejected.
- A report can move the request only through deterministic transition guards.

## PlanningTask

Purpose: mechanical task asking eligible planners to produce a proposed graph.

Core fields:

- `planning_task_id`
- `request_id`
- `task_type=propose_task_graph`
- `required_output_schema`
- `required_capabilities`
- `minimum_runner_trust_level`
- `claim_policy`
- `state`

States:

- `open`
- `claimed`
- `submitted`
- `completed`
- `cancelled`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `open` | system_core | Request moderation passed. |
| `open` | `claimed` | runner via system_core | Eligible actor and lease created. |
| `claimed` | `submitted` | lease holder | Proposal submitted before lease expiry. |
| `submitted` | `completed` | system_core | Proposal persisted and task terminal for this claim. |
| `claimed` | `open` | system_core | Active lease expired and retry is allowed. |
| `claimed` | `open` | lease holder via system_core | Lease released and retry is allowed. |
| `open` or `claimed` | `cancelled` | curator/admin/system_core | Request rejected/quarantined/deprecated, or retry limit exhausted with request transition to `planning_failed`. |

Planning task state never means the proposal is valid or promoted.

Only one active lease may exist for a planning task at a time unless a later scheduling spec explicitly defines multi-claim fanout. Concurrent claim attempts must resolve through an atomic compare-and-set on task state plus lease creation.

## ProposedTaskGraph

Purpose: runner-proposed decomposition of a request into work packets.

Core fields:

- `proposal_id`
- `request_id`
- `planning_task_id`
- `planner_actor_id`
- `schema_version`
- `source_request_summary`
- `assumptions`
- `missing_information`
- `proposed_artifacts`
- `proposed_tasks`
- `validation_plan`
- `human_review_required_for`
- `state`

States:

- `proposed`
- `schema_rejected`
- `policy_rejected`
- `schema_policy_validated`
- `verification_required`
- `verification_blocked`
- `verified_for_mvp_promotion`
- `promotion_rejected`
- `promoted`
- `superseded`
- `quarantined`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `proposed` | planner runner via system_core | Active planning lease and payload accepted for validation. |
| `proposed` | `schema_rejected` | system_core | Schema validation fails. |
| `proposed` | `policy_rejected` | system_core | Policy validation fails. |
| `proposed` | `schema_policy_validated` | system_core | Schema and policy pass. |
| `schema_policy_validated` | `verification_required` | system_core | Verification task created. |
| `verification_required` | `verification_blocked` | system_core | Accepted verification reports blocking findings. |
| `verification_required` | `verified_for_mvp_promotion` | system_core | Accepted independent verification has no blocking findings. |
| `verified_for_mvp_promotion` | `promotion_rejected` | system_core | Promotion guard fails. |
| `verified_for_mvp_promotion` | `promoted` | system_core | Promotion decision accepted and work packets materialized idempotently. |
| any non-terminal | `superseded` | curator/system_core | Better accepted plan selected. |
| any non-terminal | `quarantined` | curator/system_core | Abuse/safety trigger. |

Runner-submitted `status` fields are non-authoritative claims.

## PlanVerificationTask

Purpose: deterministic-core-created task asking an independent verifier to check a proposed graph.

Core fields:

- `plan_verification_task_id`
- `proposal_id`
- `request_id`
- `task_type=verify_proposed_task_graph`
- `required_output_schema`
- `required_capabilities`
- `minimum_runner_trust_level`
- `claim_policy`
- `state`

States:

- `open`
- `claimed`
- `submitted`
- `completed`
- `cancelled`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `open` | system_core | Proposal is `schema_policy_validated` and verification is required. |
| `open` | `claimed` | verifier runner via system_core | Eligible actor, actor differs from `planner_actor_id`, same scope, and lease created. |
| `claimed` | `submitted` | lease holder | Verification submitted before lease expiry for the matching proposal and task. |
| `submitted` | `completed` | system_core | Verification evidence persisted and task terminal for this claim. |
| `claimed` | `open` | system_core | Active lease expired and retry is allowed. |
| `claimed` | `open` | lease holder via system_core | Lease released and retry is allowed. |
| `open` or `claimed` | `cancelled` | curator/admin/system_core | Parent request/proposal rejected, quarantined, promoted, superseded, or deprecated. |

- It can be created only after proposal schema/policy validation passes.
- Claiming actor must differ from `planner_actor_id` for the proposal in the MVP.
- Submitted verification must reference the task, proposal, and active lease.
- Only one active verification lease may exist for a verification task at a time unless a later scheduling spec explicitly defines multi-review fanout.

## PlanVerification

Purpose: advisory verifier evidence for a proposed graph.

Core fields:

- `verification_id`
- `proposal_id`
- `plan_verification_task_id`
- `verifier_actor_id`
- `outcome`
- `findings`
- `checked_items`
- `authority=advisory_only`
- `state`

States:

- `submitted`
- `accepted_as_evidence`
- `rejected`
- `blocking`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `submitted` | verifier runner via system_core | Active verification lease. |
| `submitted` | `accepted_as_evidence` | system_core | Valid report, independent verifier, no blocking findings. |
| `submitted` | `blocking` | system_core | Valid report with blocking/critical/major findings. |
| `submitted` | `rejected` | system_core | Invalid authority, stale lease, wrong proposal, self-verification, or schema failure. |

Plan verification never counts as human review.

## PromotionDecision

Purpose: immutable record of a promotion attempt for a proposed graph.

Core fields:

- `promotion_decision_id`
- `proposal_id`
- `request_id`
- `decision`
- `reason_codes`
- `idempotency_key`
- `created_work_packet_ids`
- `state`

States:

- `accepted`
- `denied`
- `no_op_existing`

Rules:

- Promotion is transactional.
- Promotion is idempotent by `(proposal_id, source_local_task_id)` and idempotency key where supplied.
- The persisted store must enforce uniqueness of `(proposal_id, source_local_task_id)` for work packets.
- If promotion partially fails, the transaction must roll back so no orphaned or duplicate work packets remain.
- Accepted promotion creates work packets that snapshot proposal task data and preserve `proposal_id` plus `source_local_task_id`.
- Denied promotion creates no work packets.

## WorkPacket

Purpose: claimable unit of work materialized from a promoted proposal.

Core fields:

- `work_packet_id`
- `request_id`
- `proposal_id`
- `source_local_task_id`
- `phase`
- `task_type`
- `dependencies`
- `required_capabilities`
- `risk_level`
- `outputs`
- `validation_required`
- `human_review_required_for`
- `claimed_by_actor_id` optional.
- `submitted_by_actor_id` optional.
- `accepted_output_actor_id` optional.
- `state`

States:

- `blocked_by_dependency`
- `open`
- `claimed`
- `submitted`
- `accepted`
- `rejected`
- `interrupted`
- `cancelled`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `blocked_by_dependency` or `open` | system_core | Promotion materializes packet. |
| `blocked_by_dependency` | `open` | system_core | Dependencies satisfied. |
| `open` | `claimed` | runner/reviewer via system_core | Eligible actor and lease created. |
| `claimed` | `submitted` | lease holder | Output submitted before lease expiry. |
| `submitted` | `accepted` | system_core | Output accepted by schema/policy/state rules. |
| `submitted` | `rejected` | system_core | Output rejected by schema/policy/state rules. |
| `submitted` | `interrupted` | system_core | Spec `014` repair interruption report accepted and lease consumed. |
| `claimed` | `open` | system_core | Active lease expired and retry is allowed. |
| `claimed` | `open` | lease holder via system_core | Lease released and retry is allowed. |
| any non-terminal | `cancelled` | curator/system_core | Parent request/proposal/artifact cancelled or quarantined. |

Only one active lease may exist for a work packet at a time unless a later scheduling spec explicitly defines parallel roles for that packet.

For MVP human review, the promoted proposal materializes a `WorkPacket` with `phase=human_review`; that work packet remains `blocked_by_dependency` until the selected artifact bundle is `machine_validated`. `ReviewTask` is the claim/review projection opened from that human-review work packet, not a separate planning surface.

## Lease

Purpose: binds a claimable entity to an actor for a time-limited submission window.

Core fields:

- `lease_id`
- `entity_type`
- `entity_id`
- `actor_id`
- `lease_slot` optional for explicitly fanned-out claim surfaces.
- `claim_token_hash`
- `idempotency_key` optional.
- `submission_result_id` optional.
- `claimed_at`
- `expires_at`
- `released_at`
- `consumed_at`
- `state`

States:

- `active`
- `expired`
- `released`
- `consumed`
- `revoked`

Rules:

- Claim tokens are secret and stored only as safe hashes or equivalent non-recoverable verification material.
- Lease tokens never appear in logs, errors, audit events, or public output.
- Submissions require active lease, matching actor, matching entity, and unexpired time.
- Successful lease-bound state-changing submissions atomically transition the lease from `active` to `consumed`.
- `consumed`, `released`, `expired`, `revoked`, and replaced leases cannot submit state-changing output.
- Lease creation, claimable-entity state change, and transition-event creation are one atomic operation.
- Lease consumption and submitted-output persistence are one atomic operation.
- Replaying the same consumed lease with the same idempotency key and identical payload returns the original result without creating a new record or transition event.
- Replaying the same consumed lease with a different payload or different idempotency key is rejected with a safe replay/conflict reason code.

## Artifact

Purpose: central metadata record for an artifact bundle.

Core fields:

- `artifact_id`
- `request_id`
- `proposal_id`
- `work_packet_ids`
- `generated_by_actor_ids`
- `repair_root_artifact_id` optional.
- `repair_parent_artifact_id` optional.
- `repair_attempt_id` optional.
- `repair_attempt_index` optional.
- `repair_continuation_index` optional.
- `selected_artifact_bundle=true`
- `manifest_summary`
- `central_status`
- `validation_status`
- `review_status`
- `public_label`

States:

- `draft_generated`
- `validation_failed`
- `machine_validated`
- `review_requested`
- `peer_reviewed`
- `quarantined`
- `deprecated`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `draft_generated` | system_core | Generation output accepted. |
| `draft_generated` | `validation_failed` | system_validator via system_core | Trusted deterministic validation failed. |
| `draft_generated` | `machine_validated` | system_validator via system_core | Trusted deterministic validation passed. |
| `machine_validated` | `review_requested` | system_core | Review task opened. |
| `machine_validated` or `review_requested` | `peer_reviewed` | system_core | All required human review gates have accepted valid records with `approved_for_peer_reviewed`, no disqualifying conflicts, and no open blocking findings. |
| any non-terminal | `quarantined` | curator/system_core | Abuse/safety trigger. |
| any non-terminal | `deprecated` | curator/admin | Superseded or unsafe reason code. |

Artifact manifest `status_claim` is never authoritative.

## ValidationReport

Purpose: trusted deterministic validator result or untrusted runner evidence if later allowed.

Core fields:

- `validation_report_id`
- `artifact_id`
- `validator_actor_id`
- `validator_kind`
- `status`
- `checks`
- `failures`
- `state`

States:

- `trusted_passed`
- `trusted_failed`
- `trusted_incomplete_static_only`
- `untrusted_evidence`
- `rejected`

Rules:

- Only `validator_kind=deterministic_core_validator` through the trusted validator path can move artifact state.
- Runner-submitted validation success is non-authoritative.
- Raw unsafe stdout/stderr is not persisted.

## ReviewTask

Purpose: claimable human review work.

Core fields:

- `review_task_id`
- `review_work_packet_id`
- `artifact_id`
- `request_id`
- `review_types`
- `required_reviewer_trust_level`
- `state`

States:

- `open`
- `claimed`
- `submitted`
- `completed`
- `cancelled`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `open` | system_core | Artifact is eligible for human review and required review type is known. |
| `open` | `claimed` | human reviewer via system_core | Eligible reviewer, same scope, no disqualifying conflict, and lease created. |
| `claimed` | `submitted` | lease holder | Review submitted before lease expiry for the matching artifact and task. |
| `submitted` | `completed` | system_core | Review record persisted and task terminal for this claim. |
| `claimed` | `open` | system_core | Active lease expired and retry is allowed. |
| `claimed` | `open` | lease holder via system_core | Lease released and retry is allowed. |
| `open` or `claimed` | `cancelled` | curator/admin/system_core | Artifact/request quarantined, deprecated, or review requirement removed. |

Only one active lease may exist for a review task at a time unless a later review spec explicitly defines parallel review fanout.

`ReviewTask.review_work_packet_id` references the promoted `WorkPacket` with `phase=human_review`. The review task inherits request, proposal, artifact, required review type, and scope from central parent records. Submitted IDs cannot change that lineage.

## Code Critique and Repair Evidence

Spec `014` adds runner-produced code critique, repair, and repair-interruption evidence.

Core entities:

- `CodeCritiqueReport`
- `CodeRepairReport`
- `CodeRepairInterruptionReport`
- `RepairAttempt`
- `RepairContinuationDecision`

States:

- `submitted`
- `accepted_advisory`
- `rejected`
- `superseded`

Rules:

- These records are internal evidence only.
- They cannot move an artifact to `machine_validated`, `review_requested`, `peer_reviewed`, or any public label.
- Repair output creates a new `Artifact` in `draft_generated`; it never mutates the source artifact in place.
- Interruption output creates no artifact and routes only to bounded `runner_operator`, curator, admin review, or continuation rules from spec `014`.
- Submitted lineage fields are evidence only and must match server-derived request, proposal, artifact, work packet, lease, runner actor, and repair attempt context.
- Automated repair-loop metadata is central-derived and is not an artifact state.
- Repair lineage fields on repaired artifacts are server-derived: `repair_root_artifact_id`, `repair_parent_artifact_id`, `repair_attempt_id`, `repair_attempt_index`, and `repair_continuation_index`.
- `RepairAttempt` uniqueness prevents duplicate repair work packets or repaired artifacts for the same logical attempt.

`RepairAttempt` state transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `open` | system_core | Spec `014` deterministic repair routing creates attempt and repair work packet transactionally. |
| `open` | `claimed` | system_core | Repair work packet claimed. |
| `continued` | `claimed` | system_core | Continuation repair work packet claimed for the same logical attempt with incremented continuation index. |
| `claimed` | `open` | system_core | Repair work packet lease expired, was released, or was revoked before output; attempt remains retryable within same continuation index. |
| `claimed` | `interrupted` | system_core | Signed interruption report accepted and repair work packet moves to `interrupted`. |
| `interrupted` | `continued` | runner_operator/curator/admin via system_core | Spec `014` continuation decision accepted. |
| `claimed` or `continued` | `artifact_created` | system_core | Accepted repair output creates the preallocated target artifact. |
| `interrupted` or `continued` | `stopped` | runner_operator/curator/admin/system_core | Stop/quarantine/limit outcome. |
| any non-terminal | `cancelled` | curator/admin/system_core | Source request/artifact quarantined, deprecated, or cancelled. |

Only `artifact_created`, `stopped`, and `cancelled` are terminal. Terminal attempts cannot create continuation work packets.

## Review

Purpose: authoritative human review record after validation of actor authority and active claim.

Core fields:

- `review_id`
- `artifact_id`
- `review_task_id`
- `review_work_packet_id`
- `reviewer_actor_id`
- `review_types`
- `outcome`
- `findings`
- `recommended_next_state`
- `state`

States:

- `submitted`
- `accepted`
- `rejected`
- `superseded`

Allowed transitions:

| From | To | Actor | Guard |
|---|---|---|---|
| none | `submitted` | human reviewer via system_core | Active review lease and role eligibility. |
| `submitted` | `accepted` | system_core | Schema/policy pass and no disqualifying conflicts. |
| `submitted` | `rejected` | system_core | Invalid authority, stale lease, schema/policy failure, or conflict. |
| `accepted` | `superseded` | curator/system_core | Later review or deprecation supersedes it. |

Submitted `reviewer_id`, `outcome`, and `recommended_next_state` are not authoritative without authenticated review context and central transition rules.

`Review.state=accepted` means the review record is valid evidence, not that the artifact is approved. The MVP approval outcome that can contribute to artifact `peer_reviewed` is `approved_for_peer_reviewed`. Valid accepted reviews with `changes_requested`, `rejected_for_use`, `needs_subject_matter_fix`, or open blocking findings do not raise artifact state.

Minimum MVP disqualifying conflicts:

- The reviewer must not be the artifact-generating actor.
- The reviewer must not be the planner actor for the promoted proposal.
- The reviewer must not be the verifier actor whose plan verification enabled promotion.
- The reviewer must not be the same authenticated actor as any accepted output actor for the artifact's source work packets, except the human-review work packet itself.

## Finding

Purpose: structured issue attached to verification, validation, or review.

Core fields:

- `finding_id`
- `parent_type`
- `parent_id`
- `severity`
- `finding_type`
- `safe_location`
- `safe_message`
- `state`
- `blocking`

States:

- `open`
- `resolved`
- `waived`
- `duplicate`
- `superseded`

Rules:

- `critical` and `major` findings are blocking by default unless a later review spec defines a waiver rule.
- Free-text finding messages are inert escaped text.
- Findings never directly transition artifact/request/proposal state without a deterministic rule consuming them.

## RunnerCapabilitySummary

Purpose: redacted central view of runner capabilities.

Core fields:

- `runner_actor_id`
- `capability_schema_version`
- `capabilities`
- `trust_level`
- `policy_summary`
- `state`

States:

- `active`
- `paused`
- `revoked`

Rules:

- Full provider config remains local.
- Capability summaries do not contain credentials, auth paths, provider base URLs, local prompt templates, or exact private quotas.

## StateTransitionEvent

Purpose: append-only safe audit record for state changes.

Core fields:

- `event_id`
- `scope_id`
- `actor_type`
- `actor_id`
- `action`
- `entity_type`
- `entity_id`
- `previous_state`
- `next_state`
- `safe_reason_code`
- `safe_field_path`
- `created_at`
- Related IDs such as `request_id`, `proposal_id`, or `artifact_id` when safe.

Rules:

- Events never contain raw rejected payloads, secrets, local paths, cookies, tokens, provider config, prompt templates, or student PII.
- Every accepted state transition creates an event.
- Rejected transition attempts create safe denial events where useful.

## PublicArtifactLabel

Purpose: conservative public label derived from central state.

Labels:

- `draft_generated`
- `machine_validated`
- `peer_reviewed`
- `quarantined`
- `deprecated`

Rules:

- Public label is derived from artifact central state, validation report state, and review state.
- Manifest claims cannot raise public label.
- `classroom_ready` is not an MVP label.

## Lineage Rules

Central lineage is server-derived from the active lease and persisted parent entities.

- Submitted `request_id`, `proposal_id`, `planning_task_id`, `plan_verification_task_id`, `work_packet_id`, `artifact_id`, `review_task_id`, and `review_work_packet_id` are claims to be checked, not authority.
- A request moderation report derives `request_id`, `scope_id`, and moderator actor from the claimed `RequestModerationTask` and active lease.
- A planning submission derives `request_id`, `scope_id`, and planner actor from the claimed `PlanningTask` and active lease.
- A plan verification submission derives `proposal_id`, `request_id`, `scope_id`, and verifier actor from the claimed `PlanVerificationTask` and active lease.
- A work-packet output derives `request_id`, `proposal_id`, `source_local_task_id`, `scope_id`, and submitting actor from the claimed `WorkPacket` and active lease.
- An artifact record derives its request, proposal, work packets, scope, and generating actors from accepted work-packet outputs selected by central rules.
- A validation report derives its artifact, request, proposal, scope, and validator actor from the trusted validator invocation context, not from runner-submitted manifest fields.
- A review derives its artifact, review task, human-review work packet, request, proposal, scope, and reviewer actor from the claimed `ReviewTask` and active lease.
- Any cross-scope or cross-parent mismatch is rejected with a safe lineage-mismatch reason code and cannot create or update child records.

## Idempotency and Uniqueness

Every mutating command accepts an idempotency key or uses a deterministic natural key. Replayed successful commands with the same key and identical canonical payload return the original result without emitting duplicate transition events. The same key with a different payload is rejected.

Required uniqueness constraints:

| Surface | Unique key |
|---|---|
| Active lease on single-claim surfaces | `(entity_type, entity_id)` where `state=active` |
| Active lease on explicitly fanned-out planning surfaces | `(entity_type, entity_id, lease_slot)` where `state=active`, with bounded slots defined by the scheduling spec |
| Claim with idempotency key | `(entity_type, entity_id, actor_id, idempotency_key)` |
| Request moderation report submission | `(request_moderation_task_id, lease_id)` |
| Planning proposal submission | `(planning_task_id, lease_id)` |
| Plan verification submission | `(plan_verification_task_id, lease_id)` |
| Promotion work packet | `(proposal_id, source_local_task_id)` |
| Work-packet output submission | `(work_packet_id, lease_id)` |
| Artifact bundle for MVP selected proposal | `(proposal_id, selected_artifact_bundle=true)` |
| Repair attempt | `(repair_root_artifact_id, repair_parent_artifact_id, repair_attempt_index)` |
| Repair continuation | `(repair_attempt_id, repair_continuation_index)` |
| Repaired artifact for attempt | `(repair_attempt_id, target_artifact_id)` |
| Trusted validation report | `(artifact_id, validator_kind, schema_version)` for the validation generation being recorded |
| Review submission | `(review_task_id, lease_id)` |
| Public artifact label | `(artifact_id)` |
| State transition event | `(entity_type, entity_id, previous_state, next_state, command_id)` |

Release, expiry, quarantine, deprecation, validation, and review acceptance commands must use the same replay rule: identical replay returns original result; changed replay under the same idempotency key is rejected; no replay creates duplicate evidence, duplicate output, or duplicate transition events.

## Cross-Entity Authority Rules

- Request aggregate state summarizes downstream state but does not authorize downstream transitions.
- Proposed task graph state does not imply work packets exist until a promotion decision is accepted.
- Work packet state does not imply artifact state until output is accepted.
- Artifact `machine_validated` requires a trusted deterministic validation report.
- Artifact `peer_reviewed` requires valid accepted human-review records with approval outcomes and no open blocking findings.
- Plan verification never counts as human review.
- Runner capability never grants reviewer authority.
- Reviewer authority never grants runner/provider capability.
- Lease state gates submission authority; task state alone is never enough to submit.

## Rust Type Requirements

Implementation should use:

- Newtype IDs per entity.
- Enums for states and actor types.
- Typed transition functions returning `Result<TransitionApplied, TransitionError>`.
- Separate submitted payload types from persisted record types.
- Separate public response types from internal records.
- `thiserror` error enums for domain errors.
- No panics for invalid external input.

## Acceptance Tests

### SM-001: Valid MVP happy path reaches peer reviewed

Run the accepted `001` happy path.

Expected:

- Every entity reaches the expected state.
- Every state change emits a safe `StateTransitionEvent`.
- Public artifact label is `peer_reviewed`, not `classroom_ready`.

### SM-002: Proposal validation does not materialize work packets

Given a proposal in `schema_policy_validated`, before verification and promotion:

- No work packets exist.
- Request aggregate state is not `decomposed`.

### SM-003: Verification without authority is rejected

Submit verification without active lease, for wrong proposal, after lease expiry, or from planner actor.

Expected:

- Verification state is `rejected`.
- Proposal remains non-promotable.
- No work packets materialize.

### SM-004: Blocking verification blocks promotion

Submit accepted verification with a blocking finding.

Expected:

- Verification state is `blocking`.
- Proposal state is `verification_blocked`.
- Promotion is denied.

### SM-005: Promotion is idempotent

Request promotion twice for the same verified proposal.

Expected:

- First request creates work packets.
- Second request returns `no_op_existing` or same accepted decision.
- No duplicate work packets exist for `(proposal_id, source_local_task_id)`.

Submitting the same idempotency key with a different proposal snapshot or task mapping is rejected and emits no duplicate events.

### SM-006: Runner-submitted status claims are non-authoritative

Submit elevated `status`, `outcome`, `recommended_next_state`, or manifest `status_claim` fields.

Expected:

- Central states do not change unless deterministic transition guards pass.
- Public labels reflect central state only.

### SM-007: Forged validation cannot machine validate

Submit runner-controlled passing validation report outside trusted validator path.

Expected:

- Validation report is rejected or untrusted evidence.
- Artifact remains `draft_generated`.

### SM-008: Review laundering is rejected

Submit review with forged `reviewer_id` or without matching active review claim.

Expected:

- Review is rejected.
- Artifact does not become `peer_reviewed`.

### SM-009: Lease expiry blocks stale submission

Submit planning, verification, generation, or review output after lease expiry.

Expected:

- Submission is rejected.
- Entity state does not advance.
- Safe audit event records denial.

### SM-010: Lease replay cannot duplicate submissions

Submit a valid proposal, plan verification, work-packet output, and review. Then replay the same still-unexpired lease token after each successful submission.

Expected:

- Each successful submission consumes the lease atomically.
- Identical replay with the same idempotency key returns the original result.
- Replay with changed payload or changed idempotency key is rejected.
- No duplicate proposals, verifications, artifacts, reviews, outputs, or transition events exist.

### SM-011: Cross-lineage submissions are rejected

Submit otherwise valid payloads that reference a different request, proposal, work packet, artifact, validation report target, review task, or scope than the active lease/task parent chain.

Expected:

- Submission is rejected with safe lineage-mismatch reason code.
- Child records are not created or relinked.
- Central parentage remains server-derived.

### SM-012: Human review validity is not approval

Submit a valid human review record with `changes_requested`, `rejected_for_use`, or an open blocking finding.

Expected:

- Review record may be `accepted` as valid evidence.
- Artifact remains `machine_validated` or `review_requested`.
- Public artifact label does not become `peer_reviewed`.

### SM-013: Review conflict laundering is rejected

Try to peer-review an artifact using the same actor that planned, verified, generated, or accepted source work-packet output for that artifact.

Expected:

- Review is rejected or accepted only as non-approving evidence according to policy.
- Artifact does not become `peer_reviewed`.
- Safe audit event records conflict reason code.

### SM-014: Unsafe fields never appear in transition events

Trigger rejected payloads containing fake secrets, local paths, prompt fields, and student PII.

Expected:

- StateTransitionEvent contains only safe reason codes and schema-derived safe field paths.

## Security Checklist Coverage

Applicable categories from `002`:

- Central-Core Boundary Checks: states and transitions are deterministic and do not require inference.
- Data Classification Checks: entity fields and audit events use `004` allowlists.
- Request Intake and Prompt-Injection Checks: request text is untrusted; intake workflow belongs to `006`.
- Identity, Authorization, and Replay Checks: leases and actor-bound submissions are modeled here; endpoint auth belongs to `008`.
- Runner Submission Trust Checks: runner submissions are non-authoritative until deterministic transition guards accept them.
- Plan Verification Gate Checks: modeled through `PlanVerificationTask` and `PlanVerification`.
- Planning and Promotion Abuse Checks: proposal and promotion states prevent implicit materialization.
- Artifact and Generated-Code Checks: artifact and validation report authority are modeled; validator details belong to `010`.
- Human Review and Publication Checks: review authority and public labels are modeled; detailed review policy belongs to `011`.
- Logging, Error, and Audit Checks: safe `StateTransitionEvent` is modeled.

No applicable non-deferrable checklist item is deferred for core data model or state-machine surfaces.

## Review Checklist

Reviewers should fail this spec if:

- A submitted status field can directly set central state.
- Proposal, planning task, selected plan, and work packet states can be confused.
- Promotion can create duplicate work packets.
- Any mutating command can replay into duplicate durable records or duplicate transition events.
- Submitted child IDs can override server-derived lineage from the active lease.
- Artifact status can be raised by manifest claims.
- Machine validation can be forged by runner evidence.
- Peer review can be forged by submitted reviewer IDs.
- A valid review record with non-approval outcome can raise an artifact to `peer_reviewed`.
- Review conflict rules cannot identify planner, verifier, generator, and reviewer overlap.
- Plan verification can count as human review.
- Actor capability and trust are collapsed.
- Audit events can contain raw rejected payloads or secret-like values.
