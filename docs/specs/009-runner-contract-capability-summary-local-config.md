# 009. Runner Contract, Capability Summary, and Local Configuration Spec

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

## Purpose

This spec defines the local runner contract for the Rust MVP: local configuration, redacted capability summaries, task claim/heartbeat/release/submit behavior, dummy planner/verifier/generator modes, provider-adapter boundaries for future local inference, workspace isolation, and no-leak rules.

The runner is the inference-capable layer. The central backend remains deterministic. A runner may use local rules, templates, local models, or provider adapters in later versions, but it may send only structured, schema-valid outputs and redacted capability summaries to the central API.

## Runner Roles

Runner roles are local modes, not central authority:

- `request_moderator`: claims `RequestModerationTask` and submits `RequestModerationReport`.
- `planner`: claims `PlanningTask` and submits `ProposedTaskGraph`.
- `plan_verifier`: claims `PlanVerificationTask` and submits `PlanVerification`.
- `generator`: claims generation `WorkPacket` and submits `ArtifactBundleReference`.
- `review_assistant`: may produce `CritiqueReport` in later specs, but cannot approve human review.
- `code_critic`: claims spec `014` code critique work packets and submits signed advisory code critique reports.
- `code_repairer`: claims spec `014` code repair work packets and submits signed repaired draft candidates.
- `packager`: prepares local artifact bundle layout before deterministic validation.

MVP required modes:

- `dummy_request_moderator`
- `dummy_planner`
- `dummy_plan_verifier`
- `dummy_generator`

Spec `014` preview modes are optional and non-MVP until a later gate activates
the central code-critique and code-repair ingestion API surfaces:

- `dummy_code_critic`
- `dummy_code_repairer`
- `dummy_code_repairer_auto_loop`

Deferred modes:

- deterministic validator client or service integration outside the runner role;
- real provider-backed planner;
- real provider-backed generator;
- provider-backed critique/repair runner;
- translation/localization runner;
- browser or web-fetch runner;
- student-facing tutor;
- project-funded inference service.

Deferred modes must be unavailable or rejected until their own specs pass.

Trusted validation is not a normal runner role in the MVP. It is performed by a separately configured `system_validator` actor through the dedicated validation-work API paths from spec `008`.

## Local Configuration File

Default path:

- `lessonforge.runner.toml` in the runner working directory.

The config is local private data. It is never uploaded wholesale.

MVP config shape:

```toml
[runner]
runner_id = "actor_planner_001"
public_name = "Dummy Planner"
mode = "dummy_planner"
scope_id = "scope_default"
central_api_base = "https://127.0.0.1:8443"
workspace_root = ".lessonforge-runner-work"

[auth]
kind = "bearer_token_env"
token_env = "LESSONFORGE_RUNNER_TOKEN"

[transport.tls]
trust_policy = "loopback_development"
pinned_ca_pem_path = ""
pinned_spki_sha256 = ""
expected_server_name = ""

[attestation]
runner_key_id = ""
ed25519_private_key_path = ""
runner_private_key_dir = ""

[capabilities]
subjects = ["physics"]
languages = ["en"]
phases = ["request_planning"]
task_types = ["propose_task_graph"]
workflow_capabilities = ["request_interpretation", "task_decomposition", "policy_reasoning"]
artifact_types = ["worksheet", "answer_key", "python_checker", "teacher_notes"]
tools = ["structured_json_output"]
max_risk_level = "low"

[policy]
max_tasks_per_day_bucket = "1-5"
auto_submit_status_cap = "draft_only"
allowed_risk_level_max = "low"
allow_provider_backed_modes = false
allow_tool_execution = false
automated_repair_loop_opt_in = false
max_automated_repair_attempts = 0
```

Allowed auth kinds for MVP:

- `bearer_token_env`
- `test_static_token` only under test profile.

Deferred auth kinds:

- `mtls_files`, until a later spec defines exact TOML fields, certificate/key/CA path constraints, file permission checks, peer validation, and redacted summary behavior.

Forbidden config fields:

