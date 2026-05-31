# 011. Critique, Human Review, Publication, and Promotion Spec

Status: Passed adversarial review
Roadmap: `docs/specs/000-spec-roadmap.md`
Depends on:

- `docs/specs/001-mvp-slice-acceptance-tests.md`
- `docs/specs/002-security-privacy-abuse-resistance-checklist.md`
- `docs/specs/003-architecture-stack-no-inference-boundary.md`
- `docs/specs/004-data-classification-redaction-logging-no-leak.md`
- `docs/specs/005-core-data-model-state-machine.md`
- `docs/specs/006-request-intake-to-planning-task-workflow.md`
- `docs/specs/007-proposed-task-graph-schema-policy-promotion-reconciliation.md`
- `docs/specs/008-central-api-contract.md`
- `docs/specs/009-runner-contract-capability-summary-local-config.md`
- `docs/specs/010-artifact-manifest-validation-provenance.md`

## Purpose

This spec defines advisory critique reports, human review workflow, finding lifecycle, publication labels, and minimal MVP artifact promotion from `machine_validated` to `peer_reviewed`.

The central rule is strict:

- agent critique is evidence, not approval;
- plan verification is not human review;
- human review authority comes only from authenticated `ReviewTask` claims and accepted review records;
- public labels derive from central state, validation reports, and review records;
- no model-generated or runner-submitted field can directly promote an artifact.

## CritiqueReport

`CritiqueReport` is advisory evidence produced by a runner or future review assistant. It may help route repair or human review in later specs, but it is not approval.

Spec `014` adds a narrow code-critique ingestion path for generated `checker.py` review and repair. All other critique ingestion remains unavailable in the base MVP.

MVP status:

- `CritiqueReport` schema is defined here.
- Critique generation is not required for the MVP happy path.
- Central API rejects general critique submissions in the base MVP because spec `008` has no general critique endpoint.
- Spec `014` code critique submissions are a narrow exception with their own task type, endpoint shape, signed provenance, and non-authoritative routing rules.
- Critique records may be validated only in schema/unit tests until a future API spec adds ingestion.
- Any attempted HTTP submission of general critique data to the MVP central API is rejected as unsupported because spec `008` intentionally defines no general critique endpoint.

Allowed fields:

- `critique_id`
- `artifact_id` optional.
- `proposal_id` optional.
- `work_packet_id` optional.
- `runner_id`
- `critique_type`
- `status`
- `outcome`
- `findings`
- `checked_items`
- `authority`
- `created_at`

Enums:

- `critique_type`: `artifact_sanity_check`, `plan_consistency_check`, `accessibility_draft_check`, `license_draft_check`
- `status`: `submitted`
- `outcome`: `no_blocking_findings`, `blocking_findings`, `needs_human_review`, `invalid_input`
- `authority`: `advisory_only`

Rules:

- Exactly one of `artifact_id` or `proposal_id` must be present.
- `runner_id` is checked against authenticated runner context if accepted through an API.
- Critique findings use `CritiqueFindingInput` below and do not use persisted `Finding` authority fields.
- Critique cannot create central `Finding` rows in the MVP.
- Critique cannot create `Review`, cannot satisfy review quorum, cannot mark `approved_for_peer_reviewed`, and cannot publish artifacts.
- Critique `recommended_next_state` is forbidden in MVP to avoid state laundering.

Allowed `CritiqueFindingInput` fields:

- `severity`
- `finding_type`
- `safe_location`
- `safe_message`

Forbidden critique finding fields:

- `finding_id`
- `parent_type`
- `parent_id`
- `blocking`
- `state`
- `created_by_actor_id`
- `created_at`
- `resolved_by_actor_id`
- `resolved_at`
- `waived_by_actor_id`
- any actor, state, resolution, or authority field.

## Agent-Driven Verification Task Types

Agent-driven verification in this project has two categories:

| Type | Entity | Authority |
|---|---|---|
| Plan verification | `PlanVerificationTask` / `PlanVerification` | May satisfy proposal promotion gate when accepted by deterministic rules. |
| Critique | `CritiqueReport` | Advisory only; may route repair/human review in later specs. |

Plan verification remains governed by specs `005`, `007`, and `008`.

Critique reports:

- cannot count as plan verification unless submitted through a `PlanVerificationTask`;
- cannot count as human review;
- cannot raise public artifact label;
- cannot resolve human-review findings;
- cannot waive validation failures.

