# 010. Artifact Manifest, Validation, and Provenance Spec

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

## Purpose

This spec defines MVP artifact bundle layout, manifest schema, deterministic validation checks, validation report schema, provenance handling, generated-code constraints, and public provenance allowlists.

The validator is deterministic. It does not grade lesson quality with a model, call provider APIs, fetch URLs, run semantic similarity, infer curriculum standards, or promote artifacts by manifest self-claims.

## Artifact Intake Boundary

Artifact bytes enter validation only through a validator-controlled local intake directory associated with an opaque API reference such as `aintake_energy_001`.

Rules:

- Central API never accepts raw bundle bytes in specs `001` through `010`.
- `artifact_intake_ref` is an opaque ID, not a path, URL, archive name, or filename.
- Only the validator/artifact subsystem maps `artifact_intake_ref` to a local bundle directory.
- Mapping from `artifact_intake_ref` to local path is private validator state and never returned to API clients.
- The validator rejects any intake reference that is missing, expired, already consumed, cross-scope, or not associated with the submitted generation work packet.
- The validator must not fetch remote URLs.

## Bundle Layout

The MVP artifact bundle is an unpacked directory with exactly these files at the bundle root:

```text
manifest.json
worksheet.md
answer_key.md
checker.py
teacher_notes.md
```

No subdirectories are allowed in the MVP.

No extra files are allowed.

No symlinks, hard links, device nodes, sockets, FIFOs, hidden files, resource forks, or platform metadata files are allowed.

## Path Normalization

For every bundle entry:

- Normalize path using platform-independent UTF-8 relative path parsing.
- Reject absolute paths.
- Reject path components `.` and `..`.
- Reject backslashes as path separators.
- Reject drive-letter prefixes.
- Reject NUL bytes and control characters.
- Reject Unicode bidi controls and invisible path separators.
- Reject duplicate names after Unicode NFC normalization and ASCII case folding.
- Reject names longer than 80 UTF-8 scalar values.

Allowed root filenames are exact ASCII names from the bundle layout.

Validator errors may contain only allowed filename enum values or safe reason codes. Rejected raw filenames are not persisted.

## Size and MIME Limits

MVP limits:

| File | Max size | Expected kind |
|---|---:|---|
| `manifest.json` | 16 KiB | JSON object |
| `worksheet.md` | 64 KiB | UTF-8 Markdown text |
| `answer_key.md` | 64 KiB | UTF-8 Markdown text |
| `checker.py` | 32 KiB | UTF-8 Python source |
| `teacher_notes.md` | 64 KiB | UTF-8 Markdown text |
| Whole bundle | 256 KiB | directory total |

Validation rejects:

- binary files;
- invalid UTF-8;
- compressed or nested archive content;
- files exceeding limits;
- MIME/type mismatch where detectable by deterministic sniffing;
- embedded NUL bytes.

## Manifest Schema

`manifest.json` must be a JSON object with exactly:

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

Unknown fields are rejected.

Field rules:

| Field | Rule |
|---|---|
| `artifact_id` | Opaque ID claim matching server-derived artifact or submitted artifact reference. |
| `request_id` | Must match server-derived request lineage. |
| `work_packet_ids` | Array containing the generation work packet ID; no extras in MVP. |
| `title` | 1 to 120 UTF-8 scalar values, inert escaped text, no secret/PII. |
| `subject` | `physics`. |
| `topic` | `conservation_of_energy`. |
| `age_range` | `14-16`. |
| `language` | `en`. |
| `license` | `CC-BY-4.0`. |
| `status_claim` | Must be `draft_generated`; non-authoritative. |
| `ai_assisted` | Boolean, must be `true` for MVP dummy generated bundle. |
| `contents` | Exact array of allowed filenames, no extras. |
| `known_limitations` | 1 to 8 inert strings, max 240 UTF-8 scalar values each. |

`status_claim` never changes central artifact state. Public labels derive from central state, validation report, and review records.

## Markdown Content Rules

Markdown files must be UTF-8 text.

Allowed Markdown constructs:

- headings;
- paragraphs;
- bullet/numbered lists;
- fenced code blocks for inert examples;
- inline math-like plain text;
- tables.

Rejected Markdown constructs:

- raw HTML;
- `<script>` or event handlers;
- remote images;
- links to external URLs;
- local absolute paths;
- embedded data URLs;
- iframes;
- forms;
- hidden comments containing secret-like values;
- named student records.

