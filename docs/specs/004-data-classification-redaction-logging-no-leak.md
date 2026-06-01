# 004. Data Classification, Redaction, Logging, and Error No-Leak Spec

Status: Passed adversarial review  
Roadmap: `docs/specs/000-spec-roadmap.md`  
Depends on:

- `docs/specs/001-mvp-slice-acceptance-tests.md`
- `docs/specs/002-security-privacy-abuse-resistance-checklist.md`
- `docs/specs/003-architecture-stack-no-inference-boundary.md`

## Purpose

This spec defines what data may be accepted, persisted, logged, returned, and published by the deterministic Rust MVP. It is the no-leak contract for the central core, runner submissions, validator outputs, review records, audit events, API errors, and public artifact metadata.

The default posture is strict:

- Unknown fields are rejected unless a later spec explicitly allows them.
- Secret and forbidden values are rejected or redacted before any central persistence.
- Logs, errors, audit events, and public metadata use allowlists.
- Rejected payload storage must not keep raw secret-like values.

## Data Classes

| Class | Meaning | Central handling |
|---|---|---|
| Public | Safe for public pages, public API responses, and artifact downloads. | May be returned publicly. |
| Internal | Needed for deterministic workflow operation. | May be persisted internally; not public by default. |
| Private | Operational detail about users, runners, reviewers, or system internals. | Persist only when required; redact from public output and most logs. |
| Secret | Credential or credential-equivalent value. | Reject or redact before central persistence, logs, errors, audit events, or public output. |
| Forbidden | Data the MVP must not process. | Reject, quarantine, or strip according to deterministic policy; never use to drive workflow. |

## Secret and Forbidden Value Definitions

Secret values include:

- API keys and tokens.
- OAuth refresh/access tokens.
- Browser cookies and session cookies.
- Passwords.
- Codex `auth.json` contents.
- Paths to Codex `auth.json`, browser profiles, cookie stores, SSH keys, API key files, or OS credential stores.
- Provider account identifiers that identify private accounts.
- Provider base URLs unless a later reviewed spec creates an explicit safe disclosure field.
- Local model service secrets.
- Local prompt templates.
- Exact private quota/account details.
- Local filesystem paths outside safe workspace metadata.
- Session secrets, API tokens, and lease tokens.

Forbidden values include:

- Named student records.
- Student grades, placement decisions, profiles, or disciplinary records.
- Medical, legal, financial, or security-exploitation tasks.
- Credential-handling tasks.
- Provider-account-management tasks.
- Executable prompt fields intended for model execution.
- Hidden network requirements.
- Full runner provider configuration.

Secret-like values must be detected by both field name and value pattern where feasible. Pattern checks are a defense-in-depth layer; field allowlists remain authoritative.

## Attacker-Controlled Text Rule

Every attacker-controlled string, list item, filename-like value, object key, JSON pointer segment, and free-text field is untrusted even when its containing field is allowlisted.

Before central persistence and before public output, those values must be:

- Length-limited.
- Checked for secret-like values.
- Checked for forbidden student PII and forbidden workflow content.
- Rejected, quarantined, or redacted according to deterministic policy.
- Stored and displayed as inert escaped text.

This applies at least to:

- Request `title` and `constraints`.
- Runner public names.
- Proposed graph `source_request_summary`, `assumptions`, and `missing_information`.
- Artifact manifest `title`, `contents`, and `known_limitations`.
- Validation `safe_message`.
- Critique and review finding `message`.
- Review notes or future public metadata text.

Central code must never parse free-text fields as workflow authority, must never render them as raw executable HTML/Markdown, and must never copy them into central prompts.

## Accepted Redaction Outcomes

When a submitted payload includes a secret or forbidden value, central code may store only:

- A stable structured error code.
- A safe schema-derived field name or JSON pointer after the secret value is removed.
- A redaction marker such as `[REDACTED_SECRET]`.
- A hash-free classification such as `secret_like_value`.
- A safe policy reason code.

Central code must not store:

- Raw secret value.
- Hash of the secret value.
- Prefix/suffix of the secret value.
- Raw local path.
- Raw cookie or token.
- Raw rejected payload containing the secret.
- Raw attacker-supplied object key, JSON pointer segment, filename, or path-like manifest key.

For unknown or rejected fields, central code may store only a generic code such as `unknown_field`, plus a non-sensitive schema location or category. It must not store the raw unknown key/path because keys and path segments are attacker-controlled.

## Surface Allowlists

### Request Intake

Allowed request fields (caller-supplied):

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

Stored / system-derived request fields:

- `id`
- `status`
- `created_at`
- `updated_at`

The API must reject caller payloads containing server-owned request fields: `id`, `status`, `created_at`, or `updated_at`.

Request text fields are untrusted data. They may contain user-provided educational intent, but must not contain student PII.

