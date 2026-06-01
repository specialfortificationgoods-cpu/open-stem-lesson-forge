# 003. Architecture, Stack, and No-Inference Boundary Spec

Status: Passed adversarial review  
Roadmap: `docs/specs/000-spec-roadmap.md`  
Depends on:

- `docs/specs/001-mvp-slice-acceptance-tests.md`
- `docs/specs/002-security-privacy-abuse-resistance-checklist.md`

## Purpose

This spec chooses the MVP service boundaries, implementation stack, schema ownership model, local development commands, and no-inference guardrails for Open STEM Lesson Forge.

The approved MVP stack is Rust-first. The project may borrow Rust workspace and verification practices from `clan_guild_creed`, but must not inherit game-specific Bevy/Avian3d/EGUI assumptions or dependencies.

## User-Approved Stack Decision

Decision: **Rust MVP**.

Scope:

- Central API in Rust.
- Runner CLI in Rust.
- Deterministic validator in Rust.
- Shared schemas and contract tests in the Rust workspace.
- JSON Schema remains the cross-boundary contract format.

Rationale:

- The project is trust-boundary and state-machine heavy.
- Rust gives strong types for workflow state, policy outcomes, leases, identifiers, and validation results.
- Rust supports strict compile-time and lint guardrails against unsafe code, panics, ignored results, and accidental dependency drift.
- A single-language Rust workspace can keep API, runner, validator, schemas, and guardrail tooling coherent.
- Later hardened validator and sandbox components are likely easier to evolve from a Rust-first base than from a Python-first MVP.

Accepted tradeoffs:

- Rust may be slower than Python/FastAPI for early API iteration.
- JSON Schema and web ergonomics need deliberate crate choices.
- The design document originally suggested Python/FastAPI as the boring default, but the user prefers trying Rust and borrowing established local Rust project practices.

## Borrowed Rust Practices

Borrow from `clan_guild_creed` only where they improve robustness for this platform:

- Use a Cargo workspace with domain crates under `crates/` and policy/verification tools under `tools/`.
- Apply strict workspace lints.
- Forbid unsafe code in normal crates.
- Deny `unwrap`, `expect`, `panic`, `dbg!`, and ignored fallible results in application/library code.
- Prefer explicit error types with `thiserror`.
- Keep functions focused and testable.
- At trust boundaries and externally influenced state transitions, invalid or unexpected input returns typed structured errors and safe audit events, not panics.
- Assertions are allowed only for unreachable internal invariants after validation and only where external input cannot trigger them; otherwise use `Result`.
- Keep dependency additions intentional and reviewable.
- Prefer small, clear modules over large mixed-responsibility files.
- Treat verification tooling as part of the product, not as an afterthought.

Do not borrow:

- Bevy, Avian3d, EGUI, game-loop allocation policy, or game-specific crates.
- `cgc_` crate naming.
- Game-specific Power-of-10 assertion thresholds as mandatory for this MVP before a Lesson Forge-specific lint policy exists.

## MVP Slice to Support

The architecture must support the first slice from `001`:

- Public request intake for a physics conservation-of-energy lesson pack.
- Mechanical creation of a planning task.
- Dummy planner submission of a `ProposedTaskGraph`.
- Deterministic schema/policy validation.
- Deterministic creation of a plan verification task.
- Dummy verifier submission of advisory plan verification.
- Deterministic, idempotent promotion into work packets.
- Dummy worker generation of worksheet, answer key, Python checker, teacher notes, and manifest.
- Deterministic artifact validation.
- Minimal human review promotion from `machine_validated` to `peer_reviewed`.

The architecture must also satisfy the security checklist in `002`, including no central inference, no credential leakage, identity/authorization/replay controls, public artifact serving restrictions, outbound-network/SSRF controls, and abuse/availability controls.

## Workspace Layout

The MVP repository layout is:

```text
Cargo.toml
deny.toml
.clippy.toml

crates/
  lessonforge_core/
    src/
      lib.rs
      ids.rs
      state.rs
      error.rs
      request.rs
      moderation.rs
      planning.rs
      graph.rs
      review.rs
      code_repair.rs
      validation/
        mod.rs
        pii_detection.rs
  lessonforge_api/
    src/
      lib.rs
      workflow.rs
  lessonforge_runner/
    src/
      lib.rs
  lessonforge_validator/
    src/
      lib.rs
  lessonforge_schema/
    src/
      lib.rs

tools/
  lessonforge-e2e/
    src/main.rs
  verify-no-inference-core/
    src/main.rs
  verify-no-leak-fixtures/
    src/main.rs
  verify-schema-fixtures/
    src/main.rs

schemas/
  request.schema.json
  request_moderation_report.schema.json
  proposed_task_graph.schema.json
  plan_verification.schema.json
  artifact_manifest.schema.json

examples/
  mvp/
    request.valid.json
    request_moderation_report.valid.json
    proposed_task_graph.valid.json
    plan_verification.valid.json
    artifact_manifest.valid.json

docs/
  specs/
  superpowers/plans/
```