The validator may parse Markdown conservatively using deterministic text scanning. It does not need to render Markdown.

## Python Checker Constraints

`checker.py` is the only executable-like file allowed in the MVP bundle.

Static constraints:

- UTF-8 Python source.
- No shebang required; if present, it must not contain local paths.
- No imports except `math` from the standard library.
- No `open`, `exec`, `eval`, `compile`, `__import__`, `input`, `globals`, `locals`, `vars`, `dir`, `getattr`, `setattr`, `delattr`.
- No `subprocess`, `socket`, `ssl`, `http`, `urllib`, `requests`, `pathlib`, `os`, `sys`, `shutil`, `tempfile`, `multiprocessing`, `threading`, `ctypes`, `importlib`, or dynamic import pattern.
- No environment reads.
- No filesystem reads/writes.
- No network calls.
- No process spawning.
- No top-level infinite loops detectable by simple static checks.

The static scanner may be conservative. Suspicious code is rejected.

## Checker Execution Policy

The MVP may choose one of two implementation modes:

### Static-Only Mode

If the sandboxed execution boundary from spec `003` is not implemented, validation must not execute `checker.py`.

Static-only mode may produce only a failed or incomplete validation report for the MVP slice. It may pass static checks, but it cannot produce overall `status=passed` because spec `001` requires `python_checker_runs`.

Static-only mode may record successful static checks only when:

- static constraints pass;
- required file and manifest checks pass;
- the validation report explicitly records `python_checker_runs=skipped_static_only`;
- validation report overall status is `failed` or `incomplete_static_only`;
- artifact state does not advance to `machine_validated`.

For the MVP happy path to reach `machine_validated`, sandboxed execution mode must be implemented and `python_checker_runs` must pass.

### Sandboxed Execution Mode

If execution is implemented, it must run outside the central API process with:

- no central database access;
- no central API tokens;
- no provider credentials;
- no inherited broad environment;
- no network access;
- filesystem access limited to a temporary copy of the bundle;
- read-only bundle inputs;
- writeable temp directory with size limit;
- CPU time limit;
- wall-clock time limit;
- memory limit;
- process/thread limit;
- stdout/stderr capture limit;
- deterministic sample cases only.

MVP execution command:

```text
python3 -I /validator-runtime/validator_checker_harness.py --checker /bundle/checker.py
```

Execution contract:

- `validator_checker_harness.py` is validator-owned code, not bundle content.
- Current working directory is an empty validator-controlled temp directory, not the bundle root.
- `/validator-runtime` is a read-only mount or equivalent validator-owned runtime path containing the harness.
- `/bundle` is a read-only mount or equivalent temporary copy containing only the validated bundle.
- The harness must reject any `--checker` path other than `/bundle/checker.py`.
- Standard input is empty and closed.
- Environment is empty except implementation-required deterministic variables such as `PYTHONHASHSEED=0` if the platform permits them under isolated mode.
- Pass requires harness exit code `0`, harness stdout exactly `OK\n`, and stderr empty.
- Nonzero exit, timeout, resource kill, unexpected stdout, any stderr, or sandbox violation fails `python_checker_runs`.
- Raw stdout/stderr is redacted before report construction and is not persisted.

MVP checker content contract:

- `checker.py` must define these exact pure functions:
  - `kinetic_energy(mass_kg: float, speed_m_per_s: float) -> float`
  - `gravitational_potential_energy(mass_kg: float, g_m_per_s2: float, height_m: float) -> float`
  - `speed_from_kinetic_energy(kinetic_energy_j: float, mass_kg: float) -> float`
- Top-level checker code may contain only function definitions, numeric constants, docstrings, and `import math`.
- Top-level calls, prints, loops, conditionals with side effects, file access, network access, environment access, subprocess use, and dynamic imports are rejected by static AST checks before execution.
- Checker functions must be deterministic and side-effect-free under the static restrictions.

Required conservation-of-energy sample cases:

| Case | Input meaning | Expected |
|---|---|---|
| `kinetic_energy(2.0, 3.0)` | mass 2 kg, speed 3 m/s | `9.0` joules |
| `gravitational_potential_energy(1.5, 9.8, 4.0)` | mass 1.5 kg, g 9.8 m/s^2, height 4 m | `58.8` joules |
| `speed_from_kinetic_energy(18.0, 4.0)` | KE 18 J, mass 4 kg | `3.0` m/s |