- provider API key literal;
- provider base URL in transmitted summaries;
- local prompt template content in transmitted summaries;
- browser profile path;
- cookie path;
- Codex auth path;
- exact quota/account values;
- local filesystem paths outside `workspace_root`, except `runner_private_key_dir` and an `ed25519_private_key_path` under its canonical directory as constrained by the attestation rules below;
- shell command templates;
- tool/function-call execution templates;
- hidden network destinations.

TLS config fields:

- `trust_policy`: `loopback_development`, `pinned_ca`, `pinned_spki`, or future `mtls`.
- `pinned_ca_pem_path`: required only for `pinned_ca`; must be a local path readable by the runner and never transmitted.
- `pinned_spki_sha256`: required only for `pinned_spki`; base64url or lowercase hex SHA-256 digest of the expected server public key info.
- `expected_server_name`: required for non-loopback DNS origins; must match the configured origin host and certificate identity.

Validation rules:

- `loopback_development` is allowed only for loopback HTTP/HTTPS under explicit local/test profile.
- Non-loopback origins require `pinned_ca`, `pinned_spki`, or a future reviewed `mtls` policy.
- `pinned_ca_pem_path` and `pinned_spki_sha256` are local private config and must not appear in capability summaries, central API payloads, logs, or errors.
- Empty string fields are allowed only when not required by the selected `trust_policy`.

Attestation config rules:

- `runner_key_id` is the central-registered key ID used in spec `013` self-test attestations.
- `ed25519_private_key_path` is local private config and must not appear in capability summaries, central API payloads except by signature result, logs, or errors.
- `runner_private_key_dir` is optional local private config for storing runner private keys outside `workspace_root`.
- `runner_key_id` and `ed25519_private_key_path` are required for runner modes that submit signed provenance, including `dummy_generator`.
- `runner_key_id`, `ed25519_private_key_path`, and `runner_private_key_dir` must be empty for runner modes that do not submit signed provenance.
- When `runner_private_key_dir` is present, it must be an absolute non-symlink directory path that exists, is readable by the runner process, and is not readable by other local users under platforms that expose owner/group/other mode bits.
- The private key file must be under `workspace_root` or under the configured `runner_private_key_dir`; if `ed25519_private_key_path` is outside `workspace_root`, the runner must reject the config unless the canonical key path is under the canonical `runner_private_key_dir`.
- The central API receives only `runner_key_id`, digest fields, and the Ed25519 signature described in spec `013`.

Automated repair config rules from spec `014`:

- `automated_repair_loop_opt_in` defaults to `false`.
- Only `dummy_code_repairer_auto_loop` or future reviewed repair modes may set it to `true`.
- `max_automated_repair_attempts` must be `0` when opt-in is false and may be `1` or `2` when opt-in is true.
- The capability summary may report only the boolean opt-in and coarse attempt bucket, never local paths, prompts, provider settings, or exact private quotas.
- Runner opt-in is not authority. The central backend still assigns every repair work packet and may decline automated repair.

Unknown config fields are rejected by default.

## Central API Origin Validation

`central_api_base` is the only outbound API origin allowed in the MVP runner config.

Allowed values:

- HTTPS origin for a configured trusted Lesson Forge central API.
- HTTP origin only when all are true:
  - explicit test or local profile;
  - host is loopback literal `127.0.0.1`, `::1`, or `localhost`;
  - no provider-backed mode is enabled.

Rejected values:

- non-loopback `http://`;
- URL with username/password;
- URL with path/query/fragment;
- provider or local-model service base URL;
- wildcard host;
- DNS name not present in trusted origin config for non-loopback deployment;
- extra outbound destination fields.

Transport rules:

- TLS certificate validation is required for HTTPS.
- Non-loopback runner deployments must configure an explicit TLS trust policy: either a pinned private CA, a pinned server certificate/SPKI hash, or an mTLS profile from a later passed spec.
- Platform trust without a pin is allowed only for local development or a later deployment spec that accepts that risk.
- TLS peer name must match configured `central_api_base`; IP literals require explicit local/test profile or pinned certificate identity.
- Configured mTLS peer validation is deferred with `mtls_files`.
- Redirects are disabled by default.
- If redirects are later enabled, auth and claim tokens must not be forwarded across scheme, host, or port changes.
- Ambient proxy environment variables are ignored by default.
- Proxy use requires an explicit future reviewed config field; no MVP proxy forwarding.
- Runner must never send central auth token or claim token to an origin other than validated `central_api_base`.

