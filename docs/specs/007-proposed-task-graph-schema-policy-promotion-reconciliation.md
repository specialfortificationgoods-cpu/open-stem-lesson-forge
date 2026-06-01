# 007. ProposedTaskGraph Schema, Policy Validation, Promotion, and Reconciliation Spec

Status: Passed adversarial review
Roadmap: `docs/specs/000-spec-roadmap.md`
Depends on:

- `docs/specs/001-mvp-slice-acceptance-tests.md`
- `docs/specs/002-security-privacy-abuse-resistance-checklist.md`
- `docs/specs/003-architecture-stack-no-inference-boundary.md`
- `docs/specs/004-data-classification-redaction-logging-no-leak.md`
- `docs/specs/005-core-data-model-state-machine.md`
- `docs/specs/006-request-intake-to-planning-task-workflow.md`

## Purpose

This spec defines how runner-submitted `ProposedTaskGraph` payloads are schema-validated, policy-validated, independently verified, promoted into `WorkPacket` records, or rejected/escalated.

The central backend may parse structured JSON, validate schemas, enforce enum and graph rules, apply deterministic policy checks, compare exact normalized fields, create plan verification tasks, and transactionally materialize work packets. It must not semantically merge, rewrite, summarize, rank, or repair proposed plans.

## ProposedTaskGraph Input Contract

A proposed task graph submission is accepted for validation only through a valid `PlanningTask` lease from spec `006`.

Required top-level fields:

- `proposal_id`
- `request_id`
- `planning_task_id`
- `planner_runner_id`
- `schema_version`
- `status`
- `source_request_summary`
- `assumptions`
- `missing_information`
- `proposed_artifacts`
- `proposed_tasks`
- `validation_plan`
- `human_review_required_for`

Unknown top-level fields are rejected.

Runner-submitted `status` is a claim. It must equal `proposed` in the submitted payload, but it does not set central `ProposedTaskGraph.state`.

## Versioning

MVP schema version:

- `1.0`

Version rules:

- Unknown schema versions are rejected with safe code `unsupported_proposed_graph_schema_version`.
- Accepted persisted proposals record both the runner-submitted `schema_version` and central validator version.
- Schema migration is not performed in the central core during MVP.
- A later schema version must be additive or have a reviewed migration spec before accepted.

## Source Request Summary

`source_request_summary` may contain only:

- `subject`
- `topic`
- `age_range`
- `duration_minutes`
- `language`

Each value must exactly match the server-derived request fields for the active planning task, except `duration_minutes` maps to `lesson_duration_minutes`.

Mismatch is rejected with safe code `proposal_request_summary_mismatch`.

The summary is not allowed to add inferred standards, objectives, audience labels, student profiles, or hidden constraints.

## Assumptions and Missing Information

`assumptions` and `missing_information` are required arrays that may be empty. They are runner-authored explanatory lists.

Limits:

- Maximum 12 items each.
- Maximum 240 UTF-8 scalar values per item.
- Items are escaped inert text.
- Items are scanned by data-classification rules from spec `004`.

These fields cannot alter central policy, risk, validation, promotion, or review requirements.

## Validation Plan

`validation_plan` is a required array of deterministic validation check IDs.

Allowed values:

- `manifest_schema`
- `required_files`
- `license_metadata`
- `ai_assistance_disclosure`
- `obvious_pii_heuristic`
- `obvious_inappropriate_content_heuristic`
- `python_checker_runs`
- `no_external_network_static`

Rules:

- The array must contain each allowed value exactly once for the MVP generation task.
- Values must be strings from the enum above.
- No free text is allowed.
- No object items are allowed.
- Unknown nested structure is impossible because the field is an array of enum strings only.
- `validation_plan` is advisory consistency evidence for the proposal. Validator authority comes from the deterministic validator path, not from this field.

Rejected examples include values or structures such as `endpoint`, `command`, `network_required`, `model_hint`, `headers`, `localFile`, URL-like text, local paths, provider hints, executable instructions, or nested objects.