The validator-owned harness imports the checked module only after static AST checks pass, calls the exact functions with validator-owned sample cases, and compares numeric results with tolerance `1e-9`. Runner-controlled stdout or self-reported success is ignored and causes failure if emitted.

If the exact function contracts are absent, validation records `python_checker_runs=failed` and the artifact cannot become `machine_validated`.

Sandbox enforcement:

- Execution is enabled only when the implementation can enforce an OS sandbox, container, restricted subprocess profile, or equivalent platform mechanism that denies network access and limits filesystem access to the temporary bundle and a read-only Python runtime/stdlib allowlist.
- Python interpreter and required standard library files may be read from a validator-owned read-only runtime path.
- No user home directory, runner workspace, central API config, central database, OS credential stores, provider config, or environment secrets are visible.
- Static scanner bans network/filesystem/process imports before execution.
- Runtime sandbox must deny socket creation and file access outside the allowed runtime/bundle/temp paths.
- Timeout/resource violations kill the subprocess tree and produce safe failure summaries.

If the platform cannot enforce these runtime sandbox properties, execution mode is unavailable and static-only mode applies.

MVP resource limits:

| Resource | Limit |
|---|---:|
| Wall time | 2 seconds |
| CPU time | 1 second |
| Memory | 64 MiB |
| Processes | 1 |
| Stdout/stderr capture | 8 KiB each |
| Temp writes | 64 KiB |

Raw stdout/stderr is not persisted if it contains secret-like, path-like, URL-like, or PII-like values. Validation report stores only safe summaries.

## Validation Checks

The trusted validator performs exactly these MVP checks:

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
- `python_checker_static_safety`
- `python_checker_runs`
- `no_external_network_static`
- `public_provenance_allowlist`

Checks are deterministic and ordered as listed.

Failure of any required check maps the persisted `ValidationReport.state` to `trusted_failed`. Static-only validation that passes all executable-independent checks but skips checker execution maps to `trusted_incomplete_static_only`.

### Obvious Inappropriate Content Heuristic

The deterministic validator performs a conservative static screen over public text files before an artifact can become `machine_validated`.

Scope:

- `worksheet.md`
- `answer_key.md`
- `teacher_notes.md`
- `manifest.json` public text fields

Rejected content includes obvious sexual content, sexual content involving minors, graphic violence, self-harm instructions, hateful or harassing slurs, illicit instructions, weapon construction instructions, privacy invasion instructions, and age-inappropriate classroom content for the MVP `14-16` audience.

Rules:

- The check is deterministic and may use conservative phrase/blocklist patterns.
- False positives are acceptable; the artifact can be regenerated.
- The check must not call model providers or moderation APIs.
- If a later provider-backed artifact moderation gate is added, it must remain outside the central core and cannot replace this deterministic validator check.
- Matched raw unsafe text is not persisted in validation reports, logs, errors, or public metadata; reports use safe reason codes.

## Validation Report Schema

Spec `010` supersedes the shorter validation report fixture in spec `001`. The MVP validation report must contain exactly one `checks` record for each validation check listed in this spec, in the listed order. `failures` is the subset of failed check records.

Trusted validation report shape, with `checks` abbreviated for readability:

```json
{
  "validation_report_id": "vreport_energy_001",
  "artifact_id": "art_energy_001",
  "validator": "deterministic_validator",
  "validator_version": "1.0",
  "status": "passed",
  "checks": [
    {
      "check": "manifest_schema",
      "status": "passed",
      "safe_message": "manifest schema passed",
      "safe_location": "manifest.json"
    }
  ],
  "failures": []
}
```

Allowed report statuses:

- `passed`
- `failed`
- `incomplete_static_only`

Allowed check statuses:

- `passed`
- `failed`
- `skipped_static_only`

Wire status to core state mapping:

| Report `status` | `ValidationReport.state` | Artifact effect |
|---|---|---|
| `passed` | `trusted_passed` | May move `draft_generated -> machine_validated`. |
| `failed` | `trusted_failed` | May move `draft_generated -> validation_failed`. |
| `incomplete_static_only` | `trusted_incomplete_static_only` | Must not move to `machine_validated`; may move to `validation_failed` with safe reason `checker_execution_not_performed`. |

Only `report.status=passed` with every required check `passed` may create `ValidationReport.state=trusted_passed`. Any `failed` or `skipped_static_only` required check produces a non-passing report state and cannot open review.

Report rules:

- `validation_report_id` is generated or accepted according to spec `008` idempotency.
- `artifact_id` must match server-derived artifact lineage.
- `validator` must be `deterministic_validator`.
- `validator_version` must be a safe version string.
- `safe_message` is from static templates or safe reason codes.
- `safe_location` is one of allowed filenames or schema-derived locations.
- `failures` contains only safe check records.

Report must not include:

- raw stdout/stderr;
- stack traces;
- local paths;
- rejected raw filenames;
- secret-like values;
- URLs;
- provider data;
- prompt text;
- student PII.

## Lineage and Provenance

Validator input context is server-derived from:

- validation work packet;
- artifact record;
- generation work packet;
- promoted proposal;
- request;
- scope;
- system-validator actor.

Manifest IDs are claims checked against that lineage.

Mismatch rejects validation with safe code `artifact_lineage_mismatch` and cannot move artifact state.

Validator recomputes spec `013` file and bundle digests from artifact intake bytes when runner digest metadata is present.

Rules:

- Digest mismatch is recorded with safe code `artifact_digest_mismatch`.
- Digest mismatch prevents trusted validation success.
- Runner self-test reports are internal provenance evidence only.
- Runner self-test `passed` is ignored as validation authority.
- Validator harness execution from this spec remains required for `python_checker_runs`.

Provenance records may include internally:

- request ID;
- proposal ID;
- work packet IDs;
- generator actor ID;
- validator actor ID;
- validation report ID;
- runner self-test report ID;
- file digests;
- bundle digest;
- execution policy ID;
- review IDs after review;
- timestamps;
- safe source type `runner_generated`.

Internal provenance must not include:

- provider names;
- model names;
- provider account IDs;
- local prompt templates;
- local workspace paths;
- raw model transcripts;
- private reviewer notes;
- exact private quota/account details.

## Public Provenance Allowlist

Public artifact metadata may include:

- artifact ID;
- title;
- subject;
- topic;
- age range;
- language;
- license;
- AI assistance disclosure;
- central public label;
- validation state summary;
- review state summary;
- known limitations;
- safe generated-by category such as `runner_assisted`;
- safe validation category such as `deterministic_validator`;
- timestamps rounded or exact according to later publication spec.

Public metadata must not include:

- runner private config;
- provider/model names;
- local paths;
- prompt templates;
- raw validation logs;
- reviewer private notes;
- internal audit events;
- draft/private artifact existence before publication rules allow it.

## Artifact State Effects

Trusted validation report effects:

- `trusted_passed` may move artifact `draft_generated -> machine_validated`.
- `trusted_failed` may move artifact `draft_generated -> validation_failed`.
- `trusted_incomplete_static_only` may move artifact `draft_generated -> validation_failed` with safe reason `checker_execution_not_performed`.
- untrusted runner validation evidence cannot move artifact state.
- manifest `status_claim` cannot move artifact state.
- public label cannot exceed central artifact state.

Review task opening after validation is defined by specs `005`, `008`, and `011`; this spec only confirms validation may make the artifact eligible for review.

## Acceptance Tests

### ART-001: Valid MVP bundle passes

Validate the spec `001` dummy bundle with exact required files and manifest.

Expected:

- All required checks pass.
- Report contains exactly one check record per spec `010` validation check, in order.
- Validation report status is `passed`.
- Artifact can move to `machine_validated`.
- Public provenance contains only allowlisted fields.

### ART-002: Missing or extra files fail

Validate bundle missing `answer_key.md`, containing an extra file, or containing a subdirectory.

Expected:

- Validation report status is `failed`.
- Safe failure code identifies required/extra file category.
- Artifact does not become `machine_validated`.

### ART-003: Path traversal and unsafe entries fail

Validate bundle with absolute path, `..`, symlink, hard link, hidden file, duplicate normalized name, Unicode bidi path, or archive entry.

Expected:

- Bundle is rejected safely.
- Raw unsafe path is not persisted.
- Artifact does not become `machine_validated`.

### ART-004: Manifest self-claims cannot raise state

Set `status_claim` to `machine_validated`, `peer_reviewed`, or `classroom_ready`.

Expected:

- Manifest is rejected or accepted only as non-authoritative metadata according to schema rule.
- Artifact state remains central-state-derived.

### ART-005: Lineage mismatch fails

Set manifest `request_id`, `artifact_id`, or `work_packet_ids` to another request/proposal/work packet.

