# 012. MVP Guardrail and End-to-End Test Matrix

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

Non-blocking reference:

- `docs/specs/013-task-scoped-code-execution-sandbox-provenance.md` describes sandbox details used by code-execution rows. It is not required to implement or validate this matrix spec.

## Purpose

This spec defines the implementation test matrix for the Rust MVP. It does not add product behavior. It names the required local gates that prove the central backend is deterministic, runner-first, no-inference, no-leak, and able to execute the first vertical slice from request intake through `peer_reviewed`.

No implementation phase may treat a behavior as complete unless the corresponding matrix item has an automated test or a documented manual fixture gate.

## Test Command Contract

Once the Rust workspace exists, the MVP implementation gate must expose these exact command families:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check --disable-fetch
cargo run -p verify-no-inference-core
cargo run -p verify-schema-fixtures
cargo run -p verify-no-leak-fixtures
cargo run -p lessonforge-e2e -- --suite mvp
```

Implementation may add faster package-scoped commands, but the full gate above is authoritative.

Expected gate behavior:

- Every command exits `0` on the accepted MVP implementation.
- Any failure prints safe reason codes or file/test names, not secret values.
- The e2e command uses deterministic fixtures and dummy actors only.
- The e2e command must not require provider credentials, model downloads, network access to model providers, embeddings, browser profiles, or external URLs.
- `cargo deny check --disable-fetch` must run from committed deny configuration and a locally available advisory/index setup. The full gate must not require live network access.

## Fixture Profiles

Required fixture profiles:

| Profile | Purpose |
|---|---|
| `mvp_happy_path` | Valid conservation-of-energy request through `peer_reviewed`. |
| `mvp_denied_requests` | Invalid request payloads from spec `006`. |
| `mvp_denied_graphs` | Invalid proposed graphs from spec `007`. |
| `mvp_abuse_inputs` | Fake secrets, prompt injection, PII-like strings, unsafe URLs, and local paths. |
| `mvp_runner_contract` | Dummy planner, verifier, generator, and config behavior from spec `009`. |
| `mvp_artifact_bundles` | Valid and invalid artifact bundles from spec `010`. |
| `mvp_review_publication` | Human review, findings, conflicts, and publication labels from spec `011`. |

Fixtures must be deterministic, checked into the repository, and free of real credentials, real student data, local absolute paths, provider account details, and raw model outputs.

## Matrix Status Vocabulary

Each matrix row must be represented in the implementation test manifest with one of:

- `automated`: test runs under the full command gate.
- `manual_fixture`: deterministic fixture is validated by a documented command but cannot yet be fully automated.
- `deferred_by_spec`: behavior is explicitly unavailable in the MVP and has a denial test.

Rows marked `deferred_by_spec` still need a negative test proving the unavailable behavior cannot be invoked.

## Required Row Statuses

The first Rust MVP gate must assign these statuses:

| Row ID | Required status |
|---|---|
| `E2E-001` | `automated` |
| `E2E-002` | `automated` |
| `REQ-GATE-001` | `automated` |
| `REQ-GATE-002` | `automated` |
| `REQ-GATE-003` | `automated` |
| `REQ-GATE-004` | `automated` |
| `REQ-GATE-005` | `automated` |
| `GRAPH-GATE-001` | `automated` |
| `GRAPH-GATE-002` | `automated` |
| `GRAPH-GATE-003` | `automated` |
| `GRAPH-GATE-004` | `automated` |
| `GRAPH-GATE-005` | `automated` |
| `LEASE-001` | `automated` |
| `LEASE-002` | `automated` |
| `LEASE-003` | `automated` |
| `LEASE-004` | `automated` |
| `STATE-GATE-001` | `automated` |
| `RUNNER-AUTH-001` | `automated` |
| `DB-GATE-001` | `automated` |
| `NINF-001` | `automated` |
| `NINF-002` | `automated` |
| `NINF-003` | `automated` |
| `NINF-004` | `automated` |
| `LEAK-001` | `automated` |
| `LEAK-002` | `automated` |
| `LEAK-003` | `automated` |
| `LEAK-004` | `deferred_by_spec` |
| `RUN-GATE-001` | `automated` |
| `RUN-GATE-002` | `automated` |
| `RUN-GATE-003` | `automated` |
| `RUN-GATE-004` | `automated` |
| `CODE-GATE-001` | `automated` |
| `CODE-GATE-002` | `automated` |
| `CODE-GATE-003` | `automated` |
| `CODE-GATE-004` | `automated` |
| `ART-GATE-001` | `automated` |
| `ART-GATE-002` | `automated` |
| `ART-GATE-003` | `automated` |
| `REVIEW-GATE-001` | `automated` |
| `REVIEW-GATE-002` | `automated` |
| `REVIEW-GATE-003` | `automated` |
| `REVIEW-GATE-004` | `automated` |
| `REVIEW-GATE-005` | `automated` |
| `CR-GATE-001` | `deferred_by_spec` |
| `CR-GATE-002` | `deferred_by_spec` |
| `CR-GATE-003` | `deferred_by_spec` |
| `CR-GATE-004` | `deferred_by_spec` |
| `CR-GATE-005` | `deferred_by_spec` |
| `CR-GATE-006` | `deferred_by_spec` |
| `CR-GATE-007` | `deferred_by_spec` |
| `API-GATE-001` | `automated` |
| `API-GATE-002` | `automated` |

Rows may not be downgraded to `manual_fixture` without a later passed spec revision.
CR-GATE rows remain `deferred_by_spec` in this matrix because spec `014` is not active for this MVP gate. Spec `014` exists as a follow-up code critique and repair contract, but this release does not expose its central API ingestion surfaces. The rows flip to `automated` only in a later implementation where spec `014` is active. Until then, the MVP gate must run negative-unavailable checks proving no central API critique or repair ingestion surface is exposed.

## End-to-End Happy Path

### E2E-001: Full MVP vertical slice reaches peer reviewed

Run the `mvp_happy_path` fixture.

Expected:

- request intake accepts the spec `001` request;
- central core creates exactly one request moderation task;
- dummy request moderator submits accepted moderation evidence;
- central core creates exactly one mechanical planning task;
- dummy planner submits the valid `ProposedTaskGraph`;
- deterministic graph policy accepts it;
- independent dummy verifier submits accepted plan verification;
- deterministic promotion creates generation, validation, and human-review work packets once;
- dummy generator submits an artifact bundle reference with file digests, bundle digest, and sandboxed self-test report;
- validator accepts the bundle through spec `010` trusted validation path;
- artifact becomes `machine_validated`;
- qualified non-conflicted human reviewer approves;
- artifact becomes `peer_reviewed`;
- public metadata exposes only the spec `004`, `010`, and `011` allowlist.

### E2E-002: Re-running the happy path is idempotent where specified

Replay request creation, claim loss, proposal submission, promotion, artifact submission, validation report submission, and review submission with the same idempotency keys.

Expected:

- duplicate records are not created;
- claim tokens are not re-displayed;
- stale or mismatched replay bodies are rejected;
- final state remains `peer_reviewed`.

## Denied Request Flows

### REQ-GATE-001: Unsupported MVP values rejected

Submit unsupported subject, age range, language, artifact type, license, visibility, duration, and over-limit text.

Expected:

- request is rejected before planning task creation;
- rejected raw over-limit content is not persisted;
- error response uses safe codes.

### REQ-GATE-002: Prompt injection remains inert

Submit request text that tries to override policy, request provider credentials, force hidden prompts, skip review, or publish directly.

Expected:

- request text may be stored only as untrusted input if otherwise valid;
- no central policy, prompt, runner config, state transition, or publication behavior changes;
- no central model or classifier is invoked.

### REQ-GATE-003: PII and secret-like inputs rejected or redacted

Submit fake API keys, token-like strings, local paths, email-like strings, phone-like strings, and named student rosters.

Expected:

- forbidden request shapes are rejected;
- persisted logs, audit events, and errors do not contain the unsafe values;
- accepted safe fields remain unchanged.

### REQ-GATE-004: Request moderation blocks planning

Submit valid-shaped requests that include obvious sexual, sexual-minors, graphic-violence, self-harm, hate/harassment, illicit, weapons, privacy-invasion, or age-inappropriate content. Also submit moderation reports that are stale, cross-lineage, wrong actor, contradictory, contain raw provider responses, or maliciously claim `allow_mvp_planning` with `category_flags=["none"]` for request text matching deterministic inappropriate-content heuristics.

Expected:

- accepted intake creates a request moderation task, not a planning task;
- denied or quarantined moderation creates no planning task;
- forged, contradictory, heuristic-overridden, or unsafe moderation evidence is rejected;
- central core performs no model moderation provider call.

### REQ-GATE-005: Moderation prompt injection is inert

Submit request text telling the moderation runner or model provider to ignore safety policy, mark content safe, reveal prompts, or include raw provider output.

Expected:

- text is treated as content only;
- moderation task schema remains fixed;
- raw provider output and prompt text cannot be persisted;
- planning is unlocked only by accepted safe moderation evidence.

## Proposed Graph and Promotion Gates

### GRAPH-GATE-001: Malformed and unsafe graphs fail policy

Submit graphs with malformed JSON, wrong schema version, unknown task types, cycles, duplicate local IDs, missing MVP tasks, extra artifacts, arbitrary prompts, provider-account handling, student grading, network requirements, or hidden credential tasks.

Expected:

- proposal is rejected with structured policy errors;
- no verification task, promotion decision, work packet, or artifact is created from rejected graphs.

### GRAPH-GATE-002: Plan verification is required for MVP promotion

Submit a valid planner graph but skip or forge agent-driven verification.

Expected:

- proposal cannot become `verified_for_mvp_promotion`;
- promotion endpoint rejects with a safe source-state or missing-evidence code;
- no work packets are created.

### GRAPH-GATE-003: Medium and high risk proposals do not promote

Submit proposals that deterministic policy raises to medium or high risk.

Expected:

- medium-risk proposals become `policy_rejected` in the MVP with a safe reason;
- high-risk proposals become `policy_rejected` in the MVP with a safe reason;
- no verification task, promotion decision, work packet, or artifact is created;
- rejected proposals cannot promote;
- central core does not perform semantic merge or model arbitration.

### GRAPH-GATE-004: Promotion is transactional

Force a duplicate or simulated partial promotion retry.

Expected:

- work-packet materialization is all-or-nothing;
- retry produces the same promotion result without duplicate work packets;
- provenance links back to request, proposal, verification, and promotion decision.

### GRAPH-GATE-005: Planning fanout closes deterministically

Run multiple concurrent planner leases for one planning task. Submit one malformed proposal, then two valid proposals in deterministic arrival order.

Expected:

- rejected proposals do not close fanout;
- the first accepted `schema_policy_validated` proposal closes planning fanout immediately;
- only that first accepted proposal creates a plan verification task and becomes eligible for later verification and promotion;
- remaining active planner leases are released, revoked, or rejected as no-longer-needed according to specs `006` and `008`;
- later proposal submissions cannot create verification tasks, promotion decisions, work packets, ranking evidence, or fallback candidates even if the first selected proposal later fails verification;
- central core performs no semantic comparison, ranking, merge, or model arbitration between valid proposals.

## Lease, Retry, and Replay Gates

### LEASE-001: Claim limits and actor uniqueness enforced

Try duplicate active claims by the same actor and excess parallel claims.

Expected:

- same actor cannot hold two active slots for the same task;
- fanout limits are enforced;
- safe errors do not reveal claim tokens.

### LEASE-002: Heartbeat cannot mutate payload or authority

Send heartbeat requests with changed body fields, actor IDs, output claims, or state claims.

Expected:

- heartbeat only extends server-side liveness as specified;
- mutation attempts are rejected or ignored according to endpoint contract;
- no task output is accepted through heartbeat.

### LEASE-003: Stale submits rejected

Submit moderation, planner, verifier, generator, validation, and review outputs after lease expiry or release.

Expected:

- stale submissions are rejected;
- expired leases do not create accepted evidence;
- later valid claims can proceed according to source-state guards.

### LEASE-004: Replay and replacement behavior is closed

Exercise claim replay, heartbeat replay, release replay, lease expiry, replacement claim, cancelled claim, and mismatched idempotency body across request moderation, planning, verification, generation, validation, and review leases.

Expected:

- claim tokens are returned only once and are not re-displayed on idempotent replay;
- heartbeat replay is idempotent only when body and lease authority match;
- heartbeat unknown fields and authority fields are rejected;
- release replay is idempotent and does not resurrect a lease;
- expiry allows replacement only through the configured source-state rule;
- cancelled, replaced, released, or stale claim tokens cannot submit evidence;
- mismatched idempotency body is rejected with no side effects.

## Source-State, Runner Authority, and Database Gates

### STATE-GATE-001: Mutating endpoints reject invalid source states

For every MVP mutating endpoint, enumerate allowed source states and test representative rejected source states:

| Surface | Must reject at least |
|---|---|
| request create | duplicate idempotency body mismatch, unsupported fields |
| request moderation claim | missing/completed/cancelled moderation task, wrong capability, wrong scope |
| request moderation report submit | no active moderation lease, wrong request, stale lease, raw provider response |
| planning claim | closed/cancelled task, exhausted fanout, same actor duplicate |
| proposal submit | no lease, stale lease, after fanout closure, wrong planning task state |
| verification claim | missing verification task, completed/cancelled task |
| verification submit | no active verification lease, wrong proposal state, after proposal rejection |
| promotion | unverified proposal, rejected proposal, already promoted proposal |
| generation work claim | validation/review/system work packet, closed work packet |
| generation submit | no active generation lease, wrong work packet, stale lease |
| validation claim | non-validation work packet, closed validation work packet |
| validation report submit | no active validation lease, missing artifact, mismatched artifact/work packet |
| review task claim | artifact not `machine_validated` or `review_requested`, conflicting reviewer |
| review submit | no active review lease, artifact state changed, conflicting reviewer |
| public/admin mutation attempts | unauthenticated or wrong actor type |

Expected:

- invalid source states return safe `400`, `403`, `409`, `410`, or `422` errors according to endpoint contract;
- no partial side effects occur;
- no evidence, artifact, report, review, promotion, or state transition is created.

### RUNNER-AUTH-001: Runner-submitted lineage is not authority

Use otherwise valid active credentials or claim tokens to submit cross-lineage or forged runner outputs across request moderation, planner, verifier, generator, and validation surfaces.

Cases:

- body-supplied `runner_id`, `actor_id`, `reviewer_id`, or source actor fields do not match authenticated context;
- active lease for request/proposal/work packet/artifact A submits output for B;
- moderator submits moderation report for a request not linked to its claimed moderation task;
- verifier submits verification for a proposal not linked to its claimed task;
- generator submits artifact reference for a different work packet or request;
- normal runner submits validation report through the trusted validator path;
- wrong actor type, capability, trust level, or scope submits otherwise schema-valid output.

Expected:

- server-derived authenticated actor, lease, scope, and lineage win over body fields;
- submission is rejected with safe `403`, `409`, or `422`;
- no accepted evidence, artifact, validation report, review record, promotion decision, or state transition is created;
- forged body fields are not persisted as authority.

### DB-GATE-001: Durable constraints enforce central authority

Inspect migrations and run constraint tests for server-derived IDs, immutable lineage, uniqueness, leases, idempotency, and rejected authority fields.

Expected:

- accepted records use server-derived immutable IDs where specified;
- lineage foreign keys connect request, request moderation task/report, planning task, proposal, verification, promotion, work packet, artifact, validation report, review task, review, and finding records;
- idempotency uniqueness prevents duplicate mutation effects;
- active lease uniqueness and slot/fanout limits are enforced transactionally;
- persisted findings/reviews cannot store submitted authority fields as trusted state;
- database or transaction boundary rejects orphaned, cross-lineage, duplicate-active, or stale records.

## No-Inference Central Guardrails

### NINF-001: Central crates have no model-provider dependencies

Run `verify-no-inference-core`.

Expected:

- central API/domain crates contain no provider SDK or moderation-provider SDK imports;
- dependency graph contains no model-provider, embeddings, vector database, browser automation, or local-model runtime crates in central packages;
- allowlist exceptions must be explicit and test-only.

### NINF-002: Central runtime does not read provider credentials

Run central tests with fake provider credential environment variables set.

Expected:

- central startup and tests do not read `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, moderation-provider credentials, local model paths, provider base URLs, browser profile paths, or prompt-template paths;
- no value appears in logs, errors, database rows, or public responses.