## Deterministic Routing from Critique

Base MVP routing:

- No general critique ingestion endpoint exists.
- Critique schema fixtures may be validated locally for future compatibility.
- Critique findings are advisory-only data inside the critique object and are not persisted as central `Finding` rows.
- Critique findings are not consumed by MVP `peer_reviewed` guards.
- Critique never directly changes artifact state.
- Spec `014` code critique routing is limited to generated-code repair evidence and still cannot approve, validate, publish, or satisfy human review.

Future routing may deterministically create:

- repair work packet;
- re-review task;
- quarantine recommendation;
- curator queue item.

Except for the narrow spec `014` code critique/repair path, central routing from general critique is unavailable in the MVP.

## Human Review Scope

The MVP human review gate promotes an artifact from `machine_validated` or `review_requested` to `peer_reviewed`.

Required review type:

- `subject_correctness`
- `pedagogy`

MVP combines both into one review task:

- `review_subject_and_pedagogy`

Deferred review gates:

- `classroom_ready`
- accessibility certification;
- localization review;
- age appropriateness beyond MVP age range;
- legal/licensing expert review beyond `CC-BY-4.0` metadata check;
- appeal board review;
- multi-reviewer quorum beyond MVP.

## ReviewTask Creation

After selected artifact bundle reaches `machine_validated`, `system_core` creates one `ReviewTask`.

`ReviewTask` fields:

- `review_task_id`
- `review_work_packet_id`
- `artifact_id`
- `request_id`
- `review_types`
- `required_reviewer_trust_level`
- `state`

Creation guards:

- artifact state is `machine_validated`;
- validation report state is `trusted_passed`;
- selected human-review work packet exists and depends on validation work packet;
- no open blocking validation finding exists;
- no existing active review task for the artifact/review type.

Review task state follows spec `005`.

## Reviewer Roles and Qualifications

MVP reviewer actor requirements:

- `actor_type=human_reviewer`;
- `status=active`;
- same `scope_id`;
- `trust_level` at least `reviewer_candidate`;
- capabilities include `human_subject_review` and `human_pedagogy_review`;
- reviewer profile or local fixture marks subject `physics` and age range `14-16`.

Reviewer profile fields visible to central core:

- `reviewer_actor_id`
- `scope_id`
- `operator_account_id`
- `conflict_group_id`
- `independence_verified`
- `review_capabilities`
- `trusted_subjects`
- `trusted_age_ranges`
- `trust_level`
- `status`

Reviewer profile must not include private employer details, full credentials documents, local paths, provider data, student records, or private notes in MVP.

## Conflict of Interest

A reviewer is disqualified when they are:

- planner actor for the promoted proposal;
- verifier actor whose accepted plan verification enabled promotion;
- generator actor for the selected artifact;
- accepted output actor for any source work packet of the artifact except the human-review work packet itself;
- same `operator_account_id` or `conflict_group_id` as any disqualified actor;
- missing or unverified `operator_account_id`, `conflict_group_id`, or `independence_verified` metadata;
- suspended, revoked, or outside scope.

Conflict checks use server-derived actor lineage and server-derived identity metadata, not submitted IDs.

Peer-reviewed promotion fails closed when independence metadata is absent, unverified, or matches any planner, verifier, generator, or accepted source-output actor.

Conflict result:

- claim is rejected with safe reason `review_conflict`;
- submitted review is rejected if conflict is discovered at submission;
- artifact does not become `peer_reviewed`.

## Review Claim, Lease, and Submission

Reviewers claim only through `/v1/review-tasks/{review_task_id}/claim`.

They do not claim the human-review `WorkPacket` directly.

Submission requires:

- active review lease;
- matching reviewer actor;
- matching artifact/review task lineage;
- unexpired lease;
- idempotency key;
- schema-valid review payload;
- no conflict.

Review submission body may contain:

- `artifact_id`
- `review_task_id`
- `review_types`
- `outcome`
- `findings`
- `recommended_next_state`

`reviewer_id` in the body is optional evidence only and must match authenticated context if present. It is not authority.

`review_id` is server-derived from the accepted review submission. Clients use the required idempotency key for retry safety and must not submit authoritative review IDs.

`findings` in the submitted review body use `ReviewFindingInput`, not persisted `Finding`.

Allowed `ReviewFindingInput` fields:

- `severity`
- `finding_type`
- `safe_location`
- `safe_message`

Rejected submitted finding fields:

- `finding_id`
- `parent_type`
- `parent_id`
- `blocking`
- `state`
- `created_by_actor_id`
- `created_at`
- `resolved_by_actor_id`
- `resolved_at`
- `waived_by_actor_id`
- any actor, state, resolution, or authority field.

The server derives persisted finding ID, parent linkage, creator actor, timestamps, `blocking`, and initial `state=open`.

## Review Outcomes

Allowed outcomes:

- `approved_for_peer_reviewed`
- `changes_requested`
- `rejected_for_use`
- `needs_subject_matter_fix`

Only `approved_for_peer_reviewed` can contribute to `peer_reviewed`.

Even with `approved_for_peer_reviewed`, promotion requires:

- review record accepted as valid evidence;
- required review types present;
- no disqualifying conflict;
- no open blocking findings attached to review, artifact, validation report, or selected proposal;
- artifact still `machine_validated` or `review_requested`;
- artifact not quarantined/deprecated.

`recommended_next_state` is non-authoritative. It may be stored as inert evidence after validation/redaction.

## Finding Schema and Lifecycle

Finding fields:

- `finding_id`
- `parent_type`
- `parent_id`
- `severity`
- `finding_type`
- `safe_location`
- `safe_message`
- `blocking`
- `state`
- `created_by_actor_id`
- `created_at`
- `resolved_by_actor_id` optional.
- `resolved_at` optional.

Severity:

- `critical`
- `major`
- `minor`
- `note`

Finding types:

- `physics_error`
- `unsafe_instruction`
- `missing_answer_key`
- `checker_mismatch`
- `license_issue`
- `pii_or_secret_leak`
- `pedagogy_issue`
- `accessibility_note`
- `formatting_issue`
- `other_safe`

Central blocking policy:

| Finding type | Allowed submitted severities | Central blocking | Waivable in MVP |
|---|---|---|---|
| `pii_or_secret_leak` | `critical`, `major`, `minor`, `note` | always blocking | no |
| `unsafe_instruction` | `critical`, `major`, `minor`, `note` | always blocking | no |
| `license_issue` | `critical`, `major`, `minor`, `note` | always blocking | no |
| `checker_mismatch` | `critical`, `major`, `minor`, `note` | always blocking | no |
| `missing_answer_key` | `critical`, `major`, `minor`, `note` | always blocking | no |
| `physics_error` | `critical`, `major`, `minor` | blocking unless `minor` | no if blocking |
| `pedagogy_issue` | `major`, `minor`, `note` | blocking when `major` | no if blocking |
| `formatting_issue` | `minor`, `note` | nonblocking | no waiver command in MVP |
| `accessibility_note` | `minor`, `note` | nonblocking | no waiver command in MVP |
| `other_safe` | `minor`, `note` | nonblocking | no waiver command in MVP |

Submitted `severity` is input only. The server normalizes `blocking` from the table above. Submitted `blocking` is rejected.

Submitted severities outside the allowed set for a finding type are rejected. Always-blocking finding types remain blocking regardless of submitted severity. The central backend never trusts a reviewer, runner, manifest, critique, or request body to downgrade a finding type into nonblocking.

States:

- `open`
- `resolved`
- `waived`
- `duplicate`
- `superseded`

MVP waiver rule:

- No waiver command is implemented in the MVP.
- `waived`, `duplicate`, and `superseded` states exist in the data model for later curator workflows but are not reachable through MVP API commands.
- Nonblocking minor/note findings may coexist with promotion only when the central blocking policy marks them nonblocking; they are not waived.
- Open blocking findings prevent `peer_reviewed`.

Resolution rules:

- Base MVP human-review findings do not implement repair loops. Spec `014` adds a separate generated-code repair loop that creates new draft artifacts and cannot resolve human-review findings directly.
- Curator duplicate/superseded actions are deferred until a command/API spec defines them.
- Finding free text is inert and redacted under spec `004`.

## MVP Peer-Reviewed Promotion

Artifact may transition to `peer_reviewed` only when all are true:

- artifact state is `machine_validated` or `review_requested`;
- trusted validation report is `trusted_passed`;
- review task is completed;
- review record is accepted as valid evidence;
- review outcome is `approved_for_peer_reviewed`;
- review includes both `subject_correctness` and `pedagogy`;
- reviewer actor passes qualifications;
- no conflict of interest;
- no open blocking findings on selected proposal, plan verification, artifact, validation report, review task, or review;
- artifact/request/proposal are not quarantined/deprecated;
- central transition function applies the state change.