Prompt-injection-like prose inside allowed request text fields may be stored as inert data unless deterministic intake rules reject it. Storing that prose must not change central policy, create work packets directly, bypass review gates, or become executable prompt material.

Disallowed at request intake:

- Student names with records or grading data.
- Provider credentials.
- Prompt fields intended to control runners or central policy.
- Attachment URLs for central fetching in MVP.
- Local filesystem paths.

### Runner Capability Summary

Allowed fields:

- `runner_id`
- `runner_public_name`
- `capability_schema_version`
- `capabilities.subjects`
- `capabilities.languages`
- `capabilities.phases`
- `capabilities.task_types`
- `capabilities.workflow_capabilities`
- `capabilities.artifact_types`
- `capabilities.tools`
- `trust_level`
- `policy_summary.max_tasks_per_day_bucket`
- `policy_summary.auto_submit_status_cap`
- `policy_summary.allowed_risk_level_max`

Disallowed fields:

- API keys.
- Auth mode details.
- Environment variable names containing provider credentials.
- Provider base URLs.
- Exact model account identifiers.
- Exact private quota values.
- Local auth paths.
- Full provider config.
- Local prompt templates.
- Local filesystem paths except safe workspace-relative paths created for an active task.

Exact quota values should be bucketed, for example `0`, `1-5`, `6-20`, `21+`, rather than transmitted as precise private account telemetry.

### ProposedTaskGraph

Allowed top-level fields:

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

Allowed proposed task fields:

- `local_id`
- `phase`
- `task_type`
- `subject`
- `topic`
- `age_range`
- `language`
- `risk_level`
- `depends_on`
- `required_capabilities`
- `outputs`
- `validation_required`
- `human_review_required_for`

Disallowed anywhere in proposed graphs:

- `prompt`
- `arbitrary_prompt`
- `system_prompt`
- `developer_prompt`
- `provider_prompt_template`
- `provider_config`
- `api_key`
- `auth_path`
- `base_url`
- `cookie`
- `local_path`
- `credential`
- `student_grading`
- `student_placement`
- `provider_account_management`

### PlanVerification and CritiqueReport

PlanVerification allowed fields:

- `verification_id`
- `proposal_id`
- `verification_task_id` where applicable
- `verifier_runner_id`
- `verification_type`
- `status`
- `outcome`
- `findings`
- `checked_items`
- `authority`
- `created_at`

CritiqueReport allowed fields:

- `critique_id`
- `proposal_id` where applicable
- `artifact_id` where applicable
- `runner_id`
- `critique_type`
- `status`
- `outcome`
- `findings`
- `checked_items`
- `authority`
- `created_at`

Spec `014` code critique, repair, and interruption reports use their own closed field allowlists. They inherit this spec's no-leak rules and additionally forbid exact quota values, provider account details, prompts, raw model responses, local paths, URLs, raw stdout/stderr, and partial artifact bytes.

Finding fields:

- `severity`
- `finding_type`
- `location`
- `message`
- `recommended_next_state`

Finding `message` is untrusted free text. It must be length-limited, redacted, and never treated as executable instruction.

Disallowed:

- Model prompts.
- Full model responses containing prompt traces or credentials.
- Provider config.
- Local paths.
- Cookies.
- API keys.
- Claims of human approval.
- Claims that override central validation, verification, promotion, or review state.

### Artifact Manifest

Allowed fields:

- `artifact_id`
- `request_id`
- `work_packet_ids`
- `title`
- `subject`
- `topic`
- `age_range`
- `language`
- `license`
- `status_claim`
- `ai_assisted`
- `contents`
- `known_limitations`
- `created_at`

`status_claim` is runner-submitted metadata. It must not raise central state.

Disallowed:

- Provider credentials.
- Provider base URL.
- Local filesystem paths.
- Local prompt templates.
- Exact private model account details.
- Claims that override central validation/review state.
- External scripts or network requirements.

### ValidationReport

Allowed fields:

- `validation_report_id`
- `artifact_id`
- `validator`
- `status`
- `checks`
- `failures`
- `created_at`

Allowed check fields:

- `check`
- `status`
- `safe_message`
- `safe_location`

Validation reports from the trusted deterministic validator path may move central state. Runner-submitted reports may be stored only as untrusted evidence if a later spec allows it.

Submitted `status`, `outcome`, `recommended_next_state`, and equivalent state-like fields are non-authoritative unless produced by the deterministic core or by the trusted deterministic validator path defined for the relevant state transition. They may be stored only as claims/evidence after validation and redaction.

Disallowed:

- Raw generated-code stdout/stderr if it contains secret-like data.
- Full stack traces containing host paths.
- Environment variable dumps.
- Provider config.
- Credentials.

### Human Review

Human-review record fields:

- `review_id`
- `artifact_id`
- `review_task_id`
- `reviewer_id`
- `review_types`
- `outcome`
- `findings`
- `recommended_next_state`
- `created_at`

`reviewer_id`, accepted review authority, and promotion authority are system-bound fields derived from authenticated review context and active review claims. They must not be trusted from an arbitrary submitted body.

Review notes/findings are untrusted free text. They must be length-limited and redacted.

Disallowed:

- Provider credentials.
- Local paths.
- Private model account details.
- Student PII.
- Claims that bypass configured promotion rules.

### Audit Events

Allowed fields:

- `event_id`
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
- `request_id` where relevant
- `proposal_id` where relevant
- `artifact_id` where relevant

Audit events must not store raw request bodies, raw rejected payloads, secret values, local paths, cookies, tokens, provider config, prompt templates, or student PII.

### Logs

Central logs must be structured and allowlisted.

Allowed log fields:

- timestamp
- level
- event name
- request ID
- actor type
- safe actor ID
- entity type
- entity ID
- status code
- safe reason code
- latency bucket

Disallowed log fields:

- Raw request/response body.
- Raw rejected payload.
- Authorization headers.
- Cookies.
- API keys.
- Local filesystem paths.
- Provider config.
- Prompt templates.
- Student PII.

### API Errors

Error responses must include:

- `error.code`
- `error.message` using safe generic text.
- `error.field` only if it is a safe field path.
- `error.request_id`

Error responses must not include:

- Raw rejected values.
- Secret-like values.
- Stack traces.
- Local paths.
- SQL errors with internal schema details.
- Provider config.
- Prompt text.

### Public Artifact Metadata

Allowed public fields:

- `artifact_id`
- `title`
- `subject`
- `topic`
- `age_range`
- `language`
- `license`
- `ai_assisted`
- `central_status`
- `validation_status`
- `review_status`
- `known_limitations`
- `contents`
- `created_at`
- `updated_at`

Public metadata must not expose:

- Runner private configuration.
- Provider base URL.
- Exact private model account.
- Local paths.
- Internal reviewer notes not marked public.
- Internal audit events.
- Secret-like values.

## Redaction Timing

Redaction must occur before:

- Central persistence.
- Audit event creation.
- Structured logging.
- Error response construction.
- Public response construction.
- Publication metadata generation.

If redaction cannot safely preserve a record, reject or quarantine the submission and store only safe policy metadata.

## Rust Type Requirements

Rust implementation must represent classified data deliberately:

- Public response types must be distinct from internal database types.
- Error response types must be distinct from internal error types.
- Audit event types must accept only safe reason codes and safe field paths.
- Newtypes should be used for sensitive identifiers where useful.
- Fallible constructors or validators must reject unsafe fields before persistence.
- `Debug` output for structures that may temporarily hold submitted payloads must not be logged before redaction.

## Acceptance Tests

Later implementation must include tests for:

### DT-001: Secret-like runner capability fields are rejected or redacted

Submit a runner capability summary containing:

```text
OPENAI_API_KEY=sk-test-secret
auth_path=/Users/alice/.codex/auth.json
provider_base_url=http://localhost:11434
local_prompt_template=/Users/alice/prompts/planner.md
exact_quota=47
```

Expected:

- No raw value is persisted.
- No raw value appears in logs, audit events, errors, or public output.
- The response contains only safe structured error codes or accepted bucketed fields.

### DT-002: Prompt fields are rejected anywhere in runner-submitted objects

Submit `prompt`, `system_prompt`, `developer_prompt`, `provider_prompt_template`, and `arbitrary_prompt` fields inside proposed graphs, artifact manifests, validation reports, critique reports, and review notes.

Expected:

- The submitted object is rejected or quarantined.
- The prompt values are not centrally persisted.
- The error response contains safe field paths and reason codes only.

### DT-003: Artifact status claim cannot override central status

Submit an artifact manifest with `status_claim=classroom_ready`.

Expected:

- The manifest is either rejected or accepted only as draft metadata.
- Central status remains determined by validation/review state.
- Public metadata does not show `classroom_ready`.

### DT-004: Validation output redacts unsafe stdout/stderr

Run validation against a checker or bundle that attempts to print fake secrets, environment variables, local paths, or tokens.

Expected:

- Validation report stores only safe failure summaries.
- Raw stdout/stderr with secret-like values is not persisted.
- Logs and API responses remain clean.

### DT-005: Free text is inert, escaped, and redacted

Submit prompt-injection text, HTML/script, Markdown links, fake secrets, local paths, and named student records inside request constraints, proposed graph assumptions, validation messages, known limitations, review findings, and critique findings.

Expected:

- Unsafe values are rejected, quarantined, or redacted according to deterministic policy.
- Stored/displayed text is inert and escaped.
- Text is not rendered as executable HTML or trusted Markdown.
- Text is not parsed for workflow decisions.
- Text is not copied into central prompts.
- No workflow state changes because of free-text instructions.

