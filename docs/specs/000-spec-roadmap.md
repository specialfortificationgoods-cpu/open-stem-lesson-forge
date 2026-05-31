# Open STEM Lesson Forge Spec Roadmap

Status: Passed adversarial review  
Source design: `/Users/macmini/Downloads/open_stem_lesson_forge_design_v0_2_deterministic_core.md`  
Repository instructions: `AGENTS.md`

## Purpose

This roadmap defines the implementation-grade specs that must be written before coding Open STEM Lesson Forge. The design document sets direction; these specs turn it into testable contracts for stack boundaries, data classification, APIs, state transitions, verification gates, and acceptance tests.

The roadmap follows the project architecture contract:

- The central backend is deterministic.
- All LLM inference, provider selection, model calls, provider-specific prompt construction, and semantic request decomposition live in runners or explicitly separate non-core services.
- Request-to-task generation is runner-first.
- Runner outputs are untrusted until schema validation, deterministic policy checks, agent-driven verification where configured, and human review gates where required.
- No endpoint, table, log schema, public response, or artifact publication path should be implemented ahead of its passed spec.

## Review Method

Each spec must pass targeted adversarial review before implementation work starts from it.

Required review roles:

- Deterministic-core reviewer: checks that no inference, model dependency, prompt execution path, provider credential, embedding path, semantic merge, or model-provider network call enters the central core.
- Workflow/trust reviewer: checks request-to-task generation, agent verification, human review, state transitions, leases, identity/trust boundaries, abuse cases, and promotion safety.
- MVP/stack reviewer: checks scope, sequencing, stack fit, testability, and whether the spec is small enough to implement.

Review status vocabulary:

- `pass`: no blocking gaps remain.
- `needs_revision`: specific issues must be fixed and re-reviewed.
- `blocked`: reviewer cannot determine correctness because an upstream decision or missing artifact prevents review.

No spec is implementation-ready until all required review roles return `pass`.

## Hard Sequencing Gates

Before any core/API backend coding:

- Specs `001` through `008` must pass review.
- The no-inference and no-leak guardrail slice of spec `012` must pass review.

Before artifact, validation-report, review, publication, logging, public response, or persistence implementation:

- Specs `010` through `011` must pass review.
- The relevant `012` guardrail tests for those surfaces must pass review.

Before runner or validator implementation:

- Spec `009` must pass before runner work.
- Spec `010` must pass before validator or artifact-bundle work.

## Spec Set

### 001. MVP Slice and Acceptance Test Spec

Decision to make:

- Define the first vertical MVP slice before stack choices harden.

Must specify:

- One representative teacher request.
- One request moderation gate before planning.
- One deterministic planning task created from that request.
- One dummy planner-produced `ProposedTaskGraph`.
- One agent-driven plan verification result.
- One deterministic promotion path into work packets.
- One dummy generated artifact bundle.
- One deterministic validation report.
- One minimal human review flow from `machine_validated` to `peer_reviewed`.
- Explicit deferrals: simulations, real provider adapters, embeddings, reputation, advanced curation, translation/localization, full classroom-ready policy, and project-funded inference.
- Local acceptance-test scenarios and expected outcomes.

Pass criteria:

- A developer can implement the first slice without reading the whole design document.
- The slice proves request intake, planning-task creation, dummy planner proposal, agent verification, deterministic validation/promotion, dummy generation, deterministic artifact validation, and minimal human review.
- Acceptance tests include both allowed and denied flows.

### 002. Security, Privacy, and Abuse-Resistance Checklist

Decision to make:

- Define cross-spec security and abuse checks used during every spec review.

Must specify:

- Credential handling boundaries.
- Request prompt-injection handling.
- Runner submission trust model.
- Attachment and license handling.
- Student PII restrictions.
- Generated code execution limits.
- Artifact hosting restrictions.
- Audit logs and abuse reports.
- Provider-terms posture and local subscription framing.
- Required adversarial tests per spec.

Pass criteria:

- The checklist gates specs `003` through `011`.
- The checklist includes concrete tests for credential exfiltration, task poisoning, malicious artifacts, fake runner submissions, unsafe educational requests, prompt smuggling, public metadata leaks, review laundering, and accidental central inference.

### 003. Architecture, Stack, and No-Inference Boundary Spec

Decision to make:

- Choose the MVP stack and service/package boundaries.
- Decide whether the MVP uses Python/FastAPI, Rust, or a deliberate hybrid.
- Define why each service/package exists.