### NINF-003: Central network egress excludes model/provider calls

Run central API tests with outbound network mocks or denial instrumentation.

Expected:

- central core performs no outbound model-provider, moderation-provider, embedding, arbitrary URL, SSRF, or semantic-search calls;
- allowed loopback/local API serving does not mask forbidden egress.

### NINF-004: Central database has no inference-owned tables

Inspect migrations and schema.

Expected:

- no prompt, embedding, vector, provider credential, provider account, model trace, raw moderation/provider response, or semantic-cache tables exist in central schema.

## No-Leak and Public Response Gates

### LEAK-001: API errors and logs redact unsafe values

Inject fake secrets, local paths, provider config fields, auth paths, cookies, exact quota text, and private reviewer notes through each accepted input surface.

Expected:

- unsafe values are rejected or redacted before persistence;
- logs and errors contain safe codes only;
- audit events keep actor/action/state metadata without raw unsafe payloads.

### LEAK-002: Public endpoints do not leak draft/private existence

Read public request/artifact metadata for draft, private, validation-failed, review-requested, quarantined, and never-public deprecated fixtures.

Expected:

- public endpoint returns `404` where specs require non-disclosure;
- peer-reviewed and allowed machine-validated public metadata is allowlisted;
- no provider, model, local path, private note, claim token, audit, or reviewer private profile data is exposed.

