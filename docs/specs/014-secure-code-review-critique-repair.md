# 014. Secure Code Review, Critique, and Repair Runner Spec

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
- `docs/specs/011-critique-human-review-publication-promotion.md`
- `docs/specs/012-mvp-guardrail-end-to-end-test-matrix.md`
- `docs/specs/013-task-scoped-code-execution-sandbox-provenance.md`

## Purpose

This spec defines how runners may review, critique, and repair generated code drafts without letting runner output become central authority.

The central rule is:

- code critique is signed advisory evidence;
- code repair is a new untrusted draft artifact, never an in-place mutation;
- sandboxed runner tests are provenance evidence only;
- trusted validation still comes only from the spec `010` validator;
- human review authority still comes only from spec `011` `ReviewTask` records.

## Research Inputs

This spec extends patterns already adopted by spec `013`:

- OWASP secure code review guidance treats automated and manual review as complementary; automated review must not be the only approval gate for security-sensitive code.
- GitHub Actions self-hosted runner guidance treats untrusted code execution as a persistent compromise risk unless jobs are isolated and credentials are scoped.
- SLSA and in-toto provenance patterns bind outputs to materials, producers, and execution context through structured attestations.
- OpenSSF Scorecard-style automation is useful as repeatable evidence but is not a substitute for policy decisions.

Implementation notes:

- The MVP review target is only generated `checker.py` inside the lesson artifact bundle.
- Future specs may broaden review targets to additional code files or repository-scale code, but must define new task types, sandbox profiles, resource limits, and review schemas.

## Scope

MVP allowed capabilities:

- a runner may claim a code critique work packet for a draft or validation-failed artifact;
- a runner may run static analysis and bounded sandboxed sample tests against `checker.py`;
- a runner may submit a signed `CodeCritiqueReport`;
- a runner may claim a repair work packet created by deterministic central routing;
- a repair runner may submit a new repaired artifact bundle reference plus signed provenance;
- a teacher may request a bounded automated repair loop during request intake;
- a volunteer runner operator may opt a runner into automated repair-loop work;
- a repair runner may submit a signed interruption report when local quota or operator policy prevents completing the loop.

MVP forbidden capabilities:

- no runner may mutate an existing artifact in place;
- no runner critique may create trusted validation state;
- no runner critique may create human review approval;
- no runner repair may skip validator or human review gates;
- no runner may execute arbitrary commands, install packages, fetch URLs, run Docker/VM/browser automation, or use host paths;
- no central backend component may perform model inference, code review, semantic repair, or prompt construction;
- no teacher request or runner opt-in can grant execution authority without central deterministic policy assignment.

## Task Types and Execution Policies

Compatibility with the spec `012` MVP gate: this spec defines the secure code critique and repair contract, but its central API ingestion surfaces are not active in the current MVP gate. Spec `012` keeps `CR-GATE-*` rows as `deferred_by_spec` and requires negative-unavailable checks for central critique, repair, and interruption submission endpoints until a later gate explicitly activates this spec. The runner/core contract, schemas, and policy tests may exist ahead of activation, but the central backend must not expose these submission surfaces in the MVP API.

Post-MVP task types activated by this spec:

| Task type | Purpose | Execution policy |
|---|---|---|
| `critique_generated_code` | Review generated `checker.py` and submit advisory findings. | `sandboxed_code_critique_python_checker` |
| `repair_generated_code` | Produce a repaired artifact bundle candidate. | `sandboxed_code_repair_python_checker` |

Post-MVP execution policy enum additions:

- `sandboxed_code_critique_python_checker`
- `sandboxed_code_repair_python_checker`

Rules:

- The central backend assigns these policies only during deterministic work-packet materialization.
- Request text, artifact text, critique report text, repair proposal text, and runner config cannot request or raise execution policy.
- A critique runner may lower behavior below policy by submitting static-only critique evidence with `not_run_sandbox_unavailable`; a repair runner that cannot safely produce a complete repaired artifact must submit an interruption report instead.
- `trusted_validator_execution` remains reserved for the spec `010` validator and cannot be claimed by critique or repair runners.

## Work Packet Materialization

The central backend may create a `critique_generated_code` work packet when one of these deterministic triggers occurs:

- generation output is accepted and artifact state is `draft_generated`;
- trusted validation fails with a repairable code-related failure code;
- a human reviewer requests code-focused advisory critique through a future reviewed human-review extension.

When a later gate activates spec `014`, the deterministic default is:

- create at most one code critique work packet for the initial `draft_generated` artifact;
- create repair work packets only when either a curator explicitly requests repair or the automated repair-loop gates below pass;
- create at most two total repair attempts per artifact lineage.

## Automated Repair Loop Opt-In

Automated repair is optional and bounded. It requires both:

- teacher request preference: the request includes `auto_repair_preference=request_bounded_code_repair`;
- volunteer runner-operator opt-in: an eligible repair runner advertises `automated_repair_loop_opt_in=true` in its redacted capability summary and has matching local config.

Allowed request preference values:

- `no_automated_repair`
- `request_bounded_code_repair`

Rules:

- Default is `no_automated_repair`.
- Request preference is inert policy input, not execution authority.
- Request text such as "keep fixing until it works" is ignored unless the structured request preference is present and passes intake schema.
- Central may still decline automated repair using only the closed deterministic decline reasons below.
- Runner opt-in is capability evidence, not authority; central still assigns each repair work packet.
- Runner operators may set a stricter local maximum than central policy.
- Central must not hide automated repair from the teacher; request status and safe public/internal metadata must show when automated repair was requested, declined, running, exhausted, or completed.

Closed automated-repair decline reasons, in deterministic priority order:

1. `not_requested`
2. `request_not_eligible`
3. `moderation_not_allowed`
4. `artifact_state_not_eligible`
5. `non_code_safety_checks_failed`
6. `quarantine_required`
7. `human_review_already_open`
8. `attempt_limit_reached`
9. `no_opted_in_runner_available`
10. `runner_scope_or_capability_mismatch`
11. `curator_or_admin_disabled`

Any "risk" or "policy" decline must map to one of these closed deterministic reasons or to an explicit curator/admin command. The central backend must not perform semantic risk scoring.

Automated repair-loop creation requires all of:

- request has `auto_repair_preference=request_bounded_code_repair`;
- source artifact belongs to that request and selected proposal lineage;
- source artifact is `validation_failed` after deterministic validation has run, or `draft_generated` only when deterministic pre-validation safety checks for non-code files and manifest public text have passed;
- source artifact is not `quarantined`, `deprecated`, `review_requested`, or `peer_reviewed`;
- critique or trusted validation produced repairable code reason codes and no non-code safety failure is open;
- no open human review task exists for the source artifact;
- repair attempt count for the artifact lineage is below the request and central limits;
- an eligible repair runner has opted in and matches scope/capability rules.

Automated repair-loop status enum:

- `not_requested`
- `declined_by_policy`
- `waiting_for_opted_in_runner`
- `waiting_for_operator_review`
- `needs_operator_review`
- `running`
- `exhausted_attempts`
- `completed_repaired_draft_created`
- `stopped_for_quarantine`
- `stopped_for_human_review`

Terminal statuses:

- `not_requested`
- `declined_by_policy`
- `exhausted_attempts`
- `completed_repaired_draft_created`
- `stopped_for_quarantine`
- `stopped_for_human_review`

Non-terminal statuses:

- `waiting_for_opted_in_runner`
- `waiting_for_operator_review`
- `needs_operator_review`
- `running`

Allowed transitions:

| From | To | Guard |
|---|---|---|
| none | `not_requested` | Request preference absent or `no_automated_repair`. |
| none | `declined_by_policy` | Highest-priority decline reason is present. |
| none | `waiting_for_opted_in_runner` | Teacher requested repair, artifact eligible, no opted-in runner currently available. |
| none or `waiting_for_opted_in_runner` | `running` | Eligible opted-in repair runner work packet created transactionally. |
| `running` | `completed_repaired_draft_created` | Repair output accepted and new draft artifact created. |
| `running` | `waiting_for_operator_review` | Interruption reason is quota, local budget, provider unavailable, or sandbox unavailable. |
| `running` | `needs_operator_review` | Interruption reason is suspected loop, suspected malicious task, or local policy refused. |
| `waiting_for_operator_review` or `needs_operator_review` | `running` | Authorized continuation decision creates next repair work packet. |
| `waiting_for_operator_review` or `needs_operator_review` | `exhausted_attempts` | Authorized decision stops or continuation limit reached. |
| any non-terminal | `stopped_for_quarantine` | Curator/admin quarantine or deterministic quarantine guard. |
| any non-terminal | `stopped_for_human_review` | Human review opens for source or repaired artifact. |

Loop statuses are central-derived metadata. They do not replace artifact state.

Non-code safety precondition:

- Automated repair may copy non-code files byte-for-byte only after deterministic checks prove those files are safe to carry forward.
- Required safe checks before automated repair:
  - `bundle_shape`
  - `path_normalization`
  - `manifest_schema`
  - `required_files`
  - `file_size_limits`
  - `utf8_text`
  - `license_metadata`
  - `ai_assistance_disclosure`
  - `manifest_lineage`
  - `contents_match_files`
  - `markdown_safety`
  - `obvious_pii_heuristic`
  - `obvious_inappropriate_content_heuristic`
  - `secret_like_value_heuristic`
  - `no_external_network_static`
  - `public_provenance_allowlist`
- If any required non-code safety check fails, automated repair is stopped with `declined_by_policy` and `auto_repair_decline_reason=non_code_safety_checks_failed` or `quarantine_required`.
- The only validation failures that automated repair may try to fix are `python_checker_static_safety` and `python_checker_runs`.

## Repair Loop Interruption and Continuation

Automated repair can stop before a repair proposal exists. This is expected when a volunteer runner reaches local quota, loses provider availability, detects a loop, or hits local operator policy.

Allowed interruption reasons:

- `runner_quota_exhausted`
- `runner_operator_budget_exhausted`
- `provider_unavailable_local`
- `sandbox_unavailable`
- `suspected_looping_bug`
- `suspected_malicious_task`
- `local_policy_refused`

Forbidden interruption data:

- exact quota values;
- account IDs;
- provider names;
- provider response bodies;
- prompts or raw model responses;
- local paths;
- secrets or tokens;
- raw generated code excerpts.

Global submitted-text rule:

- `safe_location`, `safe_message`, `safe_summary_code`, reason codes, and any future bounded text fields must not contain source-code snippets, tool-call JSON, Markdown, HTML, prompt-like instructions, URLs, local paths, raw stdout/stderr, provider output, or secret-like values.
- Locations are structured pointers only. Messages are inert human-readable summaries only.
- Violations are rejected before persistence and before log/event emission.

`CodeRepairInterruptionReport` is a closed JSON object. Unknown fields are rejected.

Required fields:

- `interruption_report_id`
- `work_packet_id`
- `lease_id`
- `runner_actor_id`
- `source_artifact_id`
- `source_bundle_digest`
- `repair_attempt_id`
- `repair_attempt_index`
- `repair_continuation_index`
- `repair_root_artifact_id`
- `repair_parent_artifact_id`
- `execution_policy`
- `interruption_reason`
- `safe_summary_code`
- `attestation`
- `created_at`

Optional fields:

- `source_critique_report_id`, omitted only when validation-triggered repair has no critique report.
- `partial_work_digest`

Allowed `safe_summary_code`:

- `local_quota_exhausted_before_output`
- `operator_budget_exhausted_before_output`
- `provider_unavailable_before_output`
- `sandbox_unavailable_before_output`
- `loop_suspected_no_output`
- `malicious_task_suspected_no_output`
- `local_policy_refused_no_output`

`partial_work_digest` uses the spec `013` digest string format and covers only local scratch bytes retained by the runner; the scratch bytes are not submitted to central API.

Rules:

- Interruption report submission consumes the active lease and makes the current repair work packet terminal as `interrupted`.
- No artifact is created from an interruption report.
- `partial_work_digest` may cover local scratch bytes for provenance, but partial bytes are not uploaded to central API in the MVP.
- The report is internal evidence for `runner_operator`, curator, or admin review only.
- The report cannot count as repair failure, trusted validation, human review, or publication approval.
- Interruption report submission cannot include continuation `outcome`, continuation `safe_reason_code`, continuation decision ID, stop command, continue command, or continuation idempotency key. Those fields are accepted only by `RepairContinuationDecision`.

Central interruption routing:

- If `interruption_reason=runner_quota_exhausted`, `runner_operator_budget_exhausted`, `provider_unavailable_local`, or `sandbox_unavailable`, central sets automated loop metadata to `waiting_for_operator_review`.
- If `interruption_reason=suspected_looping_bug`, `suspected_malicious_task`, or `local_policy_refused`, central sets automated loop metadata to `needs_operator_review`.
- A `runner_operator`, `curator`, or `admin` continuation decision may choose exactly one closed outcome:
  - `continue_on_another_runner`
  - `stop_exhausted_attempts`
  - `stop_policy_refused`
  - `quarantine_request_or_artifact`
  - `mark_repair_bug_for_human_triage`
- Continuation creates a new repair work packet with the same source artifact and same repair attempt index only when no repaired artifact was submitted for that attempt.
- Continuation must target an eligible opted-in runner actor different from the interrupted runner when the interruption reason was quota or operator budget exhaustion.
- Continuation is still bounded by the same per-lineage repair attempt and interruption limits.
- MVP limit: at most two interruption continuations per repair attempt.

## RepairContinuationDecision

`RepairContinuationDecision` is a durable central command record for interrupted automated repair.

Allowed actor types:

- `runner_operator` for runners they own within the same scope;
- `curator`;
- `admin`.

Required fields:

- `repair_continuation_decision_id`
- `interruption_report_id`
- `repair_attempt_id`
- `source_artifact_id`
- `repair_attempt_index`
- `interrupted_repair_continuation_index`
- `next_repair_continuation_index` required only when `outcome=continue_on_another_runner`
- `actor_id`
- `outcome`
- `safe_reason_code`
- `idempotency_key`
- `created_at`

Allowed `outcome`:

- `continue_on_another_runner`
- `stop_exhausted_attempts`
- `stop_policy_refused`
- `quarantine_request_or_artifact`
- `mark_repair_bug_for_human_triage`

Allowed `safe_reason_code`:

- `quota_exhausted_continue_elsewhere`
- `operator_budget_exhausted_continue_elsewhere`
- `provider_unavailable_continue_elsewhere`
- `sandbox_unavailable_continue_elsewhere`
- `quota_exhausted_stop`
- `provider_unavailable_stop`
- `sandbox_unavailable_stop`
- `loop_suspected_stop`
- `malicious_task_suspected_stop`
- `operator_budget_stop`
- `local_policy_refused_stop`
- `curator_quarantine`
- `human_triage_required`

Source-state guards:

- interruption report is accepted;
- repair attempt state is `interrupted`;
- `interrupted_repair_continuation_index` matches the interrupted work packet and interruption report;
- `next_repair_continuation_index=interrupted_repair_continuation_index+1` when continuing;
- automated repair-loop status is `waiting_for_operator_review` or `needs_operator_review`;
- no repaired artifact exists for the repair attempt;
- continuation count is below 2;
- source artifact is not `quarantined`, `deprecated`, `review_requested`, or `peer_reviewed`;
- actor is same scope and authorized for the interrupted runner or artifact.

Decision consistency:

- `continue_on_another_runner` is allowed only when interruption reason is `runner_quota_exhausted`, `runner_operator_budget_exhausted`, `provider_unavailable_local`, or `sandbox_unavailable`.
- `continue_on_another_runner` safe reason must be one of:
  - `quota_exhausted_continue_elsewhere`
  - `operator_budget_exhausted_continue_elsewhere`
  - `provider_unavailable_continue_elsewhere`
  - `sandbox_unavailable_continue_elsewhere`
- `suspected_looping_bug` may only map to `stop_exhausted_attempts` with `loop_suspected_stop` or `mark_repair_bug_for_human_triage` with `human_triage_required`.
- `suspected_malicious_task` may only map to `quarantine_request_or_artifact` with `curator_quarantine` or `mark_repair_bug_for_human_triage` with `malicious_task_suspected_stop`.
- `local_policy_refused` may only map to `stop_policy_refused` with `operator_budget_stop` or `mark_repair_bug_for_human_triage` with `human_triage_required`.
- Curator/admin override to continue after suspected malicious task, suspected looping bug, or local policy refusal is deferred; no MVP command supports it.

Side effects:

- `continue_on_another_runner` creates one new repair work packet transactionally with the same `repair_attempt_id`, same `repair_attempt_index`, `repair_continuation_index=next_repair_continuation_index`, and an excluded runner actor equal to the interrupted runner.
- stop outcomes close the repair attempt as `stopped` and do not create work.
- `quarantine_request_or_artifact` routes to the existing curator/admin quarantine command path and creates no repair work.
- replay with the same idempotency key and identical payload returns the original decision; changed replay is rejected.

Critique work packet fields:

- `task_type=critique_generated_code`
- `execution_policy=sandboxed_code_critique_python_checker`
- `execution_profile_id=python_checker_code_critique_v1`
- `allowed_command_id=python_checker_code_critique_harness_v1`
- `source_artifact_id`
- `source_bundle_digest`
- `network_policy=deny_all`
- `filesystem_policy=claim_workspace_read_only_input_write_report_only`
- `secret_policy=no_task_secrets`
- `max_wall_time_ms=5000`
- `max_cpu_time_ms=2000`
- `max_memory_bytes=134217728`
- `max_processes=1`
- `attestation_required=true`

Repair work packet fields:

- `task_type=repair_generated_code`
- `execution_policy=sandboxed_code_repair_python_checker`
- `execution_profile_id=python_checker_code_repair_v1`
- `allowed_command_id=python_checker_code_repair_harness_v1`
- `source_artifact_id`
- `source_bundle_digest`
- `source_critique_report_id` when repair was critique-triggered
- `repair_attempt_id`
- `repair_attempt_index`
- `repair_continuation_index`
- `repair_root_artifact_id`
- `repair_parent_artifact_id`
- `target_artifact_id`
- `target_artifact_intake_ref`
- `excluded_runner_actor_id` optional; required for continuation after quota or operator budget exhaustion
- `max_repair_attempts=2`
- `network_policy=deny_all`
- `filesystem_policy=claim_workspace_read_only_input_write_output_only`
- `secret_policy=no_task_secrets`
- `max_wall_time_ms=8000`
- `max_cpu_time_ms=3000`
- `max_memory_bytes=268435456`
- `max_processes=1`
- `attestation_required=true`

Forbidden work-packet fields:

- arbitrary shell command;
- free-form command arguments;
- package install instruction;
- remote URL;
- Docker, VM, browser, or host automation instruction;
- provider credential reference;
- host path mount;
- prompt template supplied by central core;
- instruction to approve, publish, validate, or human-review the artifact.

## RepairAttempt Ledger

The central backend owns a durable `RepairAttempt` ledger. Runners never create or mutate attempt counters directly.

Core fields:

- `repair_attempt_id`
- `request_id`
- `proposal_id`
- `repair_root_artifact_id`
- `repair_parent_artifact_id`
- `source_artifact_id`
- `source_bundle_digest`
- `repair_attempt_index`
- `repair_continuation_index`
- `target_artifact_id`
- `target_artifact_intake_ref`
- `trigger_kind`
- `trigger_report_id`
- `state`
- `created_at`
- `closed_at` optional

Allowed `trigger_kind`:

- `accepted_code_critique`
- `trusted_validation_failure`
- `curator_requested_repair`
- `continuation_after_interruption`

Allowed `state`:

- `open`
- `claimed`
- `interrupted`
- `continued`
- `artifact_created`
- `stopped`
- `cancelled`

Uniqueness and idempotency:

- Unique logical attempt key: `(repair_root_artifact_id, repair_parent_artifact_id, repair_attempt_index)`.
- Unique continuation key: `(repair_attempt_id, repair_continuation_index)`.
- Exactly one `target_artifact_id` and `target_artifact_intake_ref` are preallocated per logical attempt.
- Accepted repair output closes the attempt as `artifact_created` and prevents later continuation or duplicate artifact creation for that logical attempt.
- Interruption closes the current work packet and records the attempt as `interrupted`; continuation changes the prior attempt record to `continued` and creates a new work packet with incremented `repair_continuation_index`.
- Transactional creation of repair work packet, target IDs, and attempt ledger is required.
- Replays with the same idempotency key return the same attempt/work-packet IDs; changed replay is rejected.

## Sandbox Profiles

### `python_checker_code_critique_v1`