Must specify:

- Central backend runtime and persistence approach.
- Runner CLI/runtime.
- Validator package/runtime.
- Schema package format and source of truth.
- Test runner and local development commands.
- Explicit no-inference guardrails for the central backend.
- Forbidden central dependencies, environment variables, network destinations, database tables, and code paths.
- Migration path if a later component moves to Rust or another runtime.

Pass criteria:

- The chosen stack can implement the MVP slice, deterministic core, local runner, schema validation, and validator without forcing model-provider dependencies into `apps/api`.
- The choice is justified by architecture and maintainability, not inherited project assumptions.
- CI/local tests can prove `apps/api` has no model-provider SDK imports, no provider credential environment reads, no inference adapters, no embeddings/vector tables, and no outbound model-provider calls.

### 004. Data Classification, Redaction, Logging, and Error No-Leak Spec

Decision to make:

- Define what data may be accepted, persisted, logged, returned, and published.

Must specify:

- Public, internal, private, forbidden, and secret field classifications.
- Field allowlists for API payloads, runner capability summaries, proposed task graphs, artifact manifests, validation reports, critique reports, review notes, audit events, logs, errors, and publication metadata.
- Redaction rules for secrets, auth paths, local paths, provider base URLs, provider account details, local prompt templates, exact quota details, and accidental credentials.
- Error response redaction.
- Log schema and log redaction.
- Public artifact metadata allowlist.
- Rejection versus redaction behavior for fake secrets and unsafe fields.

Pass criteria:

- Every persisted object has an explicit field allowlist and redaction policy.
- Fake secrets injected through runner profile, artifact manifest, validation report, critique report, review note, publication metadata, logs, and errors are rejected or redacted before storage and public output.
- Optional provenance disclosure cannot become a provider fingerprinting or credential-leak channel.

### 005. Core Data Model and State Machine Spec

Decision to make:

- Define durable entities and allowed state transitions for the central backend.

Must specify:

- `Request`
- `PlanningTask`
- `ProposedTaskGraph`
- `PlanVerification`
- `WorkPacket`
- `Artifact`
- `ValidationReport`
- `CritiqueReport`
- `Review`
- `PromotionDecision`
- `Lease`
- `RunnerCapabilitySummary`
- `StateTransitionEvent`
- Trust level and capability fields as separate concepts.
- State machines for request, planning task, plan proposal, work packet, artifact, review, finding, and publication label.
- Immutable IDs, version fields, timestamps, audit log fields, and provenance fields.
- How promoted work packets snapshot the source proposal and preserve provenance back to validation and promotion decisions.

Pass criteria:

- Every state transition is deterministic or explicitly performed by a runner/human actor.
- Invalid transitions have defined errors.
- Proposal, request, planning task, selected plan, and work packet statuses cannot be confused.
- Review findings have a lifecycle such as open, resolved, waived, duplicate, superseded, blocking, and nonblocking.
- Artifact status cannot be raised by runner-controlled manifest fields alone.

### 006. Request Intake to PlanningTask Workflow Spec

Decision to make:

- Define the request-to-planning-task workflow.

Must specify:

- Deterministic request intake checks: required fields, enums, length limits, obvious PII heuristics, attachment/license checks, public/private visibility checks, and MVP subject/language checks.
- Immutable request fields and which fields planner proposals may reinterpret only as proposed metadata.
- Mechanical creation of `PlanningTask` from request reference plus required output schema.
- Planner runner eligibility.
- Lease, heartbeat, expiry, stale submit, duplicate submit, retry, replacement claim, and abandoned work behavior.
- Duplicate independent planning requirements.
- No central semantic decomposition.

Pass criteria:

- Request intake performs only deterministic checks.
- Planning-task creation is mechanical, not inferred lesson decomposition.
- Teacher request text is stored and treated as untrusted data, never as executable instruction.
- Prompt-injection text in requests cannot change central policy or task execution.

### 007. ProposedTaskGraph Schema, Policy Validation, Promotion, and Reconciliation Spec

Decision to make:

- Define how runner-proposed task graphs become, or fail to become, work packets.

Must specify:

- Proposed graph schema, enum sets, dependency graph rules, and versioning.
- Forbidden fields such as `arbitrary_prompt`.
- Forbidden task types and content categories.
- Deterministic policy result format and structured errors.
- Central deterministic risk escalation rules.
- Low-, medium-, and high-risk promotion rules.
- Independence requirements for duplicate planner proposals.
- Deterministic compatibility checks.
- Idempotent and transactional work-packet materialization keyed by proposal and local task ID.
- Reconciliation task creation and human curator escalation.
- Rule that conflicting plans are never semantically merged in the central core.

Pass criteria:

- Validation rejects malformed JSON, cyclic dependencies, unknown task types, arbitrary prompts, credential handling, provider-account handling, student grading, student placement, profiling, hidden network requirements, missing validation plans, and missing human review gates.
- Low-risk promotion uses only specified deterministic rules.
- Medium-risk promotion requires independent proposals plus deterministic compatibility checks, or routes to reconciliation/human review.
- Repeated promotion cannot duplicate work packets.

### 008. Central API Contract Spec

Decision to make:

- Define deterministic API endpoints and their request/response/error contracts.

Must specify:

- Request endpoints.
- Planning task endpoints.
- Plan proposal validation and promotion endpoints.
- Work packet eligibility, claim, heartbeat, release, and submit endpoints.
- Artifact, validation, critique, review, and publication endpoints only to the extent required by the MVP slice.
- Auth and authorization assumptions for MVP.
- Idempotency keys.
- Pagination.
- Error response format and redaction.
- Race, replay, stale submit, and lease-expiry behavior.
- Endpoint field allowlists.

Pass criteria:

- No endpoint accepts provider credentials, full local provider config, raw executable prompts, provider-specific prompt templates, auth paths, or provider base URLs unless a future spec explicitly adds an opt-in safe disclosure field.
- Endpoints expose structured data only, not executable prompts.
- All mutating endpoints define allowed source states, side effects, idempotency, and structured errors.

### 009. Runner Contract, Capability Summary, and Local Configuration Spec

Decision to make:

- Define local runner responsibilities, CLI behavior, redaction, local prompts, and provider adapter boundary.

Must specify:

- Runner config file format.
- Redacted capability summary format.
- Local provider adapter interface.
- Planner, generator, validator, critique, repair, and packaging runner modes.
- Manual, assisted, auto-draft, and review-only behavior.
- Local prompt-template rules.
- Local workspace isolation.
- Credential redaction and forbidden transmissions.
- Dummy planner/generator behavior for MVP tests.

Pass criteria:

- Full provider config remains local.
- Capability summaries cannot leak API keys, auth paths, base URLs, local prompt templates, local filesystem paths outside safe workspace metadata, or exact quota details unless explicitly allowed by a later reviewed spec.
- The dummy runner can complete planning and generation without real model calls.

### 010. Artifact Manifest, Validation, and Provenance Spec

Decision to make:

- Define artifact bundle structure, deterministic validation behavior, and safe provenance disclosure.

Must specify:

- Bundle layout.
- `manifest.json`.
- Required files for MVP artifact types.
- Validation report schema.
- Required-file checks.
- License/provenance checks.
- AI assistance disclosure checks.
- Obvious PII heuristics.
- Static no-network checks.
- Path normalization, file name, bundle size, MIME type, and executable-content constraints.
- Python checker execution limits.
- Validator failure format.
- Public provenance allowlist.

Pass criteria:

- Valid example bundles pass.
- Invalid bundles fail with clear structured errors.
- Validator behavior is deterministic and does not use model-based quality scoring.
- Artifact publication labels derive from central state and review records, not manifest self-claims.

### 011. Critique, Human Review, Publication, and Promotion Spec

Decision to make:

- Define how agent-driven verification and human review affect plan, artifact, and publication state.

Must specify:

- `CritiqueReport` schema.
- Agent-driven verification task types.
- Deterministic routing from critique fields to repair, re-review, plan verification, human review, or quarantine.
- Human review queues and review types.
- Reviewer roles, qualifications, conflict-of-interest rules, and trust levels.
- Review claim, lease, submission, finding lifecycle, severity semantics, quorum, appeal/reopen, and deprecation rules.
- Minimal MVP promotion from `machine_validated` to `peer_reviewed`.
- Full `classroom_ready`, localization, accessibility, and deprecation policies as deferred extensions unless required by the MVP slice.
- What model-generated critique cannot approve.

Pass criteria:

- AI critique never equals human approval.
- Backend routing from critique reports is deterministic.
- Sybil or conflict scenarios cannot promote a plan/artifact by using the same operator as planner, verifier, generator, and reviewer where independence is required.
- Public artifacts expose review state, license, AI assistance, known limitations, and approved provenance only.