### LEAK-003: Polling and list endpoints apply availability and redaction rules

Poll and list endpoints with authorized and unauthorized actors.

Expected:

- rate, pagination, and authorization controls apply;
- responses contain only safe IDs, states, and reason codes;
- no secret-like field appears in response payloads.

### LEAK-004: SSE stream unavailable in first MVP gate

Attempt to subscribe to `/v1/events/stream`.

Expected:

- disabled SSE returns an unavailable safe response and emits no data;
- no fallback WebSocket, long-poll mutation, or arbitrary event command channel is exposed;
- if a later implementation wants SSE enabled, this spec must be revised so SSE availability, authorization, rate, and redaction tests are `automated` before release.

## Runner Contract Gates

### RUN-GATE-001: Dummy runner modes complete without inference

Run request moderator, planner, verifier, and generator dummy modes.

Expected:

- outputs match spec fixtures;
- runner submits only structured schema-valid data;

### RUN-GATE-002: Unsafe runner config rejected

Validate configs containing unsupported provider-backed modes, unsafe central API origins, redirects, ambient proxy use, provider secrets, local auth paths, browser profiles, local model paths, and absolute workspace leaks.

Expected:

- config validation fails before central API interaction;
- failure output is redacted;
- loopback HTTP is accepted only under explicit local/test profile.
- non-loopback HTTPS requires explicit TLS pin/trust policy.