## Crate Responsibilities

### `lessonforge_core`

Owns deterministic domain types and rules:

- Strongly typed IDs.
- State enums.
- Transition helpers.
- Policy outcome types.
- Lease and idempotency primitives.
- Shared error types.

Must not:

- Import web framework crates.
- Import runner provider adapters.
- Import model-provider SDKs.
- Read environment variables.
- Perform network I/O.
- Store or execute prompt templates.

### `lessonforge_api`

Owns deterministic central API:

- Request intake.
- Planning task creation.
- Proposal validation and promotion orchestration.
- Lease endpoints.
- Artifact metadata records.
- Review records.
- Public status labels.
- Storage adapters for local SQLite and later PostgreSQL.

Must not:

- Perform model inference.
- Fetch attacker-controlled URLs in the MVP.
- Import runner provider adapters.
- Accept provider credentials or full runner config.
- Execute generated code.

### `lessonforge_runner`

Owns local runner behavior:

- Local config parsing.
- Redacted capability summaries.
- Dummy planner.
- Dummy verifier.
- Dummy generator.
- Local prompt templates in later provider specs.
- Provider adapter trait boundaries in later specs.

May later depend on provider adapters, but those dependencies must not enter `lessonforge_core` or `lessonforge_api`.

### `lessonforge_validator`

Owns deterministic artifact-bundle validation:

- Bundle traversal.
- Path normalization.
- Manifest validation.
- Required-file checks.
- License/provenance checks.
- Obvious PII heuristics.
- Static no-network checks.
- Restricted `checker.py` execution orchestration.
- Validation report generation.

The MVP validator remains deterministic and must not use model-based quality scoring.

Generated-code execution must not run inside the central API process. If `checker.py` execution is implemented in the MVP, it must run in a sandboxed validator worker or child process with:

- No central database access.
- No central API tokens or session secrets.
- No inherited provider credentials.
- No network access.
- Restricted filesystem access rooted to a temporary artifact-bundle directory.
- CPU, memory, process, and wall-clock time limits.
- Report-only communication back to the validator/API boundary.

If this isolation is not implemented for the MVP, `checker.py` execution must be disabled and the validator may perform only static checks until `010-artifact-manifest-validation-provenance.md` defines the execution boundary.

For the first local Rust MVP gate, a reviewed constrained-subprocess validator
profile may stand in for the stronger future OS sandbox only for the closed MVP
checker contract. That profile must run outside the central API process, pass
conservative static checks before import, use isolated Python with empty
environment and empty stdin, enforce wall-clock/CPU limits before import, deny
new file descriptors before invoking checker functions, and reject any stdout,
stderr, nonzero exit, static network/file/process marker, top-level side effect,
loop escape, or checker-contract mismatch.

This constrained profile is not sufficient for arbitrary generated code beyond
the closed MVP checker contract. A later non-MVP execution profile must add a
stronger OS sandbox, container, or equivalent filesystem/network isolation before
expanding executable artifact scope.

MVP acceptance for the constrained profile is implemented by
`lessonforge_validator` under `CheckerExecutionMode::SandboxedSubprocess`.
The automated gate must verify conservative static rejection of network,
filesystem, subprocess, dynamic import, top-level side-effect, loop escape, and
checker-contract mismatch cases; empty stdin/environment subprocess execution;
wall-clock and CPU limits before import; file-descriptor denial before checker
function calls; empty stdout/stderr pass conditions; and safe failure codes such
as `python_checker_static_safety_failed`, `no_external_network_static_failed`,
and `python_checker_runs_failed`.

### `lessonforge_schema`

Owns schema loading and validation helpers:

- Loads JSON Schema files from `schemas/`.
- Validates examples and submitted structures.
- Provides schema version constants.

The JSON Schema files are the canonical wire-contract source for MVP. Rust types may mirror them, but later tests must prove fixtures validate against schemas.

### `tools/verify-no-inference-core`

Owns static guardrails:

- Fails if central crates depend on forbidden model-provider or inference crates.
- Fails if central crates reference forbidden provider credential environment variables.
- Fails if central migrations/schema files include embedding/vector/provider credential/prompt execution fields.
- Fails if central code contains known direct inference entrypoint names.

### `tools/verify-schema-fixtures`

Owns schema fixture verification:

- Validates examples under `examples/` against JSON schemas.
- Fails on invalid fixtures.

## Supply-Chain Guardrails

The Rust MVP must include supply-chain checks in the implementation gate:

- Commit and review `Cargo.lock`.
- Run `cargo deny check` or an equivalent policy gate.
- Deny known vulnerable and yanked crates unless an explicit reviewed exception exists.
- Restrict registries, git sources, and path dependencies.
- Use a license allowlist compatible with the project.
- Require review for build scripts, procedural macros, FFI crates, native TLS/crypto crates, and crates with unsafe transitive code.
- Include transitive dependencies and feature flags in central dependency graph checks.
- Keep provider/inference forbidden-dependency checks separate from general supply-chain checks.

These rules are architecture requirements. Exact `deny.toml` policy belongs to the implementation plan or a later dependency-policy spec if needed.

## Rust Workspace Lints

The workspace should start with strict lints similar in spirit to `clan_guild_creed`:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
unused_qualifications = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
dbg_macro = "deny"
todo = "warn"
unimplemented = "warn"
print_stdout = "deny"
print_stderr = "deny"
undocumented_unsafe_blocks = "deny"
```

Rules:

- Normal crates use `#![forbid(unsafe_code)]`.
- If a future sandbox/FFI crate requires unsafe code, it must be isolated, documented, and separately reviewed.
- Public fallible APIs return typed `Result`.
- Library code must not panic.
- Test code may use test-only conveniences where explicitly allowed by lint configuration.

## Candidate Crate Families

Specific dependencies are implementation details, but the Rust MVP should prefer:

- Web/API: Axum or another small Rust HTTP framework.
- Serialization: `serde`.
- Error types: `thiserror`.
- Application errors at binary boundaries: `anyhow` only in binaries/tools, not core domain APIs.
- CLI: `clap`.
- SQLite: `sqlx` or another async-compatible Rust SQL layer.
- JSON Schema validation: a maintained Rust JSON Schema crate.
- Time: `time` or `chrono`, selected later.
- UUID/IDs: a crate selected later, or deterministic string IDs for fixtures.

Dependency rules:

- No model-provider SDKs in `lessonforge_core`, `lessonforge_api`, `lessonforge_schema`, or any shared crate imported by those crates.
- No inference orchestration frameworks in central crates.
- No embedding/vector database clients in MVP central crates.
- New dependencies must be justified by the spec or implementation plan that introduces them.

## Schema Source of Truth

For MVP, JSON Schema files are canonical.

Rules:

- JSON Schema files live in `schemas/`.
- Rust domain types must serialize/deserialize consistently with schemas.
- Examples in `examples/` validate against schemas.
- Schema version constants live in `lessonforge_schema`.
- Schema validation errors are structured and safe to expose.
- Language-native Rust types must not silently diverge from JSON Schema.

Rationale:

- JSON Schema keeps API, runner, validator, fixtures, and future non-Rust integrations aligned.
- It preserves the provider-agnostic work-packet boundary.
- It allows future client SDKs without making Rust types the only contract.

## Persistence

For MVP local development:

- Use SQLite.
- Keep schema design compatible with PostgreSQL where reasonable.
- Store only deterministic central records and approved internal metadata.
- Do not store model prompts, provider credentials, full provider config, local auth paths, or secret-like values.
- Do not store embeddings or vector indexes.

Later deployment:

- PostgreSQL is the expected production path.
- Object storage can be added for artifact bundles after local filesystem MVP.

## Local Development Commands