## Proposed Artifacts

Each `proposed_artifacts` item contains:

- `artifact_type`
- `priority`

MVP `artifact_type` enum:

- `worksheet`
- `answer_key`
- `python_checker`
- `teacher_notes`

`priority` enum:

- `required`
- `optional`

MVP policy requires exactly one `required` item for each MVP artifact type and no optional artifact types. Missing, duplicate, unknown, or extra artifacts are rejected.

## Proposed Tasks

Each `proposed_tasks` item contains:

- `local_id`
- `phase`
- `task_type`
- `subject`
- `topic`
- `age_range`
- `language`
- `risk_level`
- `depends_on` optional.
- `required_capabilities`
- `outputs`
- `validation_required`
- `human_review_required_for`
- `execution_policy` required for `generate_lesson_pack` and absent for non-generation tasks.

Unknown task fields are rejected.

### Execution Policy Field

Runner proposals may include `execution_policy` only as an advisory request. The central backend recomputes the final work-packet execution policy deterministically under spec `013`.

MVP accepted proposal value:

- `execution_policy=code_generation_only` for `generate_lesson_pack`.

Rejected proposal values:

- `sandboxed_self_test_python_checker`
- `trusted_validator_execution`
- arbitrary commands;
- network, filesystem, package installation, browser, Docker, VM, or shell execution policy fragments.

During promotion, the central backend assigns `sandboxed_self_test_python_checker` to the generation work packet only when the accepted task graph exactly matches the MVP fixture and spec `013` policy guards pass. Request text and planner text cannot grant execution authority.

### Task Local IDs

`local_id` rules:

- Unique within the proposal.
- 1 to 64 characters.
- ASCII lowercase letters, digits, underscore, and hyphen only.
- Must not start with `system_`, `admin_`, or `credential_`.
- Must not contain path separators, dots, URL syntax, shell metacharacters, or whitespace.

`local_id` is local to the proposal. Work packet IDs are generated by the central backend during promotion.

### Phase Enum

MVP phases:

- `initial_generation`
- `mechanical_validation`
- `human_review`

Phase ordering:

- `initial_generation` may have no dependencies.
- `mechanical_validation` must depend on generation work that produces the selected artifact bundle.
- `human_review` must depend on validation work.

### Task Type Enum

MVP allowed task types:

- `generate_lesson_pack`
- `run_artifact_validation`
- `review_subject_and_pedagogy`

Forbidden task types include:

- `arbitrary_prompt`
- `credential_handling_task`
- `student_grading_task`
- `student_placement_task`
- `student_profile_task`
- `send_email`
- `web_fetch`
- `browser_browse`
- `install_dependency`
- `execute_shell`
- `provider_account_setup`
- `payment_or_subscription`
- `collect_student_data`
- `publish_without_review`

Unknown task types are rejected.

### Subject and Request Field Consistency

For every proposed task:

- `subject` must equal request `subject`.
- `topic` must equal request `topic`.
- `age_range` must equal request `age_range`.
- `language` must equal request `language`.

Runner-proposed reinterpretations may appear only in inert `assumptions` or `missing_information`, not in task fields.

### Risk Level

Submitted `risk_level` enum:

- `low`
- `medium`
- `high`

Central policy computes `central_risk_level` deterministically. It may raise risk above runner-submitted risk; it may not lower risk based on runner claims.

MVP central risk rules:

- `generate_lesson_pack` for the accepted physics slice is `low` only when outputs and validations match this spec.
- `run_artifact_validation` is `low` only when it references deterministic validator work and no network/provider access.
- `review_subject_and_pedagogy` is `low` only when it is human review and does not process student records.
- Any generated executable file other than MVP `checker.py` raises to `medium`.
- Any network requirement, credential handling, student PII, student grading, profile inference, provider-account handling, payment handling, browser automation, shell execution by a runner, runner tool execution outside spec `013`, or publication without review raises to `high` and is rejected in the MVP.
- Unknown task type, unknown capability, unknown output, or unknown validation raises to rejection, not medium fallback.