### RUN-GATE-003: Claim tokens remain ephemeral

Exercise `run-once`, lost claim response, verbose logging, retry, and stale lease behavior.

Expected:

- claim tokens are held only in memory for the attempt;
- tokens are never printed, persisted, embedded in submitted objects, or recovered by replay;
- lost claim response cannot be used to recover token.

### RUN-GATE-004: Task content cannot trigger tool execution

Run request moderation, planning, verification, and generation tasks whose request text, work-packet fields, artifact text, or simulated provider output asks the runner to execute shell commands, read files, exfiltrate env vars, install packages, open browsers, call Docker, or execute tool/function-call JSON.

Expected:

- runner treats content as inert data;
- no local shell, browser, package manager, Docker, filesystem, or model tool call executes;
- unsafe output is rejected locally or submitted only as schema-valid safe data;
- logs contain safe reason codes only.

## Code Execution Policy and Sandbox Gates

### CODE-GATE-001: Task policy, not request text, grants self-test

Submit request text and planner proposal fields asking for shell execution, package installation, browser use, or unsandboxed code execution.

Expected:

- request text and planner fields cannot raise execution policy;
- generation work packet receives only the central spec `013` self-test policy after deterministic promotion;
- all other MVP runner tasks remain `no_execution` or `code_generation_only`.