Allowed command shape:

```text
python3 -I /runner-runtime/python_checker_code_critique_harness.py --bundle /workspace/input --report /workspace/output/code_critique_report.json
```

Allowed behavior:

- parse `checker.py` with AST/static rules at least as strict as spec `010`;
- compare function contract to the MVP validator contract;
- run public sample cases only through runner-owned harness code;
- check for forbidden filesystem, environment, network, subprocess, dynamic import, reflection, concurrency, and self-reporting behavior;
- emit structured safe findings and check results.

### `python_checker_code_repair_v1`

Allowed command shape:

```text
python3 -I /runner-runtime/python_checker_code_repair_harness.py --source /workspace/input --output /workspace/output --report /workspace/output/code_repair_report.json
```

Allowed behavior:

- create a complete replacement artifact bundle candidate in `/workspace/output`;
- modify only `checker.py` in the MVP;
- preserve all non-code bundle files byte-for-byte unless a later reviewed spec allows broader repair;
- run the same sandboxed self-test checks as `python_checker_code_critique_v1`;
- emit `CodeRepairReport` and artifact digest metadata.

Shared sandbox rules:

- `/runner-runtime` is read-only runner-owned code.
- `/workspace/input` is read-only source artifact intake.
- `/workspace/output` is the only writable output mount.
- No other host paths are mounted.
- Network is denied.
- Environment contains no provider credential, central API token, claim token, SSH agent, cloud credential, browser profile, home directory, or shell history.
- Process runs as non-root where supported.
- Privilege escalation is disabled where supported.
- Capabilities are dropped where supported.
- Seccomp/AppArmor or platform equivalent is enabled where available.
- Root filesystem is read-only except declared workspace mounts where supported.
- Wall time, CPU time, memory, process count, stdout, and stderr limits are enforced.
- Raw stdout/stderr are not submitted or persisted.

Platform fallback:

- If the host cannot enforce the required sandbox profile, the runner must not execute `checker.py`.
- Critique may submit static-only findings with `sandbox_status=not_run_sandbox_unavailable`.
- Repair may submit `CodeRepairReport` only when it has a complete repaired artifact proposal. If it cannot produce one, it must submit `CodeRepairInterruptionReport` and create no artifact.
- `repair_proposed_static_only` is allowed only when the repair changed `checker.py` using static transformation and did not execute generated code; trusted validation is still required.
- Trusted validation remains required before any repaired artifact can become `machine_validated`.

## CodeCritiqueReport

`CodeCritiqueReport` is a closed JSON object. Unknown fields are rejected. String IDs use the prefixes shown in examples and must otherwise be opaque ASCII identifiers from server-derived context.

Required fields:

- `critique_report_id`
- `work_packet_id`
- `lease_id`
- `runner_actor_id`
- `source_artifact_id`
- `source_bundle_digest`
- `execution_policy`
- `execution_profile_id`
- `allowed_command_id`
- `sandbox_status`
- `outcome`
- `reviewed_files`
- `checks`
- `findings`
- `attestation`
- `created_at`

Allowed `sandbox_status`:

- `enforced`
- `not_run_policy_disabled`
- `not_run_sandbox_unavailable`
- `not_run_runner_config_disabled`

Allowed `outcome`:

- `no_code_findings`
- `blocking_code_findings`
- `nonblocking_code_findings`
- `repair_recommended`
- `invalid_input`

Allowed reviewed files:

- `checker.py`

Finding object:

- closed object; unknown fields rejected;
- required fields: `finding_type`, `severity`, `safe_location`, `safe_message`, `evidence_kind`;
- optional fields: `repair_hint_code`;
- `finding_type`: `unsafe_import`, `filesystem_access`, `network_access`, `subprocess_execution`, `dynamic_code_execution`, `reflection_or_introspection`, `concurrency`, `contract_mismatch`, `sample_case_failure`, `self_reported_success`, `nondeterminism`, `style_or_maintainability`;
- `severity`: `critical`, `major`, `minor`, `note`;
- `safe_location`: structured pointer such as `checker.py:function:score_answer` or `checker.py:line:12`; no source text, Markdown, HTML, JSON tool-call text, prompt-like instruction, URL, local path, or secret-like value;
- `safe_message`: bounded inert text, no raw code snippet, Markdown, HTML, JSON tool-call text, prompt-like instruction, URL, local path, secret, prompt text, or raw stderr;
- `evidence_kind`: `static_ast`, `sandbox_sample_case`, `schema_check`, `digest_check`;
Allowed `repair_hint_code`:

- `remove_forbidden_import`
- `replace_io_with_pure_function`
- `match_expected_function_contract`
- `fix_sample_case_logic`
- `remove_self_reported_success`
- `make_deterministic`
- `manual_review_recommended`

`repair_hint_code` is advisory only.

Check result object:

- closed object; unknown fields rejected;
- required fields: `check`, `status`;
- optional fields: `safe_reason_code`;
- `status`: `passed`, `failed`, or `not_run`;
- `safe_reason_code` required when status is not `passed` and forbidden when status is `passed`.

Required checks:

- `source_digest_verified`
- `sandbox_profile_enforced`
- `checker_static_safety`
- `checker_function_contract`
- `checker_sample_cases`
- `no_network_observed`
- `no_filesystem_escape_observed`
- `no_secret_env_present`
- `raw_output_redacted`