MVP automatic promotion supports only `central_risk_level=low` for every task.

### Required Capabilities

Allowed runner/work capabilities in proposed tasks:

- `stem_pedagogy`
- `structured_markdown`
- `basic_python`
- `artifact_validation`
- `python_execution_limited`
- `human_subject_review`
- `human_pedagogy_review`

Capabilities are requirements for later claims. They do not grant trust or authority.

### Outputs

Allowed outputs for `generate_lesson_pack`:

- `worksheet.md`
- `answer_key.md`
- `checker.py`
- `teacher_notes.md`
- `manifest.json`

Allowed outputs for `run_artifact_validation`:

- `validation_report.json`

`run_artifact_validation` may be represented as a promoted work packet so the workflow has explicit dependency tracking, but the trusted validation report can only be produced through the deterministic validator path from specs `003`, `005`, and `010`. Runner-submitted `validation_report.json` remains non-authoritative evidence and cannot move artifact state.

Allowed outputs for `review_subject_and_pedagogy`:

- `review.json`

Output path rules:

- Exact basename only.
- No directories.
- No dot-dot.
- No absolute paths.
- No URL-like values.
- No shell metacharacters.
- No duplicate output names in one task.

### Validation Required

Allowed validation checks:

- `manifest_schema`
- `required_files`
- `license_metadata`
- `ai_assistance_disclosure`
- `obvious_pii_heuristic`
- `obvious_inappropriate_content_heuristic`
- `python_checker_runs`
- `no_external_network_static`

For `generate_lesson_pack`, all allowed validation checks are required.

For `run_artifact_validation`, `validation_required` must be empty because it is the validator work packet itself.

For `review_subject_and_pedagogy`, `validation_required` must be empty.

### Human Review Required For

Allowed value:

- `peer_reviewed`

The proposal must include:

- Top-level `human_review_required_for=["peer_reviewed"]`.
- Generation task `human_review_required_for=["peer_reviewed"]`.
- Human review task `human_review_required_for=["peer_reviewed"]`.

The validation task may have an empty list.

Missing human review requirements reject the proposal.

## Dependency Graph Rules

The proposed graph must be a directed acyclic graph over task `local_id` values.

Rules:

- Every dependency must reference an existing `local_id`.
- No self-dependency.
- No cycle.
- No duplicate dependency edge.
- The graph must include exactly one generation task, one validation task, and one human review task for the MVP.
- The validation task must depend on the generation task.
- The human review task must depend on the validation task.
- Generation must not depend on validation or human review.
- No disconnected required task.

Cycle detection and ordering are deterministic graph algorithms, not semantic plan interpretation.

## Forbidden Fields

Reject a proposal if any object at any depth contains keys matching:

- `prompt`
- `arbitrary_prompt`
- `system_prompt`
- `developer_prompt`
- `user_prompt`
- `hidden_instruction`
- `model`
- `provider`
- `api_key`
- `token`
- `credential`
- `provider_base_url`
- `local_path`
- `auth_path`
- `cookie`
- `headers`
- `tool_call`
- `function_call`
- `webhook`
- `url`
- `attachment`
- `student_names`
- `student_records`

The rejected key path stored in errors or transition events must be schema-derived and safe under spec `004`, not copied from attacker-controlled object keys.

## Policy Validation Result

Central policy validation produces a deterministic result:

```json
{
  "proposal_id": "plan_energy_001_a",
  "schema_result": "passed",
  "policy_result": "passed",
  "central_risk_level": "low",
  "errors": [],
  "warnings": []
}
```

Allowed `schema_result` values:

- `passed`
- `failed`

Allowed `policy_result` values:

- `passed`
- `failed`

Errors contain:

- safe `code`
- schema-derived `field_path`
- optional safe `expected`
- optional safe `actual_classification`

Errors must not contain raw rejected values, prompt text, secrets, local paths, URLs, provider config, or student PII.

## State Transitions