### CODE-GATE-002: Runner self-test sandbox is enforced

Run dummy generator self-test for `checker.py`.

Expected:

- command ID maps to the exact spec `013` harness command;
- sandbox or constrained MVP subprocess profile has no function-time network/file descriptor access;
- sandbox or constrained MVP subprocess environment has no central token, claim token, provider credential, SSH key, cloud credential, home directory, or browser profile;
- filesystem access is limited to declared workspace mounts;
- resource limits are enforced.

### CODE-GATE-003: Hashes and attestations bind output

Tamper with generated files after self-test, replay self-test report across another work packet, or submit an Ed25519 signature from the wrong registered runner key.

Expected:

- digest mismatch, lineage mismatch, or registered signature/key mismatch rejects the self-test evidence;
- stale self-test provenance cannot apply to another artifact;
- signatures/hashes do not replace lease authorization.

### CODE-GATE-004: Self-test cannot machine validate

Submit `self_test_status=passed` and attempt to mark the artifact `machine_validated`.

Expected:

- artifact remains `draft_generated` until trusted validator passes;
- public label does not change;
- raw stdout/stderr, local paths, prompts, provider responses, and secrets are not persisted.

## Artifact Validator Gates

### ART-GATE-001: Valid MVP bundle passes trusted validation

Validate the required worksheet, answer key, `checker.py`, teacher notes, and manifest bundle.

