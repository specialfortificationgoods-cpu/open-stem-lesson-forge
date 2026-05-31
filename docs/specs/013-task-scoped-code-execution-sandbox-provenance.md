# 013. Task-Scoped Code Execution, Sandbox, and Provenance Spec

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
- `docs/specs/012-mvp-guardrail-end-to-end-test-matrix.md`
- `docs/security/security-audit-001-runner-transport-ingestion-moderation.md`

## Purpose

This spec defines how MVP runners may generate code and run narrow self-tests without allowing user requests, model output, or runner-submitted fields to grant arbitrary execution authority.

The central rule is:

- requests are never trusted for execution;
- tasks may carry centrally assigned execution policy;
- runners may generate code as untrusted artifact text;
- runners may self-test only when a claimed work packet contains an explicit sandboxed self-test policy;
- runner self-test results are provenance evidence only and never replace trusted validator results.

## Research Inputs

This spec adopts these best-practice patterns from established systems:

- self-hosted runner systems treat untrusted code as a persistent compromise risk unless jobs are isolated and credentials are scoped per job;
- per-job or per-task tokens should be short-lived and least-privilege;
- sandboxed execution should combine process, filesystem, network, user, capability, and resource isolation;
- provenance should bind artifacts to the producer, instructions, materials, and build/test environment;
- attestations should be structured, verifiable claims over artifact digests rather than free-form logs.

Implementation notes:

- SLSA/in-toto-style provenance is the model for structured subject digests and predicate data.
- Kubernetes-style restricted security context is the deployment analogy for non-root, no privilege escalation, read-only root, dropped capabilities, seccomp/AppArmor where available, and explicit resource limits.
- GitHub Actions self-hosted runner guidance is the warning model: do not run untrusted code with long-lived secrets or a reusable host context.

## Execution Policy Is Task-Scoped

`ExecutionPolicy` is a central-core field attached to work packets, not requests.

MVP enum values:

- `no_execution`
- `code_generation_only`
- `sandboxed_self_test_python_checker`
- `trusted_validator_execution`

Spec `014` extends this enum for code critique and repair runner work. Those additions inherit this spec's rule that execution policy is centrally assigned and cannot be raised by request, artifact, or runner text.

Only these task types may receive non-`no_execution` policy:

| Task type | MVP execution policy |
|---|---|
| `moderate_request` | `no_execution` |
| `propose_task_graph` | `code_generation_only` only if future provider-backed planner spec allows local inference; MVP dummy remains no execution |
| `verify_proposed_task_graph` | `no_execution` |
| `generate_lesson_pack` | `sandboxed_self_test_python_checker` |
| `run_artifact_validation` | `trusted_validator_execution` |
| `review_subject_and_pedagogy` | `no_execution` |

Rules:

- The central backend assigns `ExecutionPolicy` during deterministic work-packet materialization.
- User request text, planner proposal text, model output, artifact manifest, and runner config cannot request or raise execution policy.
- A runner may refuse a task whose execution policy exceeds local configuration.
- A runner may lower local behavior below the central policy, for example generate code without self-testing.
- `trusted_validator_execution` is reserved for the validator boundary from spec `010` and cannot be claimed by normal runners.

## Work Packet Fields

Generation work packets that allow self-test include:

- `execution_policy=sandboxed_self_test_python_checker`
- `execution_profile_id=python_checker_self_test_v1`
- `allowed_command_id=python_checker_self_test_harness_v1`
- `network_policy=deny_all`
- `filesystem_policy=claim_workspace_read_write_output_only`
- `secret_policy=no_task_secrets`
- `max_wall_time_ms=3000`
- `max_cpu_time_ms=1000`
- `max_memory_bytes=134217728`
- `max_processes=1`
- `attestation_required=true`

Forbidden work-packet fields:

- arbitrary shell command;
- free-form command arguments;
- remote URL;
- package install instruction;
- Docker/VM control instruction;
- browser automation instruction;
- environment variable allowlist supplied by runner or request;
- provider credential reference;
- host path mount.

## Runner Sandbox Profile

MVP self-test sandbox profile: `python_checker_self_test_v1`.

Allowed action:

- run runner-owned self-test harness against generated `checker.py` functions before artifact submission.

Allowed command shape:

```text
python3 -I /runner-runtime/python_checker_self_test_harness.py --checker /workspace/output/checker.py
```

Rules:

- `/runner-runtime` is read-only runner-owned code.
- `/workspace/input` is read-only task input.
- `/workspace/output` contains generated bundle files.
- `/workspace/scratch` is writable scratch.
- No other host paths are mounted.
- Network is denied.
- Environment is minimal and contains no provider credentials, central API token, claim token, SSH agent, cloud credentials, browser profile paths, home directory, or shell history.
- Process runs as non-root where the platform supports it.
- Privilege escalation is disabled where the platform supports it.
- Capabilities are dropped where the platform supports it.
- Seccomp/AppArmor or platform equivalent is enabled where available.
- Root filesystem is read-only except explicit workspace mounts where the platform supports it.
- Wall time, CPU time, memory, process count, stdout, and stderr limits are enforced.
- Stdout/stderr are captured locally but not submitted raw if they contain unsafe values.

Platform fallback:

- If the host cannot enforce the sandbox profile, runner self-test is disabled and the runner submits `self_test_status=not_run_sandbox_unavailable`.
- The central backend may accept the artifact as draft evidence, but trusted validator execution remains required for `machine_validated`.

## Runner Self-Test Harness

The runner self-test harness is defense-in-depth and developer feedback. It is less trusted than the validator harness.

Required checks:

- parse `checker.py` with static constraints equivalent to or stricter than spec `010`;
- import exact pure functions expected by spec `010`;
- run the same public sample cases as the validator may run;
- reject self-printing success;
- reject filesystem, environment, network, subprocess, dynamic import, reflection, and concurrency attempts;
- emit a structured self-test report.

Self-test success does not:

- change artifact state;
- create validation report;
- satisfy `python_checker_runs`;
- skip deterministic validator;
- skip human review.

## Artifact Hashing

Before artifact reference submission, runner computes:

- `file_digests`: SHA-256 digest for each artifact file by canonical relative filename.
- `bundle_digest`: SHA-256 over canonical manifest:
  - sorted filenames;
  - file sizes;
  - file SHA-256 digests;
  - artifact bundle schema version;
  - generation work packet ID;
  - runner actor ID;
  - execution policy ID.

Rules:

- Hashes cover generated output bytes, not local paths.
- Hashes never include claim token, central auth token, provider credentials, local prompt text, raw model response, or host metadata.
- Validator recomputes file and bundle digests from its own intake copy.
- Digest mismatch fails artifact acceptance or validation.

Digest string format:

- `sha256:` followed by exactly 64 lowercase hexadecimal characters.
- Regex: `^sha256:[0-9a-f]{64}$`.

Canonical filename rules:

- File digest map keys are exact allowed bundle root filenames from spec `010`.
- No subdirectories are allowed in the MVP.
- Keys are sorted by raw ASCII byte order when constructing `bundle_digest`.
- Filename normalization and duplicate rejection follow spec `010` before digest computation.

File digest bytes:

- SHA-256 over the exact UTF-8 file bytes submitted in the artifact intake directory.
- No newline normalization, JSON reformatting, Markdown normalization, or path metadata is included.

Bundle digest canonical bytes:

- UTF-8 bytes produced by RFC 8785 JSON Canonicalization Scheme over the canonical bundle digest object below.
- Arrays stay in the exact order specified below; object keys are canonicalized by the scheme.
- Implementations must include golden test vectors for the MVP fixture before accepting digest/signature verification code.

Canonical bundle digest object:

```json
{
  "artifact_bundle_schema_version": "mvp-artifact-bundle-v1",
  "execution_policy": "sandboxed_self_test_python_checker",
  "file_digests": [
    {"path": "answer_key.md", "sha256": "sha256:<64-lower-hex>", "size_bytes": 0},
    {"path": "checker.py", "sha256": "sha256:<64-lower-hex>", "size_bytes": 0},
    {"path": "manifest.json", "sha256": "sha256:<64-lower-hex>", "size_bytes": 0},
    {"path": "teacher_notes.md", "sha256": "sha256:<64-lower-hex>", "size_bytes": 0},
    {"path": "worksheet.md", "sha256": "sha256:<64-lower-hex>", "size_bytes": 0}
  ],
  "runner_actor_id": "actor_generator_001",
  "work_packet_id": "wp_energy_001_generate_pack"
}
```

The `size_bytes` values are the decimal byte lengths of the exact file bytes. The example uses `0` placeholders only to show shape.

## Runner Self-Test Report

`RunnerSelfTestReport` is a closed JSON object. Unknown fields are rejected.

Required fields:

- `self_test_report_id`
- `work_packet_id`
- `lease_id`
- `runner_actor_id`
- `execution_policy`
- `execution_profile_id`
- `allowed_command_id`
- `sandbox_enforced`
- `self_test_status`
- `file_digests`
- `bundle_digest`
- `checks`
- `attestation`
- `created_at`

Optional fields:

- none in MVP.

Field rules:

| Field | Rule |
|---|---|
| `self_test_report_id` | Opaque ID with `rselftest_` prefix in fixtures; server may derive production ID. |
| `work_packet_id` | Claim checked against active generation lease. |
| `lease_id` | Must match active generation lease. |
| `runner_actor_id` | Evidence only; must match authenticated actor. |
| `execution_policy` | Must equal work packet policy. |
| `execution_profile_id` | `python_checker_self_test_v1`. |
| `allowed_command_id` | `python_checker_self_test_harness_v1`. |
| `sandbox_enforced` | Boolean. Must be `true` when `self_test_status=passed`. |
| `self_test_status` | Enum below. |
| `file_digests` | Object with exactly the five MVP filenames as keys and digest strings as values. |
| `bundle_digest` | Digest string. |
| `checks` | Array of closed check result objects. |
| `attestation` | Closed attestation object below. |
| `created_at` | RFC 3339 UTC timestamp with `Z`; no local timezone names. |

Allowed `self_test_status`:

- `passed`
- `failed`
- `not_run_policy_disabled`
- `not_run_sandbox_unavailable`
- `not_run_runner_config_disabled`

Allowed check IDs:

- `sandbox_profile_enforced`
- `checker_static_safety`
- `checker_function_contract`
- `checker_sample_cases`
- `no_network_observed`
- `no_filesystem_escape_observed`
- `no_secret_env_present`
- `digest_computed`

Check result object:

- closed object; unknown fields are rejected;
- required fields: `check`, `status`;
- optional fields: `safe_reason_code`;
- `check`: one allowed check ID;
- `status`: `passed`, `failed`, or `not_run`;
- `safe_reason_code`: bounded ASCII reason code from the central safe-code enum; required when status is not `passed` and forbidden when status is `passed`.

Duplicate check IDs are rejected. Unknown check IDs or statuses are rejected.

The report is submitted as part of generation output metadata. It is evidence only.

Forbidden fields:

- raw stdout/stderr;
- local absolute paths;
- provider credentials;
- central auth token;
- claim token;
- raw provider prompt or response;
- host username;
- process command line beyond `allowed_command_id`;
- environment dump.

## Attestation Strategy

MVP self-test provenance requires signed attestations.

Rules:

- each runner has a local Ed25519 signing key registered out-of-band with the central backend;
- central backend stores only runner public key, `runner_key_id`, owning `runner_actor_id`, and key status;
- self-test report includes signature over canonical payload;
- private key stays local and is never sent to central API;
- key rotation/revocation is centrally tracked;
- unsigned self-test reports are rejected outside explicitly named local unit fixtures that do not exercise central API acceptance.

Canonical attestation payload includes:

- `attestation_schema_version`;
- `runner_actor_id`;
- `runner_key_id`;
- `self_test_report_id`;
- `work_packet_id`;
- `lease_id`;
- `execution_policy`;
- `execution_profile_id`;
- `allowed_command_id`;
- `sandbox_enforced`;
- `bundle_digest`;
- `file_digests`;
- `checks`;
- `self_test_status`;
- `created_at`.

Attestation object:

| Field | Rule |
|---|---|
| `attestation_schema_version` | Must equal `runner-self-test-attestation-v1`. |
| `signature_kind` | Must equal `ed25519`. |
| `runner_key_id` | Opaque key ID registered to the authenticated runner actor; fixture prefix `rkey_`. |
| `signed_payload_digest` | SHA-256 digest string over canonical attestation payload bytes. |
| `signature` | Base64url without padding Ed25519 signature over the same canonical attestation payload bytes; regex `^[A-Za-z0-9_-]+$`. |

The canonical attestation payload is a closed JSON object serialized with the same canonical JSON rules as `bundle_digest`. Unknown payload or attestation fields are rejected.

Rules:

- Signature or MAC never replaces lease-token authorization.
- A valid signature from the wrong runner, wrong lease, wrong work packet, wrong policy, or stale key is rejected.
- The central backend recomputes `signed_payload_digest` and verifies the Ed25519 signature against the active registered key before persisting self-test provenance.

## Central Acceptance Rules

Central API accepts generation output with self-test metadata only when:

- generation work packet has `execution_policy=sandboxed_self_test_python_checker`;
- authenticated actor matches active generation lease;
- `lease_id` and `claim_token` are valid;
- self-test report references the same work packet and lease;
- execution policy/profile/command match the work packet;
- submitted digests are well-formed;
- signed attestation verifies against the registered active runner key;
- forbidden fields are absent;
- idempotency replay rules pass.

Central API rejects:

- self-test report for `no_execution` work packet;
- self-test report claiming a command not in the work packet policy;
- report with mismatched bundle digest or file list;
- report with raw stdout/stderr, local paths, env dumps, or secrets;
- report signed by wrong/revoked runner key;
- report replayed across tasks, leases, work packets, artifacts, or runners.

Accepted self-test metadata may be persisted as internal provenance. It does not alter artifact validation state.

## Validator Relationship

The trusted validator remains authoritative for `machine_validated`.

