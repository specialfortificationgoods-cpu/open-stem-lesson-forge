# Full Spec Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the passed Open STEM Lesson Forge specs as a Rust MVP with deterministic central core, local runners, validator, schemas, guardrail tools, and end-to-end fixture tests.

**Architecture:** Build outward from deterministic shared contracts. `lessonforge_core` owns state, policy, IDs, leases, redaction, and trust-boundary rules; `lessonforge_schema` owns JSON Schema validation and fixtures; `lessonforge_api` orchestrates deterministic workflow; `lessonforge_runner` provides dummy local runners; `lessonforge_validator` validates artifact bundles; tools enforce no-inference, schema, leak, and e2e gates.

**Tech Stack:** Rust 2024 Cargo workspace, serde/serde_json, thiserror, strict clippy lints, JSON Schema files as cross-boundary contracts, deterministic local fixtures, no model-provider SDKs in central crates.

---

## File Structure

- `Cargo.toml`: workspace members, shared dependencies, lint policy.
- `.gitignore`: local build output and tool cache ignores.
- `.clippy.toml`: workspace clippy policy.
- `deny.toml`: dependency/license/advisory policy for the final gate.
- `crates/lessonforge_core/src/ids.rs`: typed IDs and prefix validation.
- `crates/lessonforge_core/src/state.rs`: shared state enums and transitions from spec `005`.
- `crates/lessonforge_core/src/moderation.rs`: deterministic request moderation policy from spec `006`.
- `crates/lessonforge_core/src/planning.rs`: deterministic planning task records from specs `006` and `007`.
- `crates/lessonforge_core/src/graph.rs`: proposed graph policy, verification, and promotion from spec `007`.
- `crates/lessonforge_core/src/review.rs`: critique, human review, and publication policy from spec `011`.
- `crates/lessonforge_core/src/error.rs`: shared typed errors.
- `crates/lessonforge_core/src/code_repair.rs`: existing spec `014` code critique/repair contracts.
- `crates/lessonforge_schema`: schema validation crate and fixture checks.
- `crates/lessonforge_api`: deterministic API orchestration and in-memory/local storage adapter.
- `crates/lessonforge_runner`: dummy moderation/planner/verifier/generator/critic/repairer commands.
- `crates/lessonforge_validator`: deterministic artifact bundle validator.
- `tools/verify-no-inference-core`: static guardrail for no central inference/provider leakage.
- `tools/verify-schema-fixtures`: validates checked-in JSON fixtures against schemas.
- `tools/verify-no-leak-fixtures`: validates fixture/log/error no-leak behavior.
- `tools/lessonforge-e2e`: runs deterministic MVP vertical-slice suites.
- `schemas/`: JSON Schema contracts from specs.
- `examples/`: deterministic request/task/proposal/artifact/review fixtures.

## Phase 1: Workspace and Guardrail Foundation

**Files:**
- Modify: `Cargo.toml`
- Create: `.clippy.toml`
- Create: `deny.toml`
- Create: `crates/lessonforge_api/Cargo.toml`
- Create: `crates/lessonforge_api/src/lib.rs`
- Create: `crates/lessonforge_runner/Cargo.toml`
- Create: `crates/lessonforge_runner/src/lib.rs`
- Create: `crates/lessonforge_validator/Cargo.toml`
- Create: `crates/lessonforge_validator/src/lib.rs`
- Create: `crates/lessonforge_schema/Cargo.toml`
- Create: `crates/lessonforge_schema/src/lib.rs`
- Create: `tools/verify-no-inference-core/Cargo.toml`
- Create: `tools/verify-no-inference-core/src/main.rs`
- Create: `tools/verify-no-inference-core/tests/no_inference_core.rs`

- [ ] **Step 1: Write failing no-inference guardrail tests**

Create tests that run the `verify-no-inference-core` binary against:
- the real workspace, expected pass;
- a temporary crate containing forbidden provider/model markers, expected fail.

Run:

```bash
cargo test -p verify-no-inference-core
```

Expected: compile failure or test failure because the tool crate does not exist yet.

- [ ] **Step 2: Add workspace member shells**

Add the missing crates from spec `003` with no provider/model dependencies and simple public marker functions.

Run:

```bash
cargo test --workspace
```

Expected: the new crate shells compile; the guardrail tests still fail until the tool implementation exists.

- [ ] **Step 3: Implement `verify-no-inference-core`**

The tool must scan central deterministic crates:
- `crates/lessonforge_core`
- `crates/lessonforge_api`
- `crates/lessonforge_schema`

It must fail on provider/model markers in source or manifests:
- `openai`
- `anthropic`
- `gemini`
- `mistral`
- `ollama`
- `llama`
- `embedding`
- `vector`
- `prompt_template`
- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `provider_base_url`
- `model_provider`