Expected:

- Validation fails with `artifact_lineage_mismatch`.
- No cross-linking occurs.

### ART-006: Markdown unsafe content fails

Include raw HTML, script, external links, remote images, data URLs, local paths, fake secrets, or named student records in markdown files.

Expected:

- Validation fails or redacts according to deterministic policy.
- Unsafe raw values are not persisted in report/log/error/public metadata.

### ART-007: Checker static safety rejects dangerous code

Include imports or calls for network, filesystem, subprocess, dynamic import, environment, threading, or eval/exec.

Expected:

- `python_checker_static_safety` fails.
- Checker is not executed.
- Artifact does not become `machine_validated`.

### ART-008: Checker execution sandbox applies limits

If sandboxed execution mode is enabled, run checker that loops, allocates memory, writes temp files, or prints fake secrets.

Expected:

- Resource limits stop unsafe behavior.
- Runtime sandbox denies network and filesystem access outside allowed runtime/bundle/temp paths.
- Raw unsafe stdout/stderr is not persisted.
- Validation report contains safe failure summaries only.

### ART-008A: Checker execution contract is deterministic

Run valid and invalid `checker.py` files through the execution harness.

Expected:

- Valid checker run uses validator-owned `python3 -I /validator-runtime/validator_checker_harness.py --checker /bundle/checker.py`, empty stdin, constrained environment, and exits `0`.
- Harness stdout is exactly `OK\n`; stderr is empty.
- Harness calls the three exact required checker functions with validator-owned conservation-of-energy sample cases.
- Checker top-level self-reporting such as `print("OK")` without the required functions fails.
- Nonzero exit, unexpected stdout/stderr, timeout, sandbox denial, missing function, wrong result, side effect, or missing sample behavior fails `python_checker_runs`.

### ART-009: Static-only mode is explicit

Run validation with checker execution disabled.

Expected:

- `python_checker_runs` is `skipped_static_only`.
- Validation report clearly records static-only mode.
- Validation report overall status is `incomplete_static_only` or `failed`.
- Artifact does not become `machine_validated`.
- Implementation cannot claim full execution-based validation passed.

### ART-010: Runner-submitted validation cannot pass

Submit a runner-created `validation_report.json` through artifact bundle or runner output.

Expected:

- It is rejected or untrusted evidence.
- Only trusted validator endpoint can create `trusted_passed`.
- Artifact does not become `machine_validated` from runner report.

### ART-011: Public provenance excludes private provider details

Validate bundle whose manifest or content tries to include provider name, model name, local prompt path, local workspace path, exact quota, or raw transcript.

Expected:

- Unsafe fields are rejected/redacted.
- Public provenance excludes them.

### ART-012: Validation reports no-leak

Trigger failures containing fake secrets, local paths, URLs, prompt text, stack traces, and student PII.

Expected:

- Report, logs, API errors, events, and public metadata contain only safe codes/messages/locations.
- No raw unsafe value or hash of unsafe value is persisted.

## Security Checklist Coverage

Applicable categories from `002`:

- Central-Core Boundary Checks: validation is deterministic and no-inference.
- Data Classification Checks: manifest, reports, logs, errors, and public provenance follow spec `004`.
- SSRF/Outbound URL Checks: URL imports/fetches and remote Markdown assets are rejected.
- Artifact and Generated-Code Checks: path normalization, bundle limits, static scanner, and sandbox rules are specified.
- Human Review and Publication Checks: validation does not replace human review.
- Logging, Error, and Audit Checks: validation reports use safe messages and locations only.

No applicable non-deferrable checklist item is deferred for MVP artifact validation. If sandboxed `checker.py` execution is not implemented, static-only mode must be represented truthfully and cannot move the artifact to `machine_validated`.

## Review Checklist

Reviewers should fail this spec if:

- Artifact bytes can be uploaded directly through central API in the MVP.
- `artifact_intake_ref` can become a path, URL, or filename authority.
- Path traversal, symlinks, duplicate normalized names, or extra files can pass.
- Manifest `status_claim` can raise central or public state.
- Runner-submitted validation can become trusted validation.
- `checker.py` can validate itself, self-report success, or access network, filesystem, environment, subprocess, or central secrets.
- Static-only validation can be reported as checker execution passed.
- Validation report can include raw stdout/stderr, local paths, secrets, URLs, stack traces, prompt text, or student PII.
- Public provenance can leak provider/model/local config or private review details.