Rules:

- Submitted `outcome` is evidence only.
- The central backend may derive deterministic routing from closed findings and validation failure codes.
- The report cannot create `Review`, cannot satisfy human review quorum, cannot mark findings resolved, and cannot move an artifact to `machine_validated`, `review_requested`, `peer_reviewed`, or any public label.

## CodeRepairReport

`CodeRepairReport` is a closed JSON object for artifact-producing repair proposals only. Unknown fields are rejected.

Required fields:

- `repair_report_id`
- `work_packet_id`
- `lease_id`
- `runner_actor_id`
- `source_artifact_id`
- `source_bundle_digest`
- `repair_attempt_id`
- `repair_attempt_index`
- `repair_continuation_index`
- `repair_root_artifact_id`
- `repair_parent_artifact_id`
- `target_artifact_id`
- `target_artifact_intake_ref`
- `execution_policy`
- `execution_profile_id`
- `allowed_command_id`
- `sandbox_status`
- `repair_status`
- `changed_files`
- `source_file_digests`
- `repaired_file_digests`
- `repaired_bundle_digest`
- `checks`
- `attestation`
- `created_at`

Optional fields:

- `source_critique_report_id`, omitted only when validation-triggered repair has no critique report.

Allowed `repair_status`:

- `repair_proposed`
- `repair_proposed_static_only`

Allowed `sandbox_status`:

- `enforced`
- `not_run_static_only`

Required repair checks:

- `source_digest_verified`
- `target_ids_match_work_packet`
- `changed_files_limited`
- `non_code_files_preserved`
- `checker_static_safety`
- `checker_function_contract`
- `checker_sample_cases`
- `no_network_observed`
- `no_filesystem_escape_observed`
- `no_secret_env_present`
- `repaired_digest_computed`
- `raw_output_redacted`

Rules:

- `changed_files` must be exactly `["checker.py"]` for MVP repair proposals.
- `target_artifact_id` and `target_artifact_intake_ref` must exactly match the preallocated values in the repair work packet.
- `source_file_digests` is a closed object with exactly the five MVP filenames from spec `013`; values use spec `013` digest strings and must match source artifact intake bytes and `source_bundle_digest`.
- `repaired_file_digests` is a closed object with exactly the five MVP filenames from spec `013`; values use spec `013` digest strings and must match repaired output bytes.
- `repaired_bundle_digest` follows spec `013` digest rules with added canonical fields `repair_attempt_id`, `repair_root_artifact_id`, `repair_parent_artifact_id`, `repair_attempt_index`, `repair_continuation_index`, and `target_artifact_id`.
- The repaired artifact bundle must be complete and must include unchanged non-code files with identical digests to the source artifact.
- The central backend rejects a repair report whose source artifact, source bundle digest, critique report, lease, runner actor, repair attempt index, execution policy, or signature does not match server-derived context.
- A successful repair submission creates a new `Artifact` in `draft_generated` with `repair_parent_artifact_id` and `repair_attempt_index`; it does not modify or promote the source artifact.
- The source artifact may be marked `deprecated` or remain as-is only by deterministic policy or curator action; runner output cannot deprecate it directly.

No-artifact repair terminal cases use `CodeRepairInterruptionReport`, not `CodeRepairReport`.

## Attestation and Provenance

Critique, repair, and interruption reports require Ed25519 attestations following spec `013`.

Report attestation rule:

- the signature is over the RFC 8785 canonical JSON bytes of the entire closed report object with the `attestation` field omitted;
- `signed_payload_digest` is the spec `013` SHA-256 digest string over those canonical bytes;
- central recomputes the canonical report payload and rejects if any signed field changed.

This means all routing-sensitive fields are signed, including:

- `outcome`;
- `sandbox_status`;
- `repair_status`;
- `interruption_reason`;
- `safe_summary_code`;
- source and target artifact IDs;
- repair attempt and continuation indexes;
- checks;
- findings;
- source and repaired digests.

Rules:

- Signature never replaces active lease authorization.
- Signature never replaces trusted validation.
- Central recomputes source and repaired digests before accepting provenance.
- Reports signed by the wrong runner key, stale key, wrong lease, wrong source artifact, wrong attempt index, or wrong bundle digest are rejected.
- Raw code excerpts, raw prompts, raw model responses, stdout/stderr, Markdown links, HTML, JSON tool-call text, local paths, URLs, provider metadata, and secrets are forbidden in submitted reports.

## Central API Behavior

Claiming:

- `critique_generated_code` and `repair_generated_code` work packets use the generic work-packet claim, heartbeat, release, and submit pattern from spec `008`.
- The actor must have matching runner role, scope, capability summary, and active key registration.
- Only one active lease is allowed per critique or repair work packet.
- Repair work-packet release, expiry, or revocation before output returns the work packet to `open` and returns the associated `RepairAttempt` to `open` without incrementing `repair_attempt_index` or `repair_continuation_index`.
- Once an interruption report is accepted, normal release/expiry recovery is unavailable for that work packet; continuation requires `RepairContinuationDecision`.

Submission:

- Critique output uses `kind=code_critique_output_v1` and contains exactly `code_critique_report`.
- Repair output uses `kind=code_repair_output_v1` and contains exactly `code_repair_report`, `artifact_bundle_reference`, and `provenance`.
- Repair interruption output uses `kind=code_repair_interruption_output_v1` and contains exactly `code_repair_interruption_report`.
- Repair `artifact_bundle_reference` uses the closed spec `008` shape with the preallocated target artifact ID and target intake reference from the repair work packet.
- Repair `provenance` is closed and contains `source_file_digests`, `repaired_file_digests`, `repaired_bundle_digest`, and `code_repair_report`.

Central acceptance guards:

- active lease and claim token are valid;
- authenticated actor matches lease holder and registered signing key;
- submitted lineage fields match server-derived work packet context;
- source artifact state is eligible;
- source bundle digest matches central intake copy;
- repair attempt limit is not exceeded;
- schema, field allowlists, redaction checks, and idempotency checks pass;
- signed attestation verifies;
- forbidden fields are absent.

Side effects:

- accepted critique persists `CodeCritiqueReport` as advisory internal evidence;
- accepted critique may deterministically open one repair work packet when outcome/finding rules match repairable code issues and attempt limits permit;
- accepted repair creates a new draft artifact candidate and validation work packet;
- accepted interruption consumes the lease, creates no artifact, and routes to `runner_operator`, curator, admin review, or bounded continuation;
- no critique or repair output changes public label, human-review status, or trusted validation state.

## Deterministic Routing Rules

Central routing from critique is closed and deterministic.

Repair work packet creation requires all of:

- source artifact is `draft_generated` or `validation_failed`;
- source artifact is not `quarantined` or `deprecated`;
- either curator requested repair or automated repair-loop gates pass;
- non-code safety precondition is satisfied;
- critique report is accepted and signed, or trusted validation failed with a repairable code reason code;
- critique outcome is `blocking_code_findings` or `repair_recommended` when critique-triggered;
- at least one finding type or validation reason code is in the repairable set:
  - `unsafe_import`
  - `contract_mismatch`
  - `sample_case_failure`
  - `self_reported_success`
  - `nondeterminism`
  - `style_or_maintainability` only when severity is `minor` or `note`;
- repair attempt count for the artifact lineage is below 2.
- interruption continuation count for the repair attempt is below 2.

Quarantine recommendation requires curator/system policy when:

- finding severity is `critical` and finding type is `network_access`, `filesystem_access`, `subprocess_execution`, or `dynamic_code_execution`;
- or central deterministic unsafe-content checks from spec `010` fail.

No central semantic merge, rewrite, ranking, or model-based judgment is allowed.

## Human Review Boundary

Critique and repair evidence may be visible to human reviewers as internal context only after redaction.

Rules:

- critique findings cannot satisfy `ReviewTask`;
- repair reports cannot satisfy `ReviewTask`;
- a runner cannot approve its own repaired artifact for `peer_reviewed`;
- human reviewers see central-derived artifact state and safe critique summaries, not runner-submitted authority fields;
- accepted human review is still required before `peer_reviewed`.

## Runner Behavior

New runner modes:

- `dummy_code_critic`
- `dummy_code_repairer`
- `dummy_code_repairer_auto_loop`

Deferred modes:

- provider-backed code critic;
- provider-backed code repairer;
- multi-file repairer;
- repository-scale reviewer;
- dependency updater;
- browser/web research reviewer.

MVP dummy critic:

- claims one critique work packet;
- reads the source artifact bundle from claim workspace;
- verifies source digest;
- runs `python_checker_code_critique_v1` when sandbox is available;
- emits a deterministic `CodeCritiqueReport`;
- signs the report.

MVP dummy repairer:

- claims one repair work packet;
- reads source artifact and accepted critique evidence;
- modifies only `checker.py` using deterministic fixture logic;
- computes source and repaired digests;
- runs `python_checker_code_repair_v1` when sandbox is available;
- submits a complete replacement artifact bundle reference and signed `CodeRepairReport`.

Automated repair-loop runner requirements:

- local config default is `automated_repair_loop_opt_in=false`;
- when enabled, capability summary may advertise only redacted opt-in booleans and coarse attempt limits;
- runner may stop accepting automated repair work at any time;
- runner must not continue local repair attempts after central rejects or exhausts the loop;
- runner must not chain local retries into new unreported repair attempts;
- runner must submit `CodeRepairInterruptionReport` before releasing a repair task it cannot complete because of local quota, provider availability, sandbox availability, suspected looping behavior, suspected malicious task content, or local operator policy.

## Acceptance Tests

### CR-001: Critique cannot be requested by text

Submit request, artifact, or critique text asking the central backend to create privileged review, repair, shell, package, browser, or validator tasks.

Expected:

- text remains inert;
- central creates critique or repair work only from deterministic state/routing rules;
- no execution policy is raised by submitted content.

### CR-002: Code critique report is advisory only

Submit signed critique with `outcome=no_code_findings` and approval-like text.

Expected:

- report persists as advisory evidence;
- artifact state and public label do not change;
- human review remains required.

### CR-003: Critique sandbox is enforced

Run dummy critic against generated `checker.py`.

Expected:

- allowed command ID maps to exact critique harness command;
- no network, secrets, claim token, provider credential, home directory, or host path is available;
- raw stdout/stderr is redacted.

### CR-004: Repair creates new draft artifact