Validation transitions:

- `proposed -> schema_rejected` when JSON shape/schema fails.
- `proposed -> policy_rejected` when shape passes but deterministic policy fails.
- `proposed -> schema_policy_validated` when both pass.
- `schema_policy_validated -> verification_required` when a `PlanVerificationTask` is created.

Rejected proposals never create work packets, verification tasks, artifacts, validation reports, review tasks, or public labels.

Schema- or policy-rejected proposals do not satisfy `min_accepted_proposals` from spec `006`. Rejection consumes or rejects only the submitting lease according to the submission result, does not complete the `PlanningTask`, does not revoke other active fanout leases, and allows the planning task to return to `open` when no active leases remain and retry limits allow. A later valid proposal from an eligible lease or replacement claim may still pass schema/policy validation.

Only `schema_policy_validated` proposals can satisfy the planning fanout success condition. When the first proposal reaches `schema_policy_validated`, fanout closure from spec `006` applies: the submitting lease is consumed, other active planning leases are revoked with safe reason `planning_min_proposals_satisfied`, and the planning task becomes `completed`.

## Plan Verification Gate

After `schema_policy_validated`, the central backend creates one `PlanVerificationTask` as defined by spec `005`.

Verification-task creation is idempotent:

- Unique key: `(proposal_id, verification_type=plan_schema_policy_cross_check)`.
- Revalidating the same already-validated proposal returns the existing verification task.
- Replayed validation emits no duplicate `PlanVerificationTask` and no duplicate transition event.
- A changed validation attempt under the same command idempotency key is rejected.

Promotion is impossible until a `PlanVerification` is accepted as evidence and:

- verification task was created by central core for the same proposal;
- verifier actor differs from planner actor;
- verifier scope matches request/proposal scope;
- verifier lease is active at submission and consumed on success;
- verification outcome is `no_blocking_findings`;
- findings contain no open blocking, critical, or major item;
- verification is not human review and cannot raise artifact public labels.

Blocking verification moves proposal to `verification_blocked` and creates no work packets.

## Promotion Rules

Promotion is a deterministic transaction from one verified proposal to work packets.

### Low-Risk MVP Promotion

A proposal may promote in the MVP only when all are true:

- Request is accepted and not quarantined/deprecated.
- Planning task is completed for this proposal.
- Proposal is `verified_for_mvp_promotion`.
- `central_risk_level=low`.
- Exactly one required generation task, one validation task, and one human review task exist.
- Dependency graph satisfies this spec.
- All required artifacts and validation checks are present.
- Human review gate for `peer_reviewed` is present.
- No blocking finding is open against the proposal or verification.
- No active competing planning lease remains after spec `006` fanout closure.

Promotion creates work packets transactionally:

- one work packet per proposed task;
- server-generated `work_packet_id`;
- original `proposal_id`;
- original `source_local_task_id`;
- snapshot of normalized task fields;
- dependencies rewritten from local IDs to generated work packet IDs;
- initial state `open` when dependencies are satisfied, otherwise `blocked_by_dependency`.

The transaction creates a `PromotionDecision` with `accepted`, records created work packet IDs, emits safe transition events, and moves proposal to `promoted`.

### Medium-Risk Proposals

The MVP does not automatically promote medium-risk proposals.

If deterministic policy raises any task to `medium`, central behavior is:

- transition proposal from `proposed` to `policy_rejected` with safe reason `medium_risk_requires_future_policy`;
- keep proposal out of verification and automatic promotion;
- create no work packets;
- optionally create a safe curator-visible `Finding` if curator review features are enabled by a later spec.

A future reviewed spec may allow medium-risk promotion only with:

- two independent planner proposals from different actors;
- two independent plan verifications from actors different from both planners;
- deterministic compatibility checks over exact normalized fields;
- no central semantic merge;
- explicit curator escalation for conflicts.

Until that spec exists, medium-risk proposals are non-promotable.

### High-Risk Proposals

High-risk proposals are rejected in the MVP.