The first implementation is deterministic string scanning. Later phases can replace this with AST/dependency graph scanning if needed.

Run:

```bash
cargo test -p verify-no-inference-core
cargo run -p verify-no-inference-core
```

Expected: tests pass and the workspace scan exits `0`.

- [ ] **Step 4: Run phase verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p verify-no-inference-core
```

Expected: all pass.

- [ ] **Step 5: Review gate**

Dispatch adversarial reviewers:
- deterministic-core/no-inference reviewer;
- MVP/stack reviewer;
- security reviewer.

Expected: all return PASS or concrete blockers. Fix blockers with new failing tests before implementation changes.

## Phase 2: Core IDs, Actors, Leases, and State Machines

**Files:**
- Create/modify `crates/lessonforge_core/src/ids.rs`
- Create/modify `crates/lessonforge_core/src/state.rs`
- Create/modify `crates/lessonforge_core/src/error.rs`
- Test under `crates/lessonforge_core/tests/core_state_machine.rs`

Implement:
- ID prefix validation from spec `005`.
- Actor type, trust level, capability enums.
- Request, moderation task, planning task, proposal, verification task, work packet, artifact, validation report, review task, review, finding, and public-label states.
- Typed transition helpers that reject invalid transitions.
- Lease states and token-returned-once semantics as pure deterministic domain logic.

## Phase 3: Request Intake, Moderation, and Planning Workflow

**Files:**
- `crates/lessonforge_core/src/request.rs`
- `crates/lessonforge_core/src/moderation.rs`
- `crates/lessonforge_core/src/planning.rs`
- `crates/lessonforge_api/src/workflow.rs`
- tests under `crates/lessonforge_core/tests/request_workflow.rs`

Implement specs `006`, `001` request fixture, and moderation gate:
- deterministic intake validation;
- moderation task creation before planning;
- moderation report acceptance/rejection;
- planning task creation only after moderation pass;
- prompt injection and PII/secret-like denial rules.

## Phase 4: Proposed Task Graph, Verification, and Promotion

**Files:**
- `crates/lessonforge_core/src/graph.rs`
- `crates/lessonforge_core/src/planning.rs`
- schemas and examples for proposed graphs and plan verification.

Implement spec `007`:
- closed proposed-task graph model;
- deterministic policy validation;
- plan verification task creation;
- verifier independence checks;
- idempotent promotion into work packets.

## Phase 5: Schema and Fixture Validation

**Files:**
- `crates/lessonforge_schema`
- `schemas/*.schema.json`
- `examples/**`
- `tools/verify-schema-fixtures`

Implement schema loading, validation, and checked-in fixtures for the MVP slice.

## Phase 6: Artifact Bundle Validator

**Files:**
- `crates/lessonforge_validator`
- artifact-bundle examples

Implement spec `010`:
- manifest validation;
- file digest and bundle digest validation;
- deterministic checker static safety;
- trusted validator report model;
- no public provenance leakage.

## Phase 7: Dummy Runners

**Files:**
- `crates/lessonforge_runner`

Implement spec `009` dummy modes:
- `dummy_moderator`
- `dummy_planner`
- `dummy_verifier`
- `dummy_generator`
- `dummy_code_critic`
- `dummy_code_repairer`
- `dummy_code_repairer_auto_loop`

Provider-backed runners remain deferred.

## Phase 8: API Orchestration and Storage

**Files:**
- `crates/lessonforge_api`

Implement spec `008` deterministic command handlers against an in-memory storage adapter first:
- request create/status;
- task claim/heartbeat/release/submit;
- promotion;
- artifact submission;
- validation submission;
- review submission.

Spec `014` critique, repair, and interruption ingestion is post-MVP for the central API. Spec `012` keeps `CR-GATE-*` rows deferred and requires negative-unavailable checks until a later focused spec revision activates those surfaces with matching data contracts and acceptance tests.

## Phase 9: Review, Publication, and Public Labels

**Files:**
- `crates/lessonforge_core/src/review.rs`
- `crates/lessonforge_api/src/workflow.rs`

Implement spec `011`:
- review task state;
- conflict checks;
- finding policy;
- peer-reviewed promotion;
- public artifact label derivation.

## Phase 10: Full Gates and End-to-End MVP

**Files:**
- `tools/lessonforge-e2e`
- `tools/verify-no-leak-fixtures`
- `deny.toml`

Implement spec `012` full command contract:

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

## Review Rules for Every Phase

- Write tests before implementation.
- Run the phase verification command set.
- Dispatch targeted adversarial subagents.
- Use CodeRabbit review when the Git repository has a valid `HEAD` or a reviewable diff target.
- Update `docs/work-log.md` with implementation progress, review findings, and decisions.
- Do not mark the full implementation goal complete until every phase passes the full gate.