### 012. MVP Guardrail and End-to-End Test Matrix

Decision to make:

- Define the full test matrix for the first implementation plan.

Must specify:

- End-to-end happy path.
- Denied request flows.
- Denied proposed graph flows.
- Denied promotion flows.
- Lease expiry, stale submit, duplicate submit, and retry tests.
- No-inference central dependency/env/network/database tests.
- No-leak API/log/error/publication tests.
- Runner redaction tests.
- Artifact validator valid/invalid bundle tests.
- Minimal review promotion tests.
- Exact local commands expected after stack selection.

Pass criteria:

- Tests prove request intake, planning-task creation, dummy planner proposal, deterministic validation/promotion, dummy generation, deterministic artifact validation, and minimal review state basics.
- Tests prove no provider SDK imports, no provider credential environment reads, no embedding/vector tables, no outbound model-provider calls, no central prompt execution, and no public leak of secret-like values.

### 013. Task-Scoped Code Execution, Sandbox, and Provenance Spec

Decision to make:

- Define how code-capable runners can generate and self-test code without treating user requests as execution authority.

Must specify:

- Task-scoped execution policy schema.
- Which MVP work packets may allow code generation and sandboxed self-tests.
- Runner sandbox profile for self-tests.
- Forbidden host access, network access, inherited secrets, and tool execution.
- Per-claim execution tokens or attestations.
- Artifact hashing and manifest digest rules.
- Runner self-test report schema.
- Signature or MAC strategy for runner attestations.
- Central verification of hashes, lineage, lease binding, and execution policy.
- Separation between runner self-tests and trusted validator results.

Pass criteria:

- Requests cannot grant execution privileges directly.
- Central policy assigns execution permission only to specific work packets after deterministic gates pass.
- Runner self-tests happen only inside a constrained per-claim sandbox.
- Runner self-test success cannot make an artifact `machine_validated`.
- Hashes/attestations bind output artifacts to task ID, lease ID, execution policy, runner actor, and submitted report.
- Tests prove tampering, replay, cross-task reuse, and forged self-test reports fail.

### 014. Secure Code Review, Critique, and Repair Runner Spec

Decision to make:

- Define how runners can review, critique, and repair generated code drafts without becoming validation or human-review authority.

Must specify:

- Code critique and repair task types.
- Teacher request preference for bounded automated repair.
- Volunteer runner-operator opt-in for automated repair loops.
- Sandbox profiles for code critique and repair.
- Signed critique, repair, and interruption report schemas.
- Deterministic central routing from critique/validation failures to repair work.
- Repair attempt and interruption continuation limits.
- Quota-exhaustion and operator-review handling.
- Rule that repair creates a new draft artifact, never in-place mutation.
- Trusted validator and human-review boundaries after repair.

Pass criteria:

- Critique reports are advisory only.
- Repaired artifacts must pass trusted validation before human review.
- Automated repair requires both teacher request preference and runner-operator opt-in.
- Runner quota exhaustion produces safe signed interruption evidence and bounded operator-reviewed continuation.
- Repair loops cannot run indefinitely or hide background work.
- Critique/repair output cannot promote, publish, approve, or validate artifacts.

## Initial Sequencing

Write and review specs in this order:

1. `001-mvp-slice-acceptance-tests.md`
2. `002-security-privacy-abuse-resistance-checklist.md`
3. `003-architecture-stack-no-inference-boundary.md`
4. `004-data-classification-redaction-logging-no-leak.md`
5. `005-core-data-model-state-machine.md`
6. `006-request-intake-planning-task-workflow.md`
7. `007-proposed-task-graph-validation-promotion-reconciliation.md`
8. `008-central-api-contract.md`
9. `009-runner-contract-capability-summary-local-config.md`
10. `010-artifact-manifest-validation-provenance.md`
11. `011-critique-human-review-publication-promotion.md`
12. `012-mvp-guardrail-end-to-end-test-matrix.md`
13. `013-task-scoped-code-execution-sandbox-provenance.md`
14. `014-secure-code-review-critique-repair.md`

Implementation planning may begin only after specs `001` through `008`, plus the no-inference/no-leak guardrail slice of spec `012`, pass review.

## Current Open Decisions for User Approval

None currently blocking spec drafting.
- Minimum human review gate before task graph promotion.
- Minimum human review gate before artifact `peer_reviewed`.