High-risk conditions include credential handling, student records, grading, profiling, hidden network requirements, browser automation, shell execution, provider-account handling, payment handling, public publication without review, or any generated executable beyond the accepted `checker.py` path.

Central behavior:

- proposal becomes `policy_rejected` when high risk is detected during policy validation;
- no verification task is created after clear high-risk policy rejection;
- no work packets are created;
- safe event and finding may be created without raw unsafe payload.

## Reconciliation and Human Curator Escalation

The MVP does not implement central semantic reconciliation.

For incompatible duplicate proposals, medium-risk proposals, or proposals requiring human judgment:

- central core records safe structured reason codes;
- no work packets are created;
- proposal remains non-promotable;
- a later curator/reconciliation spec may define a `PlanningTask` or separate entity for human-mediated selection;
- any future compatibility checks must be exact structural comparisons only and must be reviewed before implementation;
- until that later spec exists, escalation is represented only by safe proposal state, `Finding`, and `StateTransitionEvent` records.

The central backend must never merge two proposals into a new task graph.

## Repeated Promotion and Reconciliation Idempotency

Repeated promotion for the same proposal follows spec `005`:

- unique work packets by `(proposal_id, source_local_task_id)`;
- identical replay returns the original `PromotionDecision`;
- changed replay with same idempotency key is rejected;
- partial failure rolls back all work packet creation.

Repeated rejection/escalation:

- creates no duplicate findings for the same `(proposal_id, code, safe_field_path)`;
- emits no duplicate transition events for identical replay;
- cannot move a rejected proposal back to promotable without a new runner submission or future curator spec.

## Acceptance Tests

### PG-001: Valid MVP graph schema and policy pass

Submit the proposed graph fixture from spec `001` through a valid planning lease.

Expected:

- Schema result is `passed`.
- Policy result is `passed`.
- Central risk is `low`.
- Proposal reaches `schema_policy_validated`.
- A plan verification task is created.
- No work packets exist yet.

Replay schema/policy validation for the same proposal.

Expected:

- Existing plan verification task is returned.
- No duplicate verification task or transition event is created.

### PG-002: Malformed JSON and unknown fields reject safely

Submit malformed JSON, wrong types, unknown top-level fields, and unknown nested task fields.

Expected:

- Proposal is `schema_rejected`.
- Safe errors contain schema-derived paths and codes.
- Raw rejected values are not logged or persisted.
- No verification task or work packets are created.
- Planning fanout remains open for another eligible proposal if retry limits allow.

### PG-003: Forbidden prompt and provider fields reject

Submit otherwise valid proposal containing `arbitrary_prompt`, `system_prompt`, `model`, `provider`, `api_key`, `provider_base_url`, `auth_path`, or `url` at any depth.

Expected:

- Proposal is rejected.
- Error output contains safe code only.
- No raw prompt, secret, provider value, URL, or local path appears in persistence/logs/errors.

### PG-003A: Validation plan is enum-only

Submit `validation_plan` containing free text, objects, nested fields, `endpoint`, `command`, `network_required`, `model_hint`, `headers`, `localFile`, URL-like text, or local path text.

Expected:

- Proposal is schema- or policy-rejected safely.
- No raw unsafe value is stored.
- No verification task or work packets are created.

### PG-004: Dependency graph rules reject cycles and disconnected required tasks

Submit proposals with cyclic dependencies, missing dependencies, self-dependencies, duplicate edges, disconnected required tasks, or wrong phase ordering.

Expected:

- Proposal is rejected before verification.
- No work packets are created.

### PG-005: Missing human review gate rejects

Submit proposal without `peer_reviewed` human review requirement or without `review_subject_and_pedagogy`.

Expected:

- Proposal is `policy_rejected`.
- No verification task or work packets are created.

### PG-006: High-risk task rejects

Submit proposal containing student grading, student records, credential handling, web fetch, browser automation, shell execution, or publish-without-review task.

Expected:

- Proposal is rejected with safe risk/policy code.
- No verification task, work packet, artifact, validation report, or review task exists.

