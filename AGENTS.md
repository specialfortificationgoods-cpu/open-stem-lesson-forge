# Lesson Forge Agent Instructions

This repository is for Open STEM Lesson Forge, a distributed educational commons for generating, validating, reviewing, and publishing open STEM teaching materials.

Do not assume this is the Rust/Bevy/Avian3d/EGUI game project. That is a separate project.

## Architecture Contract

- Treat the deterministic-core design as the project direction.
- The central backend is a deterministic workflow engine: request intake, schema validation, policy checks, task leasing, artifact storage, deterministic validation, review state, and publication metadata.
- The central backend must not perform LLM inference, call model providers, run embeddings, store prompt templates intended for execution, or store/proxy provider credentials.
- All LLM inference and provider-specific prompt construction belongs in runners or explicitly separate non-core execution services.
- Runner outputs are untrusted until schema validation, deterministic policy checks, deterministic validation, duplicate/agent review where configured, and human review gates pass.

## Specification Workflow

- Treat design documents as direction-setting material, not final implementation specs.
- Before implementation, write focused specs that define data contracts, API behavior, validation rules, state transitions, and acceptance tests.
- Prefer robust, general-purpose mechanisms over special-case fixes.
- Keep request-to-task generation runner-first: planner-capable runners produce structured task graphs, and the central backend validates and promotes them deterministically.
- Include agent-driven verification and human-review-based verification in the request-to-task workflow.
- Keep a project work log while planning and building.
- Ask the user before making substantive design decisions. Log the decision and rationale after the user answers.

## Stack Guidance

- Choose the implementation stack from architectural fit, maintainability, and testability, not from inherited assumptions.
- Python/FastAPI is acceptable for a boring deterministic API and runner CLI if it remains the best fit.
- Rust is also acceptable where strong typing, single-binary deployment, performance, or sandbox/control boundaries justify it.
- Do not mix stacks casually. Each service/package should have a clear reason to exist.

## Documentation

- Use the project design/spec documents in this repository as the source of truth once they exist.
- If a style document exists in the repository, follow it.
- If a referenced style document is missing, say so and proceed conservatively rather than inventing hidden rules.