## Local Secret Handling

Secrets may exist only in local runner process memory or OS secret storage.

Allowed local secret references:

- environment variable name for central API auth token;
- mTLS certificate/key paths when mTLS is configured;
- future provider credential references only in provider specs, never in central summary.

Rules:

- Do not print secret values.
- Do not include secret values in panic/debug output.
- Do not send secret values to central API.
- Do not hash or fingerprint secret values for central submission.
- Do not write provider prompts, credentials, or raw model transcripts to central-visible workspace.
- Redact secrets before local logs unless logs are disabled.

MVP dummy modes do not require provider credentials.

## MVP Dummy Actor Independence

The MVP uses distinct runner actor IDs for planner, verifier, and generator roles:

- `actor_planner_001`
- `actor_verifier_001`
- `actor_generator_001`

One local binary may support all dummy modes, but each running configuration must bind to exactly one runner actor identity and one active mode. The verifier actor must differ from the planner actor for plan verification. The generator actor does not gain reviewer or verifier authority.

## Redacted Capability Summary

The runner may submit or expose this central-safe summary:

```json
{
  "runner_id": "actor_planner_001",
  "runner_public_name": "Dummy Planner",
  "capability_schema_version": "1.0",
  "capabilities": {
    "subjects": ["physics"],
    "languages": ["en"],
    "phases": ["request_planning"],
    "task_types": [
      "propose_task_graph"
    ],
    "workflow_capabilities": [
      "request_interpretation",
      "task_decomposition",
      "policy_reasoning"
    ],
    "artifact_types": ["worksheet", "answer_key", "python_checker", "teacher_notes"],
    "tools": ["structured_json_output"],
    "automated_repair_loop_opt_in": false,
    "automated_repair_attempt_bucket": "0"
  },
  "trust_level": "runner_candidate",
  "policy_summary": {
    "max_tasks_per_day_bucket": "1-5",
    "auto_submit_status_cap": "draft_only",
    "allowed_risk_level_max": "low"
  }
}
```

Summary rules:

- Must match spec `004` runner capability allowlist.
- Must not contain credentials, provider base URLs, model names, local prompt templates, auth paths, local absolute paths, exact quotas, private account identifiers, cookies, or token names that reveal provider setup.
- `trust_level` is central-assigned or centrally accepted config, not self-granted authority.
- Capability summary is eligibility input only. It does not make runner output authoritative.
- Central API may reject or reduce capabilities regardless of runner claims.
- Planner claim checks evaluate `workflow_capabilities` and `tools` together under spec `006`; for example, MVP planning eligibility requires `request_interpretation`, `task_decomposition`, `policy_reasoning`, and `structured_json_output`.
- Planner-capable runners produce structured task graphs from requests; the central backend only schema-validates, policy-checks, verifies, and deterministically promotes or rejects those graphs.
- `automated_repair_loop_opt_in` is allowed only for `dummy_code_repairer_auto_loop` and future reviewed repair modes.
- `automated_repair_attempt_bucket` values are `0`, `1`, or `2`; exact private quota or budget is never reported.

Verifier and generator configs emit separate summaries with their own `runner_id`, `mode`, phases, task types, and tools.

Spec `014` code-review/repair summary values:

| Mode | `phases` | `task_types` | `tools` | Automated repair fields |
|---|---|---|---|---|
| `dummy_code_critic` | `["code_critique"]` | `["critique_generated_code"]` | `["structured_json_output", "sandboxed_python_checker_critique"]` | `automated_repair_loop_opt_in=false`, `automated_repair_attempt_bucket="0"` |
| `dummy_code_repairer` | `["code_repair"]` | `["repair_generated_code"]` | `["structured_json_output", "sandboxed_python_checker_repair"]` | `automated_repair_loop_opt_in=false`, `automated_repair_attempt_bucket="0"` |
| `dummy_code_repairer_auto_loop` | `["code_repair"]` | `["repair_generated_code"]` | `["structured_json_output", "sandboxed_python_checker_repair"]` | `automated_repair_loop_opt_in=true`, `automated_repair_attempt_bucket="1"` or `"2"` |