Expected:

- manifest schema, required files, license, AI assistance disclosure, no-PII heuristic, obvious inappropriate content heuristic, path normalization, static no-network checks, and validator-owned checker harness pass;
- validation report has one check record per spec `010`;
- artifact becomes `machine_validated` only through the trusted validation path.

### ART-GATE-002: Invalid bundles fail safely

Validate bundles with missing files, path traversal, oversized files, unsafe MIME/content, network imports, bad license, manifest self-claim, fake secrets, local paths, checker contract mismatch, and self-printing `OK`.

Expected:

- bundle fails with structured report status;
- runner self-claims do not raise artifact state;
- unsafe values do not appear in public metadata or logs.

### ART-GATE-003: Checker execution sandbox enforced

Run malicious checker fixtures attempting network access, filesystem escape, subprocess execution, environment reads, infinite loop, excessive memory, and stdout spoofing.

Expected:

- validator-owned harness command uses isolated Python with the exact spec `010` checker contract;
- sandbox denies escape attempts and enforces CPU, memory, process, filesystem, and time limits, or the constrained MVP subprocess profile enforces the spec `003` closed-checker exception with static gates, CPU/wall-clock limits, empty environment, no stdout/stderr, and function-time file-descriptor denial;
- failure report is safe and deterministic.