Rules:

- Validator recomputes digests from artifact intake.
- Validator ignores runner self-test success as validation authority.
- Validator may compare runner digest metadata to intake digest and record mismatch.
- Validator runs its own harness from spec `010`.
- Artifact can become `machine_validated` only after trusted validator report passes.

## MVP Dummy Generator Behavior

The MVP dummy generator must demonstrate code-generation runner behavior:

- generate deterministic `checker.py`;
- compute file and bundle digests;
- run `python_checker_self_test_v1` when local sandbox is available;
- submit `RunnerSelfTestReport`;
- submit artifact bundle reference and digest metadata;
- never execute generated code outside the sandbox;
- tolerate sandbox unavailable by submitting `not_run_sandbox_unavailable`.

The happy path should exercise `self_test_status=passed` under a local sandbox profile. CI may allow a fallback test where sandbox unavailability is detected and validator remains the authority, but the first release must have at least one automated environment proving the sandboxed self-test path.

## Acceptance Tests

### CODE-001: Request cannot grant execution

Submit request text asking the runner to run shell commands, enable tools, install packages, or execute code.

Expected:

- request text remains untrusted;
- central execution policy is unchanged;
- no work packet is created until moderation, planning, verification, and promotion gates pass.

### CODE-002: Work packet carries execution policy

Promote the accepted MVP task graph.

Expected:

- generation work packet has `sandboxed_self_test_python_checker`;
- validation work packet has `trusted_validator_execution`;
- planning, moderation, verification, and review tasks have no runner code-execution authority.

### CODE-003: Runner self-test runs only in sandbox

Run dummy generator self-test.

Expected:

- allowed command ID maps to exact harness command;
- no network is available;
- no central API token, claim token, provider credential, SSH key, cloud credential, or home-directory path is present in sandbox environment;
- filesystem access is limited to declared workspace mounts;
- resource limits are enforced.

### CODE-004: Sandbox unavailable disables self-test

Run on host profile without required sandbox controls.

Expected:

- runner reports `not_run_sandbox_unavailable`;
- generated code is not executed;
- artifact may be submitted as draft evidence;
- validator remains required for `machine_validated`.

### CODE-005: Hashes bind artifact bytes

Tamper with `checker.py`, `worksheet.md`, or manifest after digest computation.

Expected:

- digest mismatch is detected by runner before submit or by validator/central intake;
- no trusted validation state is created from stale digests.

### CODE-006: Self-test report cannot be replayed

Replay a valid self-test report across another work packet, lease, runner actor, execution policy, or artifact bundle.

Expected:

- central rejects lineage mismatch;
- no self-test evidence is accepted for the target task.

### CODE-007: Signature does not replace lease

Submit a correctly signed self-test report without a valid active lease token.

Expected:

- central rejects the report;
- signature is treated only as provenance integrity, not authorization.

### CODE-008: Runner self-test is not validator result

Submit self-test status `passed` and try to mark artifact `machine_validated`.

Expected:

- artifact remains `draft_generated`;
- trusted validator path is still required;
- public label does not change.

### CODE-009: Raw execution output is redacted

Make self-test fail with stderr containing fake secret, local path, and unsafe text.

Expected:

- raw stderr/stdout is not submitted or persisted;
- self-test report contains safe check IDs and reason codes only.

## Security Checklist Coverage

Applicable categories from `002`:

- Central-Core Boundary Checks: central assigns policy and verifies metadata but does not execute code.
- Runner Sandbox and Tool-Execution Checks: self-test is allowed only by task-scoped policy inside a sandbox profile.
- Runner Submission Trust Checks: self-test reports are evidence only.
- Artifact and Generated-Code Checks: generated `checker.py` is executed only in runner self-test sandbox or trusted validator sandbox.
- Identity, Authorization, and Replay Checks: reports are lease-bound, actor-bound, task-bound, and idempotent.
- Data Classification Checks: hashes and attestations exclude secrets, paths, prompts, and raw execution output.
- Logging, Error, and Audit Checks: failures use safe reason codes.

No applicable non-deferrable checklist item is deferred for MVP code-generation runner self-tests. Provider-backed code generation and advanced tool execution remain deferred.

## Review Checklist

Reviewers should fail this spec if:

- request text can grant execution authority;
- runner self-test can run outside the sandbox;
- sandbox can access network, host paths, central tokens, provider credentials, or home-directory secrets;
- runner self-test success can replace validator success;
- self-test reports are not bound to task, lease, actor, execution policy, and artifact digest;
- signatures or hashes are treated as authorization;
- raw stdout/stderr, local paths, prompts, provider responses, or secrets can be persisted;
- tampered artifact bytes can keep old self-test provenance;
- sandbox unavailable silently falls back to host execution.