Unknown phases, task types, workflow capabilities, tools, or automated repair fields are rejected.

## Runner CLI Commands

MVP CLI commands:

```text
lessonforge-runner validate-config --config lessonforge.runner.toml
lessonforge-runner capability-summary --config lessonforge.runner.toml
lessonforge-runner list-claimable --config lessonforge.runner.toml --kind planning
lessonforge-runner list-claimable --config lessonforge.runner.toml --kind plan-verification
lessonforge-runner list-claimable --config lessonforge.runner.toml --kind generation
lessonforge-runner run-once --config lessonforge.runner.toml --kind planning
lessonforge-runner run-once --config lessonforge.runner.toml --kind plan-verification
lessonforge-runner run-once --config lessonforge.runner.toml --kind generation
```

Spec `014` preview CLI commands are optional and must fail against the MVP
central API until spec `014` ingestion surfaces are activated:

```text
lessonforge-runner list-claimable --config lessonforge.runner.toml --kind code-critique
lessonforge-runner list-claimable --config lessonforge.runner.toml --kind code-repair
lessonforge-runner run-once --config lessonforge.runner.toml --kind code-critique
lessonforge-runner run-once --config lessonforge.runner.toml --kind code-repair
```

CLI output:

- Human-readable summaries may be printed to stderr/stdout.
- JSON output mode may be added for tests if it redacts secrets.
- Claim tokens must never be printed in MVP commands.

The runner must exit non-zero on config validation failure, schema validation failure, central API rejection, unsafe field detection, or unsupported mode.

`list-claimable` is read-only and never creates a lease.

`run-once` owns the full claim lifecycle in one process:

- claim;
- hold claim token only in process memory;
- heartbeat while running if needed;
- submit or release;
- erase token from memory on completion where feasible.

Standalone heartbeat/release commands are not in the MVP because claim-token replay never re-displays tokens and this spec does not define persistent local token storage.

## Central API Interaction

The runner uses spec `008` endpoints:

- list claimable tasks;
- claim request moderation task;
- claim task;
- heartbeat while working;
- submit structured output;
- release on cancellation;
- never call promotion directly unless acting as an authorized test/system actor.

Request rules:

- Always send `Idempotency-Key` for mutating commands.
- Never put auth token, claim token, or secrets in query strings.
- Never retry mutation with changed payload under the same idempotency key.
- Treat `401`, `403`, `409`, `410`, and `422` as terminal for the current attempt unless a local policy says to claim a fresh task.
- On `410 lease_not_submitting`, discard local claim token and do not resubmit.

Local retry rules:

- Network failure before response: retry same request with same idempotency key.
- Timeout after response unknown: retry same request with same idempotency key.
- `409 idempotency_conflict`: stop and require operator intervention.
- `429`: back off according to server response or local exponential backoff, whichever is stricter.

Planner eligibility rule:

- For spec `006` request-to-task planning claims, the central backend evaluates the runner summary's `workflow_capabilities` plus `tools`; `phases` and `task_types` route candidate tasks but do not substitute for semantic planning capabilities such as `request_interpretation`, `task_decomposition`, and `policy_reasoning`.

Claim timeout rule:

- If timeout occurs after claim creation but before the runner receives `claim_token`, idempotent claim replay may return lease metadata without the token under spec `008`.
- In that case, the runner treats the lease as unrecoverable.
- The runner must not attempt heartbeat or submit for that lease.
- The runner waits for expiry or uses a future administrative recovery/release path.
- The runner may claim a different task or retry after the server makes the original task claimable again.

## Local Workspace Isolation

Each claimed task gets a task workspace:

```text
{workspace_root}/
  claims/
    {lease_id}/
      input/
      output/
      scratch/
      logs/
```