Expected Rust command families:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo run -p verify-no-inference-core
cargo run -p verify-schema-fixtures
```

Later implementation plans must replace these command families with exact commands and expected output once project files exist.

## No-Inference Boundary

The central API and deterministic core must not:

- Import model-provider SDKs.
- Import provider adapters.
- Read provider credential environment variables.
- Call model-provider domains.
- Call model moderation endpoints.
- Fetch attacker-controlled URLs in the MVP.
- Run embeddings or vector search in MVP.
- Store executable prompt templates.
- Store raw prompt fields intended for model execution.
- Execute central prompt construction for runners.
- Perform semantic request decomposition.
- Perform semantic plan merge or reconciliation.
- Select exact volunteer model/provider.

Provider-specific logic belongs only in:

- `lessonforge_runner`, for local volunteer runner behavior.
- `lessonforge_runner::moderation` or a later separate non-core moderation service, for provider-backed moderation evidence.
- A later separately specified non-core project runner/service, if approved.

## Forbidden Central Dependencies and Names

Guardrails must fail if `lessonforge_core`, `lessonforge_api`, `lessonforge_schema`, the central dependency graph, or any shared crate imported by central crates includes:

- OpenAI SDKs.
- Anthropic SDKs.
- Gemini SDKs.
- Ollama/LM Studio client libraries.
- LangChain/LlamaIndex equivalents.
- Model-provider SDKs.
- Inference orchestration frameworks.
- Embedding/vector database clients for MVP.
- Provider adapters.

Guardrails must fail if central-core code paths are named or structured as direct inference entrypoints, including:

- `call_llm`
- `generate_with_model`
- `embed`
- `infer`
- `plan_with_model`
- `model_moderate`
- `semantic_merge`

Name checks are a backstop; dependency, environment, API, migration, and behavior checks are authoritative.

## Forbidden Central Environment Variables

The central API must not require, read, or document provider credential variables such as:

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `GEMINI_API_KEY`
- Provider refresh tokens.
- ChatGPT/Codex auth file paths.
- Browser-cookie paths.
- Local model service secrets.

The runner may use local provider credentials in later specs, but those remain local and must not be transmitted to the central API.

## Database and Migration Guardrails

MVP central-core schema must not include:

- Embedding columns.
- Vector indexes.
- Provider credential fields.
- Raw prompt-template fields intended for execution.
- Full local runner config.
- Local auth paths.
- Secret-like rejected payload storage.

MVP central-core schema may include:

- Request records.
- Planning task records.
- Proposed task graph records after validation/redaction.
- Plan verification records after validation/redaction.
- Work packet records.
- Artifact metadata records.
- Validation report records from trusted deterministic validator path.
- Review records after validation/redaction.
- Audit events with safe reason codes.

## Network Guardrails

MVP central core:

- Must not call model-provider domains.
- Must not fetch attacker-controlled URLs.
- May serve its API and local artifact downloads according to later artifact-serving specs.

If a later reviewed spec adds central URL fetching, it must satisfy `002` outbound network and SSRF checks.

## Security Checklist Coverage

Applicable checklist categories from `002`:

- Central-Core Boundary Checks: satisfied by no-inference, forbidden dependency/env/database/network guardrails.
- Data Classification Checks: partially satisfied here by forbidden central persistence rules; detailed field allowlists belong to `004`.
- Request Intake and Prompt-Injection Checks: central semantic decomposition is forbidden here; detailed request behavior belongs to `006`.
- Outbound Network and SSRF Checks: MVP central fetching of attacker-controlled URLs is forbidden here.
- Identity, Authorization, and Replay Checks: endpoint-level details belong to `008`.
- Runner Submission Trust Checks: provider config stays out of central core here; detailed runner contract belongs to `009`.
- Plan Verification Gate Checks: task and promotion behavior belongs to `005`, `007`, and `008`.
- Artifact and Generated-Code Checks: validator/runtime details belong to `010`.
- Public Artifact Serving Checks: serving details belong to `010`.
- Logging, Error, and Audit Checks: detailed allowlists belong to `004` and `008`.
- Provider-Terms and Public Framing Checks: public copy and onboarding details belong to later web/onboarding specs.
- Abuse, Quota, and Availability Checks: endpoint-level controls belong to `008`.

No applicable non-deferrable checklist item is deferred for the architecture surface.

## Acceptance Tests for This Spec

Later implementation must include tests proving:

- The central dependency graph excludes model-provider SDKs, inference frameworks, provider adapters, embedding/vector clients, and model orchestration libraries.
- The central dependency graph check covers transitive dependencies and feature flags.
- Supply-chain checks deny vulnerable/yanked crates, unexpected sources, and license violations.
- Build scripts, proc macros, FFI, native TLS/crypto, and unsafe transitive crates require explicit review.
- Central crates do not read provider credential environment variables.
- Central crates have no migrations or schemas for embeddings, vectors, provider credentials, or executable central prompts.
- Request intake creates planning tasks mechanically and does not create semantic work packets.
- The runner crate may define local provider-adapter interfaces without those dependencies entering central crates.
- Schema examples validate against `schemas/`.
- Rust domain types round-trip accepted JSON fixtures without losing required fields.
- The accepted MVP slice from `001` can run with dummy runner behavior and no real model-provider access.
- If `checker.py` execution is enabled, tests prove it runs outside the central API process with no central DB access, no central secrets, no network, temp-bundle-rooted filesystem access, and CPU/memory/process/time limits.
- If that generated-code isolation is not implemented, tests prove `checker.py` execution is disabled and only static validation runs.
- `cargo fmt`, `cargo clippy`, `cargo test`, `cargo deny check`, `verify-no-inference-core`, and `verify-schema-fixtures` are part of the implementation gate.

## Review Checklist

Reviewers should fail this spec if:

- It requires central model inference.
- It allows provider dependencies in central crates.
- It lets Rust type definitions replace JSON Schema as the only contract.
- It imports game-project dependencies or Bevy-specific assumptions.
- It omits no-inference, no-leak, no-SSRF, or no-provider guardrails.
- It makes unsafe code acceptable in normal crates.
- It allows untrusted input or externally influenced state transitions to panic instead of returning structured errors.
- It allows generated code execution inside the central API process or with access to central DB/secrets/network.
- It omits supply-chain policy checks for Rust dependencies.
- It makes implementation depend on real model-provider credentials.