### DT-006: Review notes cannot leak secrets or student PII

Submit review findings containing fake provider keys, local paths, cookies, and named student records.

Expected:

- The review is rejected, redacted, or quarantined according to deterministic policy.
- Raw unsafe values do not appear in review records, audit events, logs, errors, or public output.

### DT-007: Error path does not leak rejected values

Submit malformed payloads containing fake secrets in unknown fields.

Expected:

- API errors include safe codes and safe field paths.
- API errors do not echo rejected values.
- Internal logs and audit events do not include rejected raw values.

### DT-008: Attacker-controlled keys and paths do not leak

Submit fake secrets, local paths, cookies, and student names as object keys, JSON pointer segments, filenames, path-like manifest keys, and `contents` entries.

Expected:

- Central records store only generic safe codes such as `unknown_field` or schema-derived locations.
- Raw attacker-controlled keys, path segments, filenames, and manifest keys do not appear in errors, logs, audit events, validation reports, review records, or public output.
- Unsafe `contents` entries are rejected or redacted before persistence and publication.

### DT-009: Public artifact metadata is allowlisted

Publish metadata for draft, machine-validated, and peer-reviewed artifact states.

Expected:

- Public output includes only allowed public fields.
- Public output includes AI assistance, license, validation state, review state, and known limitations.
- Public output excludes internal runner, reviewer, provider, path, prompt, audit, and secret fields.

### DT-010: Audit events are useful without raw payloads

Trigger request rejection, proposed graph rejection, validation failure, review rejection, and promotion denial.

Expected:

- Audit events include actor, action, entity, states, timestamp, and safe reason code.
- Audit events do not include raw rejected payloads or secret-like values.

### DT-011: Submitted state claims are non-authoritative

Submit proposed graph, plan verification, critique, validation report, artifact manifest, and review payloads that claim elevated states such as:

- `status=schema_policy_validated`
- `status=verified_for_mvp_promotion`
- `status=passed`
- `outcome=approved_for_peer_reviewed`
- `recommended_next_state=peer_reviewed`
- `status_claim=classroom_ready`

Expected:

- Submitted fields may be rejected or stored only as non-authoritative claims/evidence after validation and redaction.
- Central state does not change unless the deterministic state-transition rule and authenticated/authorized actor context allow it.
- Public metadata reflects central state, not submitted claims.

### DT-012: Forged validation report cannot create machine validation

Submit a runner-controlled report with `validation_report.status=passed`, `validator=deterministic_validator`, or equivalent forged fields through an artifact bundle, artifact metadata, or unauthorized validation surface.

Expected:

- The report is rejected or stored only as untrusted evidence.
- The artifact remains `draft_generated`.
- Public metadata does not show deterministic validation.
- `machine_validated` requires the trusted deterministic validator path.

### DT-013: Forged reviewer identity cannot create authoritative review

Submit a review payload from a runner or unqualified actor with:

```text
reviewer_id=human_reviewer_physics_001
outcome=approved_for_peer_reviewed
recommended_next_state=peer_reviewed
```

Expected:

- `reviewer_id` from the body is not trusted as authority.
- No authoritative review record is created unless authenticated review context and active review claim match.
- The artifact does not promote to `peer_reviewed`.

## Security Checklist Coverage

Applicable categories from `002`:

- Central-Core Boundary Checks: this spec forbids prompt/config/secret persistence in central surfaces.
- Data Classification Checks: directly satisfied by data classes, surface allowlists, and redaction timing.
- Request Intake and Prompt-Injection Checks: satisfied for request field classification and prompt-field rejection; detailed request workflow belongs to `006`.
- Runner Submission Trust Checks: satisfied for runner capability summaries and runner-submitted status claims; detailed runner config belongs to `009`.
- Planning and Promotion Abuse Checks: satisfied for prompt/config forbidden fields; detailed policy belongs to `007`.
- Artifact and Generated-Code Checks: satisfied for manifest/report no-leak rules; validator execution belongs to `010`.
- Human Review and Publication Checks: satisfied for review/public metadata no-leak rules; review workflow belongs to `011`.
- Logging, Error, and Audit Checks: directly satisfied.
- Provider-Terms and Public Framing Checks: provenance field exposure is restricted here; public copy belongs to later web/onboarding specs.

No applicable non-deferrable checklist item is deferred for data classification, redaction, logging, or errors.

## Review Checklist

Reviewers should fail this spec if:

- Any central surface can persist raw secret-like values.
- API errors can echo rejected secret-like values.
- Logs or audit events can retain raw rejected payloads.
- Runner-submitted status claims can raise central state.
- Public artifact metadata is not allowlisted.
- Prompt-like fields can be stored as executable central data.
- Rust public response types are allowed to reuse internal records without a redaction boundary.