Rules:

- Workspace paths are local and private.
- Central API receives only opaque IDs or safe relative artifact names allowed by specs.
- Runner must not read files outside the claim workspace except explicit local config/secret references.
- Runner must not write generated artifacts outside the claim workspace.
- Clean up claim workspace after successful submit unless `--keep-workspace` is set for local debugging.
- `--keep-workspace` must not upload local paths or retained debug data to central API.

MVP dummy runner does not execute arbitrary shell commands.

## Runner Sandbox and Tool Execution Policy

Runner inputs are untrusted even when they arrive from the central API. Requests, work packets, proposed graph fields, artifact text, and model/provider outputs are data, not executable instructions.

MVP runner sandbox baseline:

- `allow_tool_execution=false` is required.
- Runners must not execute shell commands, scripts, package managers, browser automation, Docker/VM control, SSH agent operations, filesystem operations requested by task content, or model tool/function calls.
- Runners must not read outside the claim workspace except validated local config and secret references needed for central API auth.
- Runners must not expose the user's home directory, SSH keys, browser profiles, Codex auth files, shell history, Docker socket, cloud credentials, or unrelated provider credentials to task execution.
- Runners must not execute generated code except for spec `013` task-scoped sandboxed self-tests. Trusted generated-code execution belongs only to the validator boundary in spec `010`.
- Provider-backed future modes must disable model tool/function calling by default.

Future tool execution beyond spec `013` self-test harnesses requires a separate passed spec defining allowlisted commands, argument schemas, filesystem mounts, network policy, CPU/memory/process/time limits, audit events, and targeted security review.

## Local Prompt Templates and Provider Adapter Boundary

MVP dummy modes do not use model prompts.

Future provider-backed modes may use local prompt templates only if a later spec defines:

- provider adapter trait;
- prompt-template storage rules;
- transcript retention rules;
- redaction rules;
- allowed model/provider config;
- user consent and subscription posture;
- local rate/cost controls;
- no transmission of provider config to central API.

Until then:

- `allow_provider_backed_modes=false` is required for MVP tests.
- Runner must reject config that enables provider-backed modes.
- Runner must not load local prompt-template files.
- Runner must not call model providers.

Provider adapter boundary:

- Provider adapters may live only in runner/provider crates or plugins.
- Provider adapter dependencies must not enter `lessonforge_core`, `lessonforge_api`, `lessonforge_schema`, or deterministic validator crates.
- Provider outputs must be converted to schema-validated runner submissions before sending to central API.
- Raw provider responses and prompt traces are not sent to central API.

## Dummy Request Moderator Mode

`dummy_request_moderator` claims `RequestModerationTask` and emits the accepted request moderation fixture from spec `001`.

Input requirements:

- moderation task type is `moderate_request`;
- request subject/topic/age/language match the MVP physics fixture;
- required output schema is `request_moderation_report.schema.json`;
- runner has active moderation lease.

Output:

- `moderation_kind=dummy_fixture`;
- `decision=allow_mvp_planning` only for the accepted MVP fixture;
- `category_flags=["none"]`;
- `safe_reason_codes=["moderation_allowed"]`;
- no provider raw response;
- no provider/model fields;
- no prompt fields;
- no local paths;
- no URLs;
- no student data.

The dummy moderator cannot create planning tasks, approve artifacts, call model providers, or replace human review.

## Dummy Planner Mode

`dummy_planner` claims `PlanningTask` and emits the accepted proposed task graph fixture from spec `001` as amended by spec `007`, adjusted only for server-provided IDs when necessary.

Input requirements:

- planning task type is `propose_task_graph`;
- request summary matches MVP physics request;
- required output schema is `proposed_task_graph.schema.json`;
- runner has active planning lease.

Output:

- `ProposedTaskGraph` schema version `1.0`;
- `status=proposed`;
- required artifacts worksheet, answer key, python checker, teacher notes;
- tasks `t_generate_pack`, `t_validate_pack`, `t_human_review`;
- validation plan enum array from spec `007`, exactly:
  - `manifest_schema`
  - `required_files`
  - `license_metadata`
  - `ai_assistance_disclosure`
  - `obvious_pii_heuristic`
  - `obvious_inappropriate_content_heuristic`
  - `python_checker_runs`
  - `no_external_network_static`