Run dummy repairer after repairable critique.

Expected:

- original artifact is not mutated;
- new artifact is `draft_generated`;
- repaired bundle has new digest and parent repair lineage;
- validation work packet is created for the repaired artifact.

### CR-005: Repair attempt limit is enforced

Submit repair proposals beyond the per-lineage limit.

Expected:

- excess repair work packet is not created or submit is rejected;
- state does not loop indefinitely;
- safe reason code is emitted.

### CR-005A: Automated repair requires both opt-ins

Submit requests and runner capability summaries covering all combinations of teacher preference and runner opt-in.

Expected:

- no automated repair work packet is created when teacher preference is absent;
- no automated repair work packet is created when no eligible runner has opted in;
- when both opt-ins exist, central may create repair work only after deterministic repairable critique or validation reason codes;
- request text alone cannot enable the loop.

### CR-005B: Automated repair loop stops at safe terminal states

Run automated repair through success, repeated validation failure, quarantine, and human-review-open scenarios.

Expected:

- success creates a repaired draft artifact and validation work packet;
- repeated failure stops at `exhausted_attempts`;
- quarantine stops at `stopped_for_quarantine`;
- human review opening stops at `stopped_for_human_review`;
- no hidden background repair work continues after a terminal loop state.

### CR-005C: Runner quota exhaustion routes to review and safe continuation

Run an automated repair work packet where the runner exhausts local quota before producing a repaired artifact.

Expected:

- runner submits signed `CodeRepairInterruptionReport` with safe reason code only;
- no exact quota, provider account, prompt, model response, local path, secret, or partial artifact bytes are submitted;
- current repair work packet becomes `interrupted`;
- no artifact is created;
- central opens `runner_operator`, curator, or admin review metadata;
- continuation on another opted-in runner is possible only through closed central outcome `continue_on_another_runner`;
- continuation is bounded and cannot create an infinite loop.

### CR-006: Repaired artifact must pass trusted validation

Submit repaired artifact with runner self-test passed and attempt to mark it `machine_validated`.

Expected:

- artifact remains `draft_generated`;
- trusted validator path is required;
- runner report cannot create `ValidationReport.trusted_passed`.

### CR-007: Signature and lineage replay fail

Replay a critique, repair, or interruption report across another artifact, lease, runner, attempt index, continuation index, or bundle digest. Also alter critique `outcome`, repair `repair_status`, and interruption `interruption_reason` while keeping the old signature.

Expected:

- central rejects mismatch;
- central rejects tampered signed routing fields;
- no advisory evidence or draft repair artifact is accepted for the target.

### CR-008: Repair cannot broaden changed files

Submit repair report or bundle changing `worksheet.md`, `answer_key.md`, `teacher_notes.md`, or `manifest.json`.

Expected:

- submit is rejected for MVP;
- source artifact remains unchanged;
- no repaired draft artifact is created.

### CR-009: Human review cannot be laundered

Submit critique or repair report with human approval fields, recommended next state, reviewer identity, or peer-reviewed label.

Expected:

- forbidden fields are rejected;
- no `Review` record is created;
- artifact does not become `peer_reviewed`.

### CR-010: Unsafe report content is rejected or redacted

Submit fake secrets, local paths, URLs, prompts, provider responses, raw code snippets shorter than 160 characters, tool-call JSON, Markdown/HTML links, and raw stderr in critique/repair/interruption fields including `safe_location`, `safe_message`, and interruption summaries.

Expected:

- forbidden fields or unsafe values are rejected before persistence;
- logs and error bodies contain only safe field names and reason codes.

### CR-011: Non-code safety failures stop automated repair

Submit an artifact with a repairable `checker.py` failure plus unsafe worksheet or teacher-notes content, PII, bad manifest/license, path/MIME failure, or quarantine-eligible content.

Expected:

- automated repair work is not created;
- loop status becomes `declined_by_policy` or `stopped_for_quarantine` according to deterministic reason priority;
- non-code files are not copied into a repaired artifact;
- curator/quarantine handling is used where required.

## Security Checklist Coverage

Applicable categories from spec `002`:

- Central-Core Boundary Checks: central routes deterministically and does not review or repair semantically.
- Runner Submission Trust Checks: critique and repair are untrusted until schema, lineage, signature, and policy checks pass.
- Runner Sandbox and Tool-Execution Checks: code execution is limited to named sandbox profiles.
- Identity, Authorization, and Replay Checks: reports are lease-bound and signed.
- Artifact and Generated-Code Checks: repaired code is a new artifact and must pass trusted validation.
- Human Review and Publication Checks: critique/repair cannot approve or publish.
- Data Classification Checks: reports use closed safe fields and redact unsafe output.

## Review Checklist

Reviewers should fail this spec if:

- runner critique can create validation or human-review authority;
- repair mutates an existing artifact in place;
- central backend performs semantic review, repair, inference, prompt construction, or model-provider calls;
- execution policy can be raised by request/artifact/runner text;
- repair loops are unbounded;
- reports can be replayed across artifact, lease, runner, or attempt lineage;
- signed provenance is optional for accepted critique/repair reports;
- raw code, stderr, prompts, local paths, URLs, provider metadata, or secrets can be persisted;
- a repaired artifact can skip trusted validation before human review.