### PG-006A: Rejected proposal does not close fanout

Submit a malformed or policy-rejected proposal through one planning fanout lease while another eligible planning lease or replacement claim remains available.

Expected:

- Rejected proposal does not satisfy `min_accepted_proposals`.
- Planning task does not become `completed`.
- Other active fanout leases are not revoked for `planning_min_proposals_satisfied`.
- A later valid proposal can still reach `schema_policy_validated` if retry limits allow.

### PG-007: Verification is required before promotion

Try to promote a schema/policy-valid proposal before accepted plan verification.

Expected:

- Promotion is denied.
- No work packets are created.
- Proposal remains non-promoted.

### PG-008: Blocking verification blocks promotion

Submit accepted verification with blocking, critical, or major finding.

Expected:

- Proposal becomes `verification_blocked`.
- Promotion is denied.
- No work packets are created.

### PG-009: Low-risk verified proposal promotes transactionally

Submit valid proposal and accepted independent verification, then promote.

Expected:

- Promotion decision is `accepted`.
- Exactly three work packets are created for generation, validation, and human review.
- Dependencies are rewritten from local IDs to generated work packet IDs.
- Proposal state is `promoted`.
- Request state advances to `decomposed`.

### PG-010: Promotion replay cannot duplicate work packets

Promote the same verified proposal twice.

Expected:

- First promotion creates work packets.
- Replay returns original decision or `no_op_existing`.
- No duplicate `(proposal_id, source_local_task_id)` exists.

### PG-011: Medium-risk proposal is non-promotable in MVP

Submit proposal with deterministic medium-risk condition.

Expected:

- Proposal becomes `policy_rejected` during deterministic policy validation with safe reason `medium_risk_requires_future_policy`.
- No verification task is created.
- No work packets are created.
- No semantic compatibility or merge is attempted.

### PG-012: Late fanout proposals are rejected, not merged

After one proposal has satisfied spec `006` fanout closure, submit a later structurally different proposal through a stale/revoked fanout lease.

Expected:

- Central core does not merge them.
- Late fanout submission is rejected and creates no selected proposal.
- No future compatibility engine is invoked in the MVP.

### PG-013: Cross-lineage promotion is rejected

Try to promote proposal using request, planning task, verification, or lease data from another scope/request.

Expected:

- Promotion is rejected with safe lineage reason.
- No work packets are created or relinked.

## Security Checklist Coverage

Applicable categories from `002`:

- Central-Core Boundary Checks: validation and promotion are schema/graph/policy functions only.
- Data Classification Checks: rejected fields and errors follow spec `004`.
- Runner Submission Trust Checks: runner proposals are untrusted until schema/policy/verification/promotion gates pass.
- Plan Verification Gate Checks: independent verification is required before promotion.
- Planning and Promotion Abuse Checks: forbidden fields, task types, risk escalation, idempotent promotion, and no semantic merge are specified.
- Artifact and Generated-Code Checks: `checker.py` is allowed only as part of the accepted MVP artifact bundle and later validator path.
- Human Review and Publication Checks: human review gate must be present before promotion.
- Identity, Authorization, and Replay Checks: promotion is tied to server-derived proposal/request lineage and idempotent materialization.

No applicable non-deferrable checklist item is deferred for proposed graph validation or MVP promotion.

## Review Checklist

Reviewers should fail this spec if:

- Central core can repair, rewrite, summarize, semantically compare, or merge proposals.
- Unknown fields, prompt fields, provider fields, URL fields, credentials, or local paths can survive validation.
- Runner-submitted `status` or `risk_level` can lower central risk or set central state.
- Missing human review gates can still promote.
- Plan verification can be skipped or treated as human review.
- Medium/high-risk proposals can create MVP work packets.
- Dependency graph validation is ambiguous.
- Promotion can partially create work packets or duplicate them on replay.
- Cross-request or cross-scope proposal data can promote work packets.
- Reconciliation creates a central semantic merge path.