- no prompt fields;
- no provider fields;
- no local paths;
- no URLs;
- no student data.

The dummy planner must not inspect free-text constraints as executable instructions. It may copy only allowed request summary fields and fixed fixture assumptions.

## Dummy Plan Verifier Mode

`dummy_plan_verifier` claims `PlanVerificationTask` and emits the accepted plan verification fixture from spec `001`.

Input requirements:

- verification task references a schema/policy-valid proposal;
- verifier runner actor differs from planner actor;
- runner has active verification lease.

Output:

- `verification_type=plan_schema_policy_cross_check`;
- `outcome=no_blocking_findings` only when the proposal exactly matches the MVP fixture requirements;
- `findings=[]` for valid fixture;
- blocking finding for missing review gate, forbidden task, wrong artifact set, wrong dependency graph, prompt field, provider field, or student-data field.

The dummy verifier cannot approve human review, cannot promote proposals, and cannot call model providers.

## Dummy Generator Mode

`dummy_generator` claims generation `WorkPacket` and creates a deterministic local artifact bundle.

Files:

```text
manifest.json
worksheet.md
answer_key.md
checker.py
teacher_notes.md
```

Requirements:

- Work packet task type is `generate_lesson_pack`.
- Work packet outputs match spec `007`.
- The runner writes only inside the claim workspace.
- The runner computes file and bundle digests from spec `013`.
- The runner runs the spec `013` sandboxed self-test harness when the work packet has `execution_policy=sandboxed_self_test_python_checker` and local sandbox support is available.
- The runner submits `RunnerSelfTestReport` as internal provenance evidence.
- The runner submits an `ArtifactBundleReference` from spec `008`.
- Artifact content is deterministic fixture content for conservation of energy.

`checker.py` in the dummy bundle:

- contains no network imports;
- does not read environment variables;
- does not read local files;
- does not use subprocess;
- includes deterministic sample checks only.

The dummy generator must not execute generated code outside the spec `013` self-test sandbox. Trusted execution belongs to the deterministic validator boundary in spec `010`.

## Manual, Assisted, Auto-Draft, and Review-Only Behavior

Runner local execution modes:

- `manual`: claim and prepare output locally, require operator confirmation before submit.
- `assisted`: allow local provider/model assistance only after future provider spec; require operator confirmation before submit.
- `auto_draft`: automatically submit draft runner outputs within configured risk/capability limits.
- `review_only`: never submit generation outputs; only produce local diagnostics or future critique reports.

MVP allowed modes:

- `manual`
- `auto_draft` for dummy modes only.

MVP rejected modes:

- provider-backed `assisted`;
- provider-backed `auto_draft`;
- browser/web-fetch;
- credential-handling;
- student-data processing.

`auto_draft` cannot bypass central schema/policy validation, plan verification, deterministic validation, or human review.

## Runner-Side Validation Before Submit

Before submission, runner must validate:

- output JSON shape against local schemas;
- no unknown fields;
- no forbidden prompt/provider/secret/local-path/URL fields;
- IDs match claimed task where known;
- no tool/function-call execution requests;
- output size within API limits;
- `claim_token` is present only in API request, not embedded in submitted object.

Runner-side validation is defense in depth. Central API remains authoritative and must validate again.

## Local Logs

Default runner logs:

- timestamp;
- log level;
- command;
- task kind;
- lease ID;
- safe state/result code;
- latency bucket.

Logs must not include:

- central auth token;
- claim token;
- provider credentials;
- provider prompts;
- provider responses;
- local absolute paths outside workspace root;
- raw rejected payloads containing secrets;
- student PII.

`--verbose` may include local file paths only under workspace root and only when output stays local. Verbose logs must not be submitted to central API.

## Capability Registration and Refresh

The runner may display or submit its redacted capability summary.

Registration rules:

- Central API may accept summary only through a future authenticated runner-registration endpoint, or test harness fixture.
- Until that endpoint exists, the capability summary is used by local tests and operator setup.
- Central trust level remains centrally configured; runner cannot self-upgrade.
- Runner status `paused` or `revoked` from central API must stop claim attempts.

Refresh rules:

- Capability summary can change only after config validation.
- A runner must not change public capabilities while holding active leases.
- If config changes during active lease, finish/release current lease before using new config.

## Acceptance Tests

### RUN-001: Dummy config validates and emits redacted capability summary

Run `validate-config` and `capability-summary` for the MVP dummy planner config.

Expected:

- Config validates.
- Summary includes allowed capability fields.
- Summary does not include auth token env value, provider config, prompt template, local absolute path, exact quota, or provider base URL.

### RUN-002: Config with provider secrets is rejected or kept local only

Add provider API key literal, provider base URL, auth path, cookie path, browser profile path, local prompt template, exact quota, and hidden network destination to config.

Expected:

- MVP config validation rejects unsupported provider-backed mode.
- Capability summary omits or rejects unsafe fields.
- No unsafe value is sent to central API.

### RUN-003: Dummy request moderator submits safe moderation fixture

Run `dummy_request_moderator` against the spec `001` request moderation task.

Expected:

- Runner claims request moderation task.
- Emits `RequestModerationReport` fixture with `decision=allow_mvp_planning`.
- Sends no provider raw response, prompt/provider/model fields, local paths, URLs, or student data.
- Performs no model-provider call.
- Central API can schema/lineage validate it before planning task creation.

### RUN-004: Dummy planner submits accepted fixture without inference

Run `dummy_planner` against the spec `001` planning task.

Expected:

- Runner claims planning task.
- Emits `ProposedTaskGraph` fixture.
- Sends no prompt/provider/model fields.
- Performs no model-provider call.
- Central API can schema/policy validate it.

### RUN-005: Dummy verifier blocks malformed proposals

Run `dummy_plan_verifier` against valid fixture and malformed proposals.

Expected:

- Valid fixture gets `no_blocking_findings`.
- Missing human review gate, forbidden task, prompt field, provider field, URL, local path, or student-data field gets blocking finding.
- Verification remains advisory and cannot count as human review.

### RUN-006: Dummy generator creates deterministic bundle reference

Run `dummy_generator` against generation work packet.

Expected:

- Files are created under claim workspace only.
- `ArtifactBundleReference` matches spec `008`.
- File digests, bundle digest, and `RunnerSelfTestReport` match spec `013`.
- Sandboxed self-test success is not submitted as trusted validation.
- No raw file paths are sent to central API.
- `checker.py` contains no network/env/file/subprocess behavior.

### RUN-007: Claim token is not logged or embedded

Claim any task, heartbeat, and submit output.

Expected:

- Claim token appears only in API request construction.
- Claim token does not appear in runner logs, submitted JSON object, capability summary, artifact files, or errors.

### RUN-008: Retry uses same idempotency key and unchanged body

Simulate timeout after submit.

Expected:

- Runner retries with same idempotency key and same non-secret payload.
- Changed retry under same key is not attempted.
- `409 idempotency_conflict` stops the runner.

### RUN-008A: Lost claim response does not recover token by replay

Simulate timeout after server creates a lease but before runner receives `claim_token`.

Expected:

- Claim replay returns lease metadata without token.
- Runner treats lease as unrecoverable.
- Runner does not heartbeat or submit with that lease.
- Runner waits for expiry or future administrative recovery.

### RUN-009: Stale lease stops work

Return `410 lease_not_submitting` for heartbeat or submit.

Expected:

- Runner discards claim token.
- Runner does not retry submit with stale lease.
- Runner may claim a new task only through normal claim flow.

### RUN-010: Workspace isolation blocks path leaks

Attempt to configure output paths outside workspace root or include absolute local paths in submitted artifact reference.

Expected:

- Runner rejects local path escape.
- Central submission contains only opaque artifact intake reference and safe filenames.

### RUN-011: Provider-backed mode is unavailable in MVP