## Review and Publication Gates

### REVIEW-GATE-001: Human review cannot be forged

Attempt review submission without lease, through work-packet endpoints, with spoofed reviewer ID, wrong actor type, missing trust, wrong subject, wrong age range, or wrong scope.

Expected:

- review is rejected;
- no accepted review record or promotion occurs.

### REVIEW-GATE-002: Conflict and Sybil checks fail closed

Attempt reviewer claim or submission as planner, verifier, generator, same operator account, same conflict group, missing independence metadata, unverified independence metadata, suspended actor, or revoked actor.

Expected:

- claim or submission is rejected with safe conflict code;
- artifact remains non-`peer_reviewed`.

### REVIEW-GATE-003: Findings are server-derived and blocking policy is central

Submit findings with forbidden persisted fields, submitted `blocking=false` for always-blocking types, invalid severity/type combinations, critical/major findings, and nonblocking minor/note findings.

Expected:

- forbidden fields are rejected;
- server derives finding ID, parent, creator, timestamps, state, and `blocking`;
- open blocking findings prevent `peer_reviewed`;
- nonblocking findings do not require waiver.

### REVIEW-GATE-004: Critique and plan verification cannot replace human review

Submit accepted plan verification and validate critique fixtures that claim approval, no blocking findings, recommended next state, or human-review equivalence.

Expected:

- plan verification remains plan-only evidence;
- general critique has no MVP ingestion endpoint and cannot create central findings;
- spec `014` code critique, when enabled, remains advisory repair evidence and cannot create human review;
- no artifact reaches `peer_reviewed` without accepted human review.

### REVIEW-GATE-005: Publication labels derive from central state

Read public metadata for draft, validation failed, machine validated public/private, review requested public/private, peer reviewed public/private, quarantined, and deprecated fixtures.

Expected:

- labels match spec `011` precedence table;
- manifest `status_claim`, validation self-claim, review recommendation, and critique output do not set labels;
- `classroom_ready` never appears in MVP.

## Code Critique and Repair Gates

### CR-GATE-001: Critique is advisory only

CR-GATE rows are `deferred_by_spec` in this matrix and activate only after a later release makes spec `014` active. Spec `012` names the matrix slots without making the whole MVP guardrail matrix depend on spec `014`; while spec `014` is inactive for this MVP gate, the E2E suite runs only negative-unavailable checks for the code critique and repair ingestion surface.

Submit signed code critique with no findings, blocking findings, approval-like text, and recommended next state.

Expected:

- critique persists only as advisory evidence;
- no trusted validation state is created;
- no `Review` record is created;
- artifact public label does not change.

### CR-GATE-002: Repair creates a new draft artifact

Submit accepted code repair output for a repairable `checker.py` issue.

Expected:

- source artifact is not mutated;
- repaired candidate is a new `draft_generated` artifact with repair lineage;
- trusted validation work is created for the repaired artifact;
- repaired artifact cannot skip validation or human review.

### CR-GATE-003: Automated repair requires teacher and runner opt-in

Run all combinations of request `auto_repair_preference` and runner `automated_repair_loop_opt_in`.

Expected:

- no automated repair work is created unless the teacher requested bounded repair and an eligible volunteer runner opted in;
- request text alone cannot enable repair;
- runner capability alone cannot enable repair;
- central policy may still decline repair.

### CR-GATE-004: Repair interruption is safe and bounded

Simulate runner quota exhaustion, operator budget exhaustion, provider unavailability, suspected looping bug, and suspected malicious task.

Expected:

- runner submits signed interruption report with safe reason code only;
- exact quota, account IDs, provider response, prompt, model response, local path, secret, and partial artifact bytes are not persisted;
- no artifact is created;
- continuation on another opted-in runner requires closed `runner_operator`, curator, or admin outcome;
- continuation and repair attempts stay under spec `014` limits.

### CR-GATE-005: Repair report replay and broadened changes fail

Replay critique/repair/interruption reports across another artifact, lease, runner, attempt index, continuation index, or bundle digest. Alter signed routing fields under an old signature. Submit repair output that changes non-code files.

Expected:

- lineage mismatch is rejected;
- wrong runner signature is rejected;
- tampered `outcome`, `repair_status`, or `interruption_reason` is rejected;
- non-code file changes are rejected for MVP;
- no repaired draft artifact is created from invalid repair output.

### CR-GATE-006: Unsafe non-code content blocks automated repair

Submit artifact bundles with both repairable `checker.py` issues and unsafe worksheet, answer key, teacher-notes, manifest, license, path/MIME, PII, secret-like, or inappropriate-content failures.

Expected:

- automated repair work is not created;
- non-code files are not copied into a repaired artifact;
- loop stops with deterministic decline or quarantine reason;
- trusted validation and curator/quarantine handling remain authoritative.

### CR-GATE-007: Critique and repair text cannot smuggle code or prompts

Submit critique, repair, and interruption reports containing short raw code snippets, tool-call JSON, Markdown/HTML links, prompt-like instructions, provider output, local paths, URLs, raw stderr, and fake secrets in location/message/summary fields.

Expected:

- unsafe fields are rejected before persistence;
- logs/events/errors contain only safe field names and reason codes;
- no continuation, repair artifact, review, validation, or public label is created from rejected reports.

## API Transport and Authorization Gates

### API-GATE-001: HTTPS and loopback rules enforced

Run API/runner config tests for HTTPS, loopback HTTP under local profile, non-loopback HTTP, TLS validation failure, redirect, proxy, and spoofed actor headers.

Expected:

- HTTPS is required outside explicit loopback/local test profile;
- redirects and ambient proxy forwarding are rejected by default;
- spoofed actor headers are stripped or rejected;
- test actor injection cannot start outside gated test profile.

### API-GATE-002: WebSocket command path unavailable

Attempt mutating workflow commands through WebSocket or arbitrary event-stream messages.

Expected:

- no WebSocket command endpoint exists in the MVP;
- SSE is read-only if enabled;
- all mutations use HTTP JSON endpoints with idempotency and source-state guards.

## Implementation Manifest

The implementation must maintain a machine-readable test manifest, such as:

```json
{
  "spec": "012",
  "suite": "mvp",
  "rows": [
    {
      "id": "E2E-001",
      "status": "automated",
      "command": "cargo run -p lessonforge-e2e -- --suite mvp --case E2E-001"
    }
  ]
}
```

Manifest rules:

- Every matrix row in this spec appears exactly once.
- `deferred_by_spec` rows name the denying spec and negative test.
- Missing, duplicate, or unknown row IDs fail `verify-schema-fixtures` or `lessonforge-e2e`.
- Manifest output must be redacted under spec `004`.

## Review Checklist

Reviewers should fail this spec if:

- Any required prior-spec guardrail lacks a matrix row.
- The test gate requires real provider credentials, model calls, embeddings, or external provider network access.
- Central no-inference checks are name-only and do not cover dependencies, env reads, network egress, and database schema.
- No-leak tests cover only public metadata and omit errors, logs, events, and audit records.
- Runner tests allow provider config or claim tokens to cross into central API.
- Artifact tests allow `checker.py` self-validation or central API execution of generated code.
- Review tests allow critique, plan verification, or reviewer-submitted authority fields to promote artifacts.
- Deferred MVP behavior lacks a negative test proving it is unavailable.