The review body cannot directly set artifact state.

## Quorum

MVP quorum:

- one qualified, non-conflicted human reviewer;
- one accepted review covering both required review types.

Deferred quorum policies:

- two reviewers;
- subject + pedagogy split reviewers;
- curator final approval;
- weighted trust;
- appeal/reopen board.

Deferred quorum policies must not be implied by MVP state names.

## Appeal, Reopen, Repair, and Deprecation

MVP does not implement appeal workflows or human-review-finding repair loops. The separate generated-code repair loop defined in Spec `014` remains in MVP scope and remains non-authoritative for approval or publication promotion.

Allowed curator/admin actions:

- none through MVP public/runner/reviewer API.

Deferred curator/admin actions:

- quarantine artifact/request with safe reason;
- deprecate artifact with safe reason;
- mark finding duplicate/superseded;
- request a new review task.

These actions require a later command/API spec with actor authorization, source-state guards, safe reason enums, idempotency, public visibility behavior, and audit events before implementation.

Deprecation:

- `peer_reviewed -> deprecated` is allowed by spec `005`;
- no MVP API command performs this transition;
- if an implementation fixture directly creates a deprecated state, public label mapping below applies;
- deprecation reason is safe code, not raw notes.

Reopen:

- Reopening `peer_reviewed` to draft/review is deferred.
- If unsafe content is discovered, quarantine/deprecate rather than reopening in MVP.

## Publication Labels

MVP labels:

- `draft_generated`
- `machine_validated`
- `peer_reviewed`
- `quarantined`
- `deprecated`

Public label rules:

- label derives from central artifact state;
- manifest `status_claim` is ignored for label;
- validation report self-claim is ignored unless trusted validator path accepted it;
- review `recommended_next_state` is ignored unless central transition guards pass;
- `classroom_ready` is not an MVP label.

State-to-public-label mapping:

| Artifact state | Visibility | Public endpoint behavior | Public label |
|---|---|---|---|
| `draft_generated` | any | `404` unless authorized internal read | none |
| `validation_failed` | any | `404` unless authorized internal read | none |
| `machine_validated` | private | `404` | none |
| `machine_validated` | public | public metadata allowed if publication policy permits | `machine_validated` |
| `review_requested` | private | `404` | none |
| `review_requested` | public | public metadata allowed if publication policy permits | `machine_validated` |
| `peer_reviewed` | private | `404` | none |
| `peer_reviewed` | public | public metadata allowed | `peer_reviewed` |
| `quarantined` | any | `404` except curator/admin | none |
| `deprecated` | public and previously public | public tombstone metadata allowed | `deprecated` |
| `deprecated` | private or never public | `404` | none |

Precedence:

- `quarantined` hides the artifact even if previously public.
- `deprecated` overrides `peer_reviewed` for public label.
- private visibility returns `404` for public endpoints regardless of state.

Public artifact metadata follows specs `004`, `008`, and `010`.

## Classroom Ready and Other Deferred Policies

Deferred:

- `classroom_ready`;
- localization;
- accessibility certification;
- formal curriculum standards alignment;
- multi-reviewer quorum;
- public download of raw bundle files;
- public comments/ratings;
- appeal/reopen workflow;
- repair loops.

No MVP endpoint, manifest field, review field, critique field, or public metadata field may claim `classroom_ready`.

## Acceptance Tests

### REV-001: Machine validated artifact opens review task

Given artifact `machine_validated` with trusted validation passed.

Expected:

- one review task is created for `review_subject_and_pedagogy`;
- human-review work packet is dependency parent/provenance;
- no public label higher than `machine_validated` exists yet.

### REV-002: Qualified reviewer can claim review task

Claim review task with active human reviewer having physics/pedagogy capabilities and no conflict.

Expected:

- claim succeeds;
- active review lease is created;
- claim token returned once.

### REV-003: Conflict reviewer cannot claim or approve

Try to claim/submit review as planner, verifier, generator, wrong-scope actor, same `operator_account_id`, same `conflict_group_id`, or an actor with missing/unverified independence metadata.

Expected:

- claim or submission is rejected with safe conflict code;
- artifact remains non-`peer_reviewed`.
- promotion fails closed when required independence metadata is missing or unverified.

### REV-004: Approved review promotes to peer reviewed

Submit accepted review with `approved_for_peer_reviewed`, both review types, and no findings.

Expected:

- review is accepted as valid evidence;
- artifact transitions to `peer_reviewed`;
- public label becomes `peer_reviewed`;
- review body did not directly set state.

### REV-005: Changes requested does not promote

Submit valid review with `changes_requested`, `needs_subject_matter_fix`, or `rejected_for_use`.

Expected:

- review may be accepted as evidence;
- artifact remains `machine_validated` or `review_requested`;
- public label does not become `peer_reviewed`.

### REV-006: Blocking findings prevent promotion

Submit review with critical or major finding.

Expected:

- finding is open and blocking;
- artifact does not become `peer_reviewed`;
- critical/major finding cannot be waived in MVP.

### REV-007: Waiver and duplicate commands unavailable in MVP

Submit minor/note findings and then attempt reviewer or curator waiver, duplicate, and superseded commands through MVP APIs.

Expected:

- commands are unsupported or rejected with a safe reason;
- no finding state changes to `waived`, `duplicate`, or `superseded`;
- nonblocking minor/note findings do not require waiver to permit promotion;
- open blocking findings still block promotion.

### REV-008: Critique cannot approve

Validate a critique schema fixture with `outcome=no_blocking_findings`, attempted approval language, and attempted central finding authority fields. Attempt HTTP critique submission against the MVP central API.

Expected:

- critique fixture is advisory only;
- central API has no general critique ingestion endpoint and rejects the submission as unsupported;
- spec `014` code critique, when enabled, remains advisory repair evidence and cannot approve;
- critique findings cannot create central `Finding` rows;
- no review record is created;
- artifact state does not change;
- public label does not change.

### REV-009: Recommended next state is non-authoritative

Submit review with `recommended_next_state=peer_reviewed` or `classroom_ready`. Validate a critique fixture containing the same attempted field.

Expected:

- review field is rejected or stored only as inert evidence;
- critique fixture with `recommended_next_state` is schema-invalid in the MVP;
- central transition rules decide state;
- `classroom_ready` is rejected in MVP.

### REV-010: Public metadata allowlist

Read public artifact metadata at draft, machine-validated, peer-reviewed, quarantined, and deprecated states.

Expected:

- public fields are allowlisted;
- draft/private artifacts do not leak existence;
- peer-reviewed metadata includes validation/review summary and known limitations;
- no provider/model/local path/private note/audit data is exposed.

### REV-011: Deprecated fixture lowers public label

Load or create an implementation test fixture in which a previously public peer-reviewed artifact is already in `deprecated` state with a safe reason code. Do not call an MVP deprecation command.

Expected:

- public label becomes `deprecated`;
- public response is tombstone metadata only;
- raw reviewer/curator notes are not public.

### REV-012: Plan verification is not human review

Try to use accepted plan verification as artifact review evidence.

Expected:

- artifact does not become `peer_reviewed`;
- review task remains required.

## Security Checklist Coverage

Applicable categories from `002`:

- Runner Submission Trust Checks: critique and recommended state claims are non-authoritative.
- Plan Verification Gate Checks: plan verification stays separate from human review.
- Human Review and Publication Checks: reviewer authority, conflicts, findings, labels, and public metadata are specified.
- Data Classification Checks: findings/review notes/public metadata follow spec `004`.
- Identity, Authorization, and Replay Checks: review claims require authenticated actor and active lease.
- Logging, Error, and Audit Checks: safe reason codes and redacted findings are required.

No applicable non-deferrable checklist item is deferred for MVP peer-reviewed promotion. `classroom_ready` is explicitly deferred and unavailable.

## Review Checklist

Reviewers should fail this spec if:

- Critique can approve, promote, waive, or count as human review.
- Plan verification can count as human review.
- Submitted `reviewer_id`, `outcome`, or `recommended_next_state` can directly set artifact state.
- Same actor/operator can plan, verify, generate, and approve where independence is required.
- Critical/major findings can be waived in MVP.
- Open blocking findings can still produce `peer_reviewed`.
- `classroom_ready` can appear as an MVP label.
- Public metadata leaks private runner/provider/reviewer/audit details.
- Draft/private artifacts leak existence through public endpoints.