Enable provider-backed planner/generator mode.

Expected:

- Runner exits non-zero with safe error.
- No provider SDK call occurs.
- No provider config is transmitted.

### RUN-011A: Unsafe central API origins are rejected

Configure `central_api_base` with non-loopback HTTP, URL credentials, path/query/fragment, provider/local-model URL, changed-origin redirect, failed TLS peer, missing required non-loopback TLS pin/trust policy, mismatched TLS peer name, or ambient proxy interception.

Expected:

- Config validation or connection setup rejects the origin.
- Auth token and claim token are not sent.
- Redirects do not forward credentials.
- Ambient proxy variables are ignored unless a future reviewed proxy config exists.

### RUN-011B: Task content cannot trigger local tool execution

Submit claim inputs, request constraints, proposed graph fields, artifact text, or simulated provider output asking the runner to run shell commands, read local files, exfiltrate environment variables, install packages, open a browser, call Docker, or execute tool/function-call JSON.

Expected:

- Runner treats the content as inert data.
- No shell, browser, package manager, Docker, filesystem, or tool/function call executes.
- Output is rejected locally or submitted only as schema-valid safe data.
- Logs contain safe reason codes only.

### RUN-012: Central API rejection is terminal for current attempt

Return `401`, `403`, `409`, `410`, or `422` from central API.

Expected:

- Runner does not mutate and resubmit the same task in a loop.
- Runner reports safe local error.
- Runner releases or discards local claim according to error class.

### RUN-013: Verbose local logs remain local

Run with `--verbose --keep-workspace`.

Expected:

- Verbose logs may include workspace-relative paths.
- Verbose logs do not include secrets, claim tokens, provider prompts, provider responses, or student PII.
- Verbose logs are not submitted to central API.

## Security Checklist Coverage

Applicable categories from `002`:

- Central-Core Boundary Checks: runner is outside deterministic core and central API remains no-inference.
- Data Classification Checks: capability summaries, logs, config, and submissions follow spec `004`.
- Runner Submission Trust Checks: runner output remains untrusted until central validation.
- Content Moderation and Age-Appropriateness Checks: dummy request moderator produces safe schema-only evidence outside the central core.
- Runner Sandbox and Tool-Execution Checks: task content and provider output cannot trigger shell/tool/code execution in MVP runners.
- Plan Verification Gate Checks: dummy verifier is advisory only.
- Identity, Authorization, and Replay Checks: runner uses leases, idempotency keys, and terminal stale-lease behavior.
- SSRF/Outbound URL Checks: runner allows only validated central API origin in MVP and rejects provider/local-model/redirect/proxy exfiltration paths.
- Provider Framing Checks: provider-backed modes are local/deferred and no provider config crosses to central API.
- Artifact and Generated-Code Checks: dummy generator executes generated code only through the spec `013` sandboxed self-test path; code critic and repairer execute generated code only through spec `014` sandbox profiles.
- Logging, Error, and Audit Checks: runner logs redact secrets and tokens.

No applicable non-deferrable checklist item is deferred for MVP dummy runner behavior.

## Review Checklist

Reviewers should fail this spec if:

- Full provider config, credentials, local prompt templates, provider base URLs, or exact quotas can cross into central API.
- Runner can self-grant trust or authority through capability summary.
- Dummy modes require real model/provider calls.
- Runner can create central state without API schema/policy validation.
- Plan verifier can count as human review or approve promotion by itself.
- Generator, code critic, or repairer can execute generated code outside the reviewed spec `013` or spec `014` sandbox profiles.
- Claim tokens can appear in logs, artifacts, capability summaries, or submitted objects.
- Local workspace can read/write outside the claim workspace.
- Provider-backed modes are enabled before their own reviewed spec.
- Runner retries can change payload under the same idempotency key.
- Runner can send auth or claim tokens to unvalidated origins, redirects, or ambient proxies.
- Lost claim responses can recover claim tokens by replay or continue without a token.
- Task content or model output can cause runner shell/tool/code execution in the MVP.
- Request moderation can call a provider from central core or send raw provider moderation responses to central API.
