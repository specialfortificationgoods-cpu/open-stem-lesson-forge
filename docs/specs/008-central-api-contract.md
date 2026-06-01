# 008. Central API Contract Spec

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

## Purpose

This spec defines the deterministic central API contract for the Rust MVP.

The API exposes structured workflow surfaces for request intake, planning task leases, proposal validation, plan verification, promotion, work-packet claims, artifact submission, validation evidence, human review, and public status. It must not expose model-provider execution, prompt execution, provider credential handling, semantic decomposition, embeddings, semantic search, or runner-local provider configuration.

## API Scope

The MVP API may expose only these resource groups:

- `/v1/requests`
- `/v1/request-moderation-tasks`
- `/v1/planning-tasks`
- `/v1/proposals`
- `/v1/plan-verification-tasks`
- `/v1/work-packets`
- `/v1/artifacts`
- `/v1/validation-work-packets`
- `/v1/validation-reports`
- `/v1/review-tasks`
- `/v1/reviews`
- `/v1/public/artifacts`
- `/v1/events`
- `/v1/events/stream`
- `/v1/health`

No endpoint accepts or returns provider API keys, provider base URLs, provider names, model names, executable prompt templates, auth paths, cookies, raw headers, local absolute paths, embeddings, vector IDs, or arbitrary tool-call payloads.

## Transport and Encoding

MVP transport:

- HTTP/1.1 or HTTP/2 over localhost or configured private deployment endpoint.
- HTTPS is mandatory for any non-loopback deployment.
- Plain HTTP is allowed only for loopback development addresses such as `127.0.0.1`, `::1`, or Unix-domain-socket proxying.
- JSON request and response bodies.
- UTF-8 only.
- `Content-Type: application/json` required for all non-empty request bodies.
- `Accept: application/json` required unless the endpoint has no body.
- Maximum JSON body size is 256 KiB unless a later artifact-upload spec defines a separate bundle-upload surface.

Rejected transport cases:

- Non-JSON body on JSON endpoint.
- Multipart upload.
- Form encoding.
- XML/YAML/TOML payloads.
- Raw file upload to central API in specs `001` through `008`.
- URL-based import or fetch.

Artifact bundle bytes are not uploaded through this spec. The MVP artifact submission endpoint accepts only a deterministic opaque artifact-intake ID or test fixture ID defined by spec `010`. Until spec `010` passes, artifact endpoints are contract placeholders for MVP state tests only.

The API-visible artifact intake reference must be an opaque ID such as `aintake_...`, not a filesystem path, URL, archive name, or user-supplied filename. Only the validator/artifact subsystem may map that opaque ID to local storage after spec `010` defines the storage boundary.

## Command Transport Choice

MVP mutation and read commands use request/response HTTP JSON endpoints, not WebSocket commands.

Rationale:

- Idempotency keys are explicit per command.
- HTTP status codes map cleanly to deterministic validation, stale lease, authorization, and conflict outcomes.
- Transaction boundaries are one command at a time.
- Replay testing is simpler.
- API logs can be redacted per command.

Real-time updates are optional and read-only in the MVP:

- Server-Sent Events may be exposed at `GET /v1/events/stream` for authorized operator/test clients.
- The event stream sends the same redacted `StateTransitionEvent` projection as `GET /v1/events`.
- The event stream must not accept client commands, tokens, proposal payloads, review payloads, or artifact data.
- Dropped event connections are resumed by cursor or `Last-Event-Id`, not by replaying mutations.

WebSocket is deferred unless a later reviewed spec requires bidirectional interactive operation. A future WebSocket spec must preserve all requirements from this API contract:

- TLS outside loopback.
- Authenticated actor binding at connection setup and per command where commands are allowed.
- Per-command idempotency keys.
- Per-command source-state guards.
- Per-command redacted errors.
- No long-lived capability elevation from a connection.
- No provider credentials, prompts, model calls, embeddings, URL fetches, or local path disclosure.
- Backpressure, message size limits, heartbeat, idle timeout, and replay behavior.

WebSocket must not become a bypass around REST command validation.

## HTTP Security Requirements

Non-loopback deployments must use:

- TLS 1.2 or newer, preferably TLS 1.3.
- HSTS when served from a stable HTTPS origin.
- Secure cookies if cookies are ever introduced.
- `Authorization` headers or mTLS-bound identity; never auth in query strings.
- `Cache-Control: no-store` on authenticated API responses unless a later public-cache spec says otherwise.
- Strict CORS allowlist; wildcard CORS is forbidden for authenticated endpoints.
- Request body size limits before JSON parsing.
- Per-actor and per-IP deterministic rate limits for mutating, read/list/poll, SSE, and public endpoints.
- Redacted access logs.

Availability controls:

- List and poll endpoints have per-actor and per-IP request limits.
- Runner claimable-task polling has a minimum poll interval or token-bucket equivalent.
- Cursor replay is throttled by actor, scope, cursor ID, and source IP.
- SSE streams have per-actor and per-IP connection caps.
- SSE streams have idle timeout, maximum connection duration, server-side send buffer limits, and backpressure disconnect behavior.
- Public artifact `404` responses are throttled to limit enumeration.
- `429` responses use safe static messages and do not reveal whether a hidden resource exists.

Browser-facing deployments must additionally define CSRF protection before cookie-authenticated mutations are enabled. The MVP may avoid CSRF complexity by using non-cookie bearer or mTLS auth for local operators/runners.

## Authentication and Actor Binding

MVP auth may be local/static, but it must be explicit and typed.

Allowed auth mechanisms:

- Development-only static bearer token mapped to an `Actor`.
- Local mTLS or reverse-proxy authenticated actor header in trusted development deployment.
- Test harness actor injection behind a compile-time or config-gated test mode.

Rejected auth behavior:

- Actor identity from request body.
- `reviewer_id`, `planner_runner_id`, `verifier_runner_id`, `validator_actor_id`, or `created_by_actor_id` accepted as authority from JSON.
- Unauthenticated mutation except public request submission when configured.
- Provider credentials as authentication to central API.

Reverse-proxy actor headers:

- Accepted only when the connection comes from a configured trusted proxy identity or mTLS-authenticated peer.
- Spoofable inbound actor headers from ordinary clients must be stripped before API handling or rejected with `400 spoofed_actor_header`.
- Trusted proxy configuration must bind allowed source address or peer certificate identity.
- Actor headers are ignored on direct client connections.

Test actor injection:

- Must be compile-time gated or require an explicit test profile.
- Must fail startup if enabled on a non-loopback listener.
- Must fail startup if enabled with production persistence configuration.
- Must be visibly reported only in local health diagnostics that do not expose secrets.

Every authenticated request resolves to:

- `actor_id`
- `actor_type`
- `scope_id`
- `trust_level`
- `capabilities`
- `status`

Submitted actor IDs in payloads are checked against authenticated context and server-derived lease lineage. They are never authority by themselves.

## Request Metadata

All mutating endpoints require:

- `Idempotency-Key` header.
- Auth context except public request submission.

`Idempotency-Key` rules:

- 8 to 128 visible ASCII characters.
- No whitespace.
- No URL syntax.
- No secret-like token patterns.
- Scoped by endpoint, actor, scope, entity ID, lease ID where applicable, and redacted canonical command hash.
- Same key and identical canonical body returns the original result.
- Same key and different canonical body returns `409 idempotency_conflict`.
- Replayed successful commands do not create duplicate records or transition events.

The redacted canonical command hash excludes raw `claim_token`, bearer tokens, cookies, secret-like values, and any field classified as `Secret` or `Forbidden` by spec `004`. Raw `claim_token` values are never persisted or re-displayed. A `Lease` may persist non-recoverable server-side verification material required by spec `005`, such as fixed-format claim-token hashes or fingerprints, so deterministic lease-token verification and idempotency lookup can work without storing raw secrets.

Claim replay is special:

- The first successful claim response returns `claim_token`.
- Idempotent replay of the same successful claim returns lease metadata, entity ID, lease slot where applicable, and expiry.
- Claim replay never returns or re-displays `claim_token`.
- A client that loses the token must release by administrative recovery or wait for expiry; the central API does not reveal it again.

All responses include:

- `X-Request-Id`: server-generated trace identifier safe for logs.
- `X-Schema-Version`: response schema version.

`X-Request-Id` is not an authorization token and must not encode actor, tenant, local path, provider, or payload details.

## Error Format

All non-2xx JSON errors use:

```json
{
  "error": {
    "code": "string_enum",
    "message": "safe static message",
    "details": [
      {
        "code": "string_enum",
        "field_path": "$.schema.derived.path",
        "expected": "optional safe enum or type",
        "actual_classification": "optional safe classification"
      }
    ],
    "request_id": "reqtrace_..."
  }
}
```

Rules:

- `message` is static or template-free safe text.
- `field_path` is schema-derived, not copied from attacker-controlled keys.
- Errors never include raw rejected values.
- Errors never include secrets, local paths, provider config, prompt text, model names, URLs, lease tokens, claim token hashes, stack traces, SQL, filesystem paths, or student PII.
- Unknown errors return `500 internal_error` with no raw exception detail.

HTTP status mapping:

| HTTP | Use |
|---|---|
| 200 | Successful read or idempotent replay returning existing result. |
| 201 | New resource created. |
| 202 | Command accepted and deterministic asynchronous follow-up required. |
| 204 | Successful no-body release/heartbeat where configured. |
| 400 | Malformed JSON or schema failure. |
| 401 | Missing or invalid authentication. |
| 403 | Authenticated actor lacks authority, trust, capability, or scope. |
| 404 | Resource not found or not visible in actor scope. |
| 409 | Idempotency conflict, state conflict, duplicate active claim, or stale transition. |
| 410 | Lease expired, released, revoked, consumed, or replaced. |
| 413 | Body too large. |
| 415 | Unsupported media type. |
| 422 | Deterministic policy validation failure. |
| 429 | Deterministic quota/abuse limit. |
| 500 | Redacted internal error. |

## Pagination and Filtering

List endpoints use cursor pagination:

- `limit`: integer 1 to 100, default 50.
- `cursor`: opaque server token.
- `sort`: fixed per endpoint, newest first unless specified.

Cursor rules:

- Cursor is opaque, integrity-protected, and scoped to actor/scope/filter.
- Cursor must not encode raw query text, secrets, local paths, or provider data in decodable form.
- Invalid cursor returns `400 invalid_cursor`.

Allowed filters are endpoint-specific enums and IDs. Free-text search, semantic search, embedding search, and arbitrary query expressions are not in MVP.

## Public Request Endpoints

### `POST /v1/requests`

Purpose: submit a teacher request.

Auth:

- May allow unauthenticated `public_requester` in local MVP.
- If authenticated, actor is bound from auth context.

Request body:

- Exactly the request input fields from spec `006`.

Forbidden fields:

- `attachments`
- `attachment_urls`
- `source_url`
- `prompt`
- `arbitrary_prompt`
- provider/model/credential/local path fields from spec `004`
- any unknown top-level field

Source states:

- none.

Side effects:

- deterministic intake validation;
- create `Request`;
- create mechanical `RequestModerationTask`;
- create safe transition events;
- internally advance request to `moderation_pending`.

Response `201`:

```json
{
  "request": {
    "id": "req_energy_001",
    "public_status": "requested",
    "internal_state": "moderation_pending",
    "subject": "physics",
    "topic": "conservation_of_energy",
    "age_range": "14-16",
    "language": "en",
    "desired_artifacts": ["worksheet", "answer_key", "python_checker", "teacher_notes"],
    "visibility": "public",
    "auto_repair_preference": "no_automated_repair"
  },
  "request_moderation_task": {
    "id": "rmtask_energy_001",
    "state": "open",
    "task_type": "moderate_request",
    "required_output_schema": "request_moderation_report.schema.json"
  }
}
```

`public_status=requested` is a response projection for spec `001`; it is not central state authority.

## Request Moderation Endpoints

Request moderation is a runner/service evidence gate before planning. It is not central model inference.

The central API must not call OpenAI or any other moderation provider. Provider-backed moderation belongs to a runner or separate non-core service that submits only the safe report projection below.

### `GET /v1/request-moderation-tasks`

Purpose: list claimable or visible request moderation tasks.

Auth:

- actor type `runner` with moderation capabilities, `curator`, `admin`, or test harness actor.

Allowed filters:

- `state`
- `scope_id`
- `request_id`

Response includes only:

- task ID;
- request ID;
- safe request metadata needed for moderation;
- required output schema;
- required capabilities;
- lease availability metadata.

Response excludes provider config, raw rejected payloads, secrets, local paths, and private audit data.

### `POST /v1/request-moderation-tasks/{task_id}/claim`

Purpose: claim a request moderation task.

Auth:

- actor type `runner`;
- capabilities include `content_moderation` and `age_appropriateness_classification`;
- trust level at least `moderation_candidate`;
- same scope;
- actor status active.

Request body:

```json
{
  "runner_id": "runner_dummy_moderator_001"
}
```

Submitted `runner_id` is evidence only and must match authenticated context.

Side effects:

- create active lease;
- mark task `claimed`;
- return claim token once.

### `POST /v1/request-moderation-tasks/{task_id}/heartbeat`

Same heartbeat contract as planning tasks, single-lease surface.

### `POST /v1/request-moderation-tasks/{task_id}/release`

Same release contract as planning tasks, single-lease surface.

### `POST /v1/request-moderation-tasks/{task_id}/reports`

Purpose: submit request moderation evidence.

Auth:

- active request moderation lease;
- matching moderator actor.

Request body:

```json
{
  "request_moderation_report_id": "rmreport_energy_001",
  "lease_id": "lease_rmoderation_energy_001",
  "claim_token": "<secret>",
  "request_id": "req_energy_001",
  "moderation_kind": "dummy_fixture",
  "decision": "allow_mvp_planning",
  "category_flags": ["none"],
  "safe_reason_codes": ["moderation_allowed"]
}
```

Allowed values:

- `moderation_kind`: `dummy_fixture`, `provider_backed`, `human_curator_fixture`.
- `decision`: `allow_mvp_planning`, `reject_request`, `quarantine_request`.
- `category_flags`: `none`, `sexual`, `sexual_minors`, `violence`, `self_harm`, `hate`, `harassment`, `illicit`, `weapons`, `privacy`, `age_inappropriate`.
- `safe_reason_codes`: `moderation_allowed`, `moderation_rejected_sexual`, `moderation_rejected_sexual_minors`, `moderation_rejected_violence`, `moderation_rejected_self_harm`, `moderation_rejected_hate_or_harassment`, `moderation_rejected_illicit`, `moderation_rejected_weapons`, `moderation_rejected_privacy`, `moderation_rejected_age_inappropriate`, `moderation_quarantine_review_needed`, `moderation_schema_invalid`, `moderation_lineage_mismatch`, `moderation_stale_lease`, `moderation_deterministic_heuristic_override`.

Consistency rules:

- `none` is mutually exclusive with every other category flag.
- `allow_mvp_planning` requires `category_flags=["none"]` and `safe_reason_codes=["moderation_allowed"]`.
- `reject_request` and `quarantine_request` require at least one non-`none` category and matching safe reason code.
- Free-form or provider-derived reason text is rejected.
- If deterministic central inappropriate-content heuristics match the stored request, an `allow_mvp_planning` report is rejected or overridden to quarantine with `moderation_deterministic_heuristic_override`.

Forbidden fields:

- raw provider response;
- provider model name unless a later provenance spec allows a coarse family bucket;
- provider API key, base URL, account ID, quota, or auth path;
- moderation prompt text;
- local paths;
- free-form unsafe content excerpts;
- `recommended_next_state`;
- any actor authority, state, or transition field.

Source states:

- request state is `moderation_pending`;
- task state is `claimed`;
- active lease is held by the authenticated moderation actor.

Side effects:

- accepted `allow_mvp_planning` report moves request to `moderation_passed`, creates one mechanical `PlanningTask`, then moves request to `planning_open`;
- accepted `reject_request` report moves request to `rejected` and creates no planning task;
- accepted `quarantine_request` report moves request to `quarantined` and creates no planning task;
- accepted report consumes the moderation lease;
- replay follows the same idempotency and changed-body conflict rules as other lease-bound submissions;
- stale, cross-lineage, wrong-actor, malformed, contradictory, heuristic-overridden, or unsafe reports are rejected with no planning task creation.

### `GET /v1/requests/{request_id}`

Purpose: read safe request status and metadata.

Auth:

- Required for private requests.
- Public requests may expose only public allowlisted metadata.

Response includes:

- `id`
- `public_status`
- `internal_state` only for authorized actors.
- safe request metadata.
- selected proposal ID only after promotion and only if visible to actor.
- selected artifact ID and public label only after artifact exists and visibility allows.

Response excludes:

- raw rejected payloads;
- unsafe constraints rejected by intake;
- runner/provider details;
- lease tokens;
- private audit data.

### `GET /v1/requests`

Purpose: list visible requests.

Filters:

- `visibility`
- `public_status`
- `subject`
- `language`

No free-text or semantic search.

## Planning Task Endpoints

### `GET /v1/planning-tasks`

Purpose: list planning tasks claimable or visible to the actor.

Auth:

- runner, curator, admin, or system actor.

Filters:

- `state`
- `task_type`
- `subject`
- `language`
- `claimable=true|false`

Runner responses include only safe request summary and task requirements. They do not include raw hidden policy prompts, provider routing, or credentials.

### `POST /v1/planning-tasks/{planning_task_id}/claim`

Purpose: claim one planning fanout slot from spec `006`.

Auth:

- runner actor with planner eligibility.

Request body:

```json
{
  "requested_lease_minutes": 60
}
```

No runner ID in body.

Source states:

- `PlanningTask.open`
- `PlanningTask.claimed` when fanout slot is available.

Side effects:

- verify actor trust/capabilities/status/scope;
- allocate deterministic lease slot;
- create active `Lease`;
- move task `open -> claimed` on first active lease or keep `claimed`;
- move the parent request `planning_open -> planning_in_progress` on the first active planning lease;
- emit safe event.

Response `201`:

```json
{
  "lease": {
    "id": "lease_...",
    "entity_type": "PlanningTask",
    "entity_id": "ptask_energy_001",
    "lease_slot": 0,
    "expires_at": "2026-05-30T01:00:00Z",
    "claim_token": "returned-once"
  }
}
```

`claim_token` is returned only in the claim response body and never again. It is not logged, listed, or returned by read endpoints.

Errors:

- `403 planner_not_eligible`
- `409 planning_claim_slots_full`
- `409 actor_already_holds_active_planning_slot`
- `409 planning_task_completed_or_cancelled`
- `409 request_planning_failed`

### `POST /v1/planning-tasks/{planning_task_id}/heartbeat`

Purpose: record liveness for an active planning lease.

Request body:

```json
{
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once"
}
```

Unknown fields are rejected. Heartbeat does not accept payload data and does not mutate request/proposal/task content.

Response `200`:

```json
{
  "lease_id": "lease_...",
  "state": "active",
  "server_time": "2026-05-30T00:10:00Z"
}
```

### `POST /v1/planning-tasks/{planning_task_id}/release`

Purpose: release an active planning lease.

Request body:

```json
{
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once"
}
```

Side effects:

- lease becomes `released`;
- slot becomes available after release commits;
- planning task returns to `open` if no active lease remains and no valid proposal passed.

Response `200`:

```json
{
  "lease_id": "lease_...",
  "state": "released"
}
```

## Proposal Endpoints

### `POST /v1/planning-tasks/{planning_task_id}/proposals`

Purpose: submit a runner-proposed task graph through an active planning lease.

Auth:

- runner actor holding the lease.

Request body:

```json
{
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once",
  "proposal": {
    "proposal_id": "plan_energy_001_a"
  }
}
```

`proposal` must match the exact schema from spec `007`. The abbreviated shape above is illustrative only.

Source states:

- `PlanningTask.claimed` with active lease.

Side effects:

- derive actor/request/scope from lease;
- validate submitted lineage IDs;
- persist proposal as `proposed`;
- run schema and deterministic policy validation;
- rejected proposal consumes or rejects only the submitting lease and does not close fanout;
- `schema_policy_validated` proposal closes fanout, completes planning task, creates one plan verification task, and revokes other active planning leases;
- emit safe events.

Response cases:

- `201` with proposal state `schema_policy_validated` and verification task summary.
- `422` with proposal state `schema_rejected` or `policy_rejected` and safe validation errors.
- `410` for stale, expired, released, consumed, revoked, or replaced lease.

Response excludes claim tokens and raw rejected values.

### `GET /v1/proposals/{proposal_id}`

Purpose: read safe proposal status and validation result.

Auth:

- actor in same scope with visibility to request/proposal.

Response includes:

- proposal ID;
- request ID;
- planning task ID;
- central state;
- central risk level if validation passed;
- safe validation errors or warnings;
- verification task ID if created;
- promotion decision ID if promoted or rejected at promotion.

Response excludes raw forbidden fields and runner-local provider data.

### `POST /v1/proposals/{proposal_id}/promote`

Purpose: promote a verified proposal into work packets.

Auth:

- system_core, curator, admin, or authorized deterministic service actor.
- MVP may expose this as a local test/admin command endpoint; public runners cannot call it directly.

Request body:

```json
{
  "promotion_reason": "mvp_low_risk_verified"
}
```

Source states:

- `ProposedTaskGraph.verified_for_mvp_promotion`.

Guards:

- all promotion rules from spec `007`.

Side effects:

- create `PromotionDecision`;
- create work packets transactionally;
- move proposal to `promoted`;
- move request to `decomposed`;
- emit safe events.

Response `201`:

```json
{
  "promotion_decision": {
    "id": "promo_...",
    "state": "accepted",
    "proposal_id": "plan_energy_001_a",
    "created_work_packet_ids": [
      "wp_energy_001_generate_pack",
      "wp_energy_001_validate_pack",
      "wp_energy_001_human_review"
    ]
  }
}
```

Replay returns the original decision without duplicate work packets.

## Plan Verification Endpoints

### `GET /v1/plan-verification-tasks`

Purpose: list verification tasks claimable by verifier runners.

Auth:

- verifier runner candidate, curator, admin, or system actor.

Filters:

- `state`
- `claimable`
- `proposal_id`

### `POST /v1/plan-verification-tasks/{task_id}/claim`

Purpose: claim a plan verification task.

Auth:

- runner actor with verifier eligibility.

Source states:

- `PlanVerificationTask.open`.

Guards:

- actor differs from proposal planner actor;
- same scope;
- no active lease already exists.

Response follows lease response pattern but with no `lease_slot` unless a later spec allows fanout.

### `POST /v1/plan-verification-tasks/{task_id}/heartbeat`

Same heartbeat contract as planning tasks, single-lease surface.

### `POST /v1/plan-verification-tasks/{task_id}/release`

Same release contract as planning tasks, single-lease surface.

### `POST /v1/plan-verification-tasks/{task_id}/verifications`

Purpose: submit structured plan verification evidence.

Auth:

- verifier runner holding active verification lease.

Request body:

```json
{
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once",
  "verification": {
    "verification_id": "pverify_energy_001_a"
  }
}
```

`verification` must match the plan verification schema from specs `001` and `005`.

Side effects:

- derive verifier actor/proposal/scope from lease;
- reject self-verification;
- persist verification as accepted evidence, blocking, or rejected;
- move proposal to `verified_for_mvp_promotion` only on accepted independent no-blocking evidence;
- move proposal to `verification_blocked` on accepted blocking evidence;
- consume lease;
- emit safe events.

Plan verification never creates review records and never raises artifact/public state.

## Work Packet Endpoints

### `GET /v1/work-packets`

Purpose: list claimable or visible work packets.

Auth:

- runner, human reviewer, curator, admin, or system actor.

Filters:

- `state`
- `phase`
- `task_type`
- `request_id`
- `proposal_id`
- `claimable`

Runner-visible fields:

- work packet ID;
- phase;
- task type;
- execution policy fields from spec `013`;
- safe request summary;
- required capabilities;
- outputs;
- validation required;
- human review required;
- dependency status.

Runner responses exclude provider routing, prompts, credentials, hidden central policy text, claim tokens, and private review notes.

### `POST /v1/work-packets/{work_packet_id}/claim`

Purpose: claim runner-generation work packets.

Auth:

- runner for generation work.

Guards:

- actor type, trust, capability, scope, and conflict rules for the work packet type;
- dependencies satisfied;
- no active lease.

Non-claimable work-packet phases:

- `phase=mechanical_validation` is system-validator provenance and dependency tracking, not a runner claim surface.
- `phase=human_review` is the parent surface for `ReviewTask`, not directly claimable by human reviewers.
- Attempts to claim validation or human-review work packets through this endpoint return `409 work_packet_not_claimable_here`.

Response follows single-lease claim pattern.

### `POST /v1/work-packets/{work_packet_id}/heartbeat`

Same heartbeat contract as planning tasks, single-lease surface.

### `POST /v1/work-packets/{work_packet_id}/release`

Same release contract as planning tasks, single-lease surface.

### `POST /v1/work-packets/{work_packet_id}/submit`

Purpose: submit work output metadata.

Auth:

- active lease holder.

Request body for generation MVP:

```json
{
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once",
  "output": {
    "kind": "generation_output_v1",
    "artifact_bundle_reference": {
      "kind": "artifact_bundle_reference",
      "artifact_intake_ref": "aintake_energy_001",
      "manifest_summary": {
        "artifact_id": "art_energy_001",
        "title": "Conservation of energy worksheet and checker",
        "subject": "physics",
        "topic": "conservation_of_energy",
        "age_range": "14-16",
        "language": "en",
        "license": "CC-BY-4.0",
        "ai_assisted": true,
        "contents": [
          "worksheet.md",
          "answer_key.md",
          "checker.py",
          "teacher_notes.md"
        ],
        "known_limitations": [
          "Not yet reviewed by a qualified physics teacher."
        ]
      }
    },
    "provenance": {
      "file_digests": {
        "manifest.json": "sha256:<fixture>",
        "worksheet.md": "sha256:<fixture>",
        "answer_key.md": "sha256:<fixture>",
        "checker.py": "sha256:<fixture>",
        "teacher_notes.md": "sha256:<fixture>"
      },
      "bundle_digest": "sha256:<fixture>",
      "runner_self_test_report": {
        "self_test_report_id": "rselftest_energy_001",
        "work_packet_id": "wp_energy_001_generate_pack",
        "lease_id": "lease_generate_energy_001",
        "runner_actor_id": "actor_generator_001",
        "execution_policy": "sandboxed_self_test_python_checker",
        "execution_profile_id": "python_checker_self_test_v1",
        "allowed_command_id": "python_checker_self_test_harness_v1",
        "sandbox_enforced": true,
        "self_test_status": "passed",
        "file_digests": {
          "manifest.json": "sha256:<fixture>",
          "worksheet.md": "sha256:<fixture>",
          "answer_key.md": "sha256:<fixture>",
          "checker.py": "sha256:<fixture>",
          "teacher_notes.md": "sha256:<fixture>"
        },
        "bundle_digest": "sha256:<fixture>",
        "checks": [
          {"check": "sandbox_profile_enforced", "status": "passed"},
          {"check": "checker_static_safety", "status": "passed"},
          {"check": "checker_function_contract", "status": "passed"},
          {"check": "checker_sample_cases", "status": "passed"},
          {"check": "no_network_observed", "status": "passed"},
          {"check": "no_filesystem_escape_observed", "status": "passed"},
          {"check": "no_secret_env_present", "status": "passed"},
          {"check": "digest_computed", "status": "passed"}
        ],
        "attestation": {
          "attestation_schema_version": "runner-self-test-attestation-v1",
          "signature_kind": "ed25519",
          "runner_key_id": "rkey_dummy_generator_001",
          "signed_payload_digest": "sha256:<fixture>",
          "signature": "<base64url-ed25519-signature>"
        },
        "created_at": "2026-01-01T00:00:00Z"
      }
    }
  }
}
```

Rules:

- No raw bundle bytes in this spec.
- No file paths, archive paths, user filenames, or URL-like references.
- No URLs.
- No provider/model/prompt fields.
- Submitted artifact IDs are claims checked against server-derived work packet lineage.
- `output` is a closed wrapper object with exactly `kind=generation_output_v1`, `artifact_bundle_reference`, and `provenance`.
- `artifact_bundle_reference` is the closed `ArtifactBundleReference` shape below; provenance fields are not embedded in that shape.
- `provenance` is a closed object with exactly `file_digests`, `bundle_digest`, and `runner_self_test_report`.
- `provenance.file_digests`, `provenance.bundle_digest`, and `provenance.runner_self_test_report` follow spec `013`.
- For generation work packets with `execution_policy=sandboxed_self_test_python_checker`, `provenance.runner_self_test_report` is required even when the sandbox was unavailable or local runner config disabled self-test. Those cases submit the full signed report with a spec `013` `not_run_*` status.
- `provenance.file_digests` must exactly equal `provenance.runner_self_test_report.file_digests`.
- `provenance.bundle_digest` must exactly equal `provenance.runner_self_test_report.bundle_digest`.
- Runner self-test metadata is internal provenance only and cannot create `ValidationReport` or move artifact to `machine_validated`.
- Central derives active `work_packet_id`, `lease_id`, authenticated `runner_actor_id`, execution policy/profile/command, and idempotency context from the lease and work packet. Any submitted self-test field that conflicts with those derived values is rejected before persistence.
- `runner_self_test_report` must be the full closed spec `013` object. Partial reports are rejected.

Side effects:

- generation work may create `Artifact` in `draft_generated` after `output`, nested `ArtifactBundleReference`, and spec `013` provenance pass their closed shapes and spec `004` field allowlists;
- consume lease on successful accepted submission;
- emit safe events.

Validation work and human-review work cannot be submitted through this endpoint.

## Code Critique and Repair Work Endpoints

Spec `014` extends generic runner work packets with `critique_generated_code` and `repair_generated_code`.

Claim, heartbeat, and release:

- use the generic `/v1/work-packets/{work_packet_id}/claim`, heartbeat, and release surfaces;
- require runner actor type, scope, capability summary, active signing key, and matching task type;
- reject validation and human-review work as before.

### Code critique submit

Request body:

```json
{
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once",
  "output": {
    "kind": "code_critique_output_v1",
    "code_critique_report": "<full CodeCritiqueReport object from spec 014>"
  }
}
```

Rules:

- `output` is a closed object with exactly `kind=code_critique_output_v1` and `code_critique_report`.
- The report follows the full closed spec `014` schema.
- Central derives work packet, lease, runner actor, source artifact, source bundle digest, execution policy/profile/command, and idempotency context from server state.
- Any submitted lineage field that conflicts with derived context is rejected.
- Accepted critique evidence is advisory only.

### Code repair submit

Request body:

```json
{
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once",
  "output": {
    "kind": "code_repair_output_v1",
    "artifact_bundle_reference": "<ArtifactBundleReference object>",
    "provenance": {
      "source_file_digests": "<closed digest map from spec 014>",
      "repaired_file_digests": "<closed digest map from spec 014>",
      "repaired_bundle_digest": "sha256:<fixture>",
      "code_repair_report": "<full CodeRepairReport object from spec 014>"
    }
  }
}
```

Rules:

- `output` is a closed object with exactly `kind=code_repair_output_v1`, `artifact_bundle_reference`, and `provenance`.
- `artifact_bundle_reference` uses the closed shape below and describes the repaired draft artifact candidate.
- `provenance` is a closed object with exactly `source_file_digests`, `repaired_file_digests`, `repaired_bundle_digest`, and `code_repair_report`.
- Provenance fields must exactly match the same fields inside `code_repair_report`.
- `artifact_bundle_reference.artifact_intake_ref` and `artifact_bundle_reference.manifest_summary.artifact_id` must exactly match the `target_artifact_intake_ref` and `target_artifact_id` preallocated in the repair work packet.
- Central rejects any repair output that changes files outside the spec `014` allowed `changed_files` set.
- Accepted repair output creates a new `draft_generated` artifact and validation work packet only; it cannot create trusted validation or human-review approval.

### Code repair interruption submit

Request body:

```json
{
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once",
  "output": {
    "kind": "code_repair_interruption_output_v1",
    "code_repair_interruption_report": "<full CodeRepairInterruptionReport object from spec 014>"
  }
}
```

Rules:

- `output` is a closed object with exactly `kind=code_repair_interruption_output_v1` and `code_repair_interruption_report`.
- The report follows the full closed spec `014` schema.
- Central derives work packet, lease, runner actor, source artifact, source bundle digest, repair attempt index, execution policy, and idempotency context from server state.
- Any submitted lineage field that conflicts with derived context is rejected.
- Accepted interruption consumes the lease, moves the repair work packet to `interrupted`, creates no artifact, and routes only through the bounded spec `014` runner-operator/curator/admin continuation rules.

### `POST /v1/repair-interruptions/{interruption_report_id}/decision`

Purpose: decide whether an interrupted automated repair attempt may continue on another runner or must stop/escalate.

Auth:

- `runner_operator` for an owned interrupted runner in the same scope;
- `curator`;
- `admin`.

Request body:

```json
{
  "decision": {
    "repair_attempt_id": "rattempt_energy_001_1",
    "source_artifact_id": "art_energy_001",
    "repair_attempt_index": 1,
    "interrupted_repair_continuation_index": 0,
    "next_repair_continuation_index": 1,
    "outcome": "continue_on_another_runner",
    "safe_reason_code": "quota_exhausted_continue_elsewhere"
  }
}
```

Rules:

- Request body is closed.
- Request requires the global `Idempotency-Key` header. The key is scoped by endpoint, actor, scope, `interruption_report_id`, and server-derived repair attempt lineage.
- Actor ID and scope are derived from authentication.
- `interruption_report_id`, `repair_attempt_id`, source artifact, attempt index, and interrupted continuation index must match server state.
- `next_repair_continuation_index` must equal interrupted continuation index plus one when outcome is `continue_on_another_runner`.
- Idempotent replay with the same header key and identical body returns the original decision; changed replay under the same header key is rejected with `409 idempotency_conflict`.
- `continue_on_another_runner` creates exactly one new repair work packet with the interrupted runner actor excluded.
- Stop/quarantine/triage outcomes create no repair work packet.
- No outcome creates an artifact, validation report, review record, or public label.

## Artifact Bundle Reference

`ArtifactBundleReference` is the closed MVP artifact-reference shape nested inside generation output:

```json
{
  "kind": "artifact_bundle_reference",
  "artifact_intake_ref": "aintake_energy_001",
  "manifest_summary": {
    "artifact_id": "art_energy_001",
    "title": "Conservation of energy worksheet and checker",
    "subject": "physics",
    "topic": "conservation_of_energy",
    "age_range": "14-16",
    "language": "en",
    "license": "CC-BY-4.0",
    "ai_assisted": true,
    "contents": [
      "worksheet.md",
      "answer_key.md",
      "checker.py",
      "teacher_notes.md"
    ],
    "known_limitations": [
      "Not yet reviewed by a qualified physics teacher."
    ]
  }
}
```

Rules:

- Unknown fields are rejected.
- `artifact_intake_ref` must match opaque ID pattern `aintake_[a-z0-9_]+`.
- `artifact_intake_ref` is not a path, URL, filename, archive name, or user display name.
- `manifest_summary` fields are the safe subset from spec `001` and spec `004`.
- `contents` must exactly match the MVP bundle filenames.
- `known_limitations` is bounded inert text.
- `status_claim` is not accepted in API output submission; if present in runner bundle manifest, it is handled later by spec `010` and remains non-authoritative.
- Successful submission creates or links one draft artifact for the generation work packet.

## Validation Work Endpoints

Validation work packets use a dedicated system-validator API path. They are not claimable through generic work-packet endpoints.

### `POST /v1/validation-work-packets/{work_packet_id}/claim`

Purpose: claim a mechanical validation work packet for the configured deterministic validator.

Auth:

- `system_validator` actor only.

Source states:

- `WorkPacket.open` with `phase=mechanical_validation`.

Guards:

- work packet belongs to the selected promoted proposal;
- generation dependency is accepted;
- selected artifact exists in `draft_generated`;
- actor scope matches artifact/request/proposal scope;
- no active validation lease exists;
- system validator status is active.

Side effects:

- create active validation lease;
- associate lease with validation work packet and artifact lineage;
- emit safe event.

Response follows single-lease claim pattern:

```json
{
  "lease": {
    "id": "lease_...",
    "entity_type": "WorkPacket",
    "entity_id": "wp_energy_001_validate_pack",
    "expires_at": "2026-05-30T01:00:00Z",
    "claim_token": "returned-once"
  },
  "artifact": {
    "id": "art_energy_001",
    "state": "draft_generated"
  }
}
```

Claim replay returns lease metadata without `claim_token`.

### `POST /v1/validation-work-packets/{work_packet_id}/heartbeat`

Same heartbeat contract as other single-lease surfaces. Requires `system_validator` actor, lease ID, and claim token.

### `POST /v1/validation-work-packets/{work_packet_id}/release`

Same release contract as other single-lease surfaces. Requires `system_validator` actor, lease ID, and claim token.

## Artifact and Validation Endpoints

### `GET /v1/artifacts/{artifact_id}`

Purpose: read safe artifact metadata and central status.

Auth:

- actor with request/artifact visibility.

Response includes:

- artifact ID;
- request ID;
- proposal ID;
- work packet IDs;
- central state;
- validation status;
- review status;
- public label;
- safe manifest summary.

Response excludes raw files, executable content, hidden validator logs, rejected payloads, local paths, provider data, and claim tokens.

### `POST /v1/artifacts/{artifact_id}/validation-reports`

Purpose: record trusted deterministic validation result.

Auth:

- `system_validator` actor only.

Request body:

```json
{
  "validation_work_packet_id": "wp_energy_001_validate_pack",
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once",
  "validator_run_id": "validator_run_...",
  "report": {
    "validation_report_id": "vreport_energy_001"
  }
}
```

`report` must match spec `010`; until spec `010` passes, this endpoint is limited to the `001` fixture shape for state tests.

Guards:

- validator actor is trusted system validator;
- validation work packet exists, has `phase=mechanical_validation`, and depends on the artifact-generating work packet;
- validation work packet is server-opened for the system validator after the artifact reaches `draft_generated`;
- active validation lease matches system validator actor, artifact, request, proposal, scope, and validation work packet;
- artifact lineage matches server-derived validation work-packet context;
- report is schema-valid;
- no raw unsafe stdout/stderr persisted.

Side effects:

- create `ValidationReport`;
- move artifact to `machine_validated` on trusted pass;
- move artifact to `validation_failed` on trusted failure;
- mark validation work packet accepted/completed on trusted report acceptance;
- consume validation lease on success;
- open review task/work-packet dependency only after machine validation;
- emit safe events.

Runner-submitted validation reports through any other endpoint are `untrusted_evidence` or rejected and cannot move artifact state.

Trusted validation report idempotency:

- Unique key: `(artifact_id, validation_work_packet_id, validator_run_id)`.
- Replay with identical non-secret canonical report returns the existing validation report.
- Changed replay under the same idempotency key or validator run ID is rejected.
- No duplicate validation report, artifact transition, work-packet completion, or review task is created.

## Review Endpoints

### `GET /v1/review-tasks`

Purpose: list claimable human review tasks.

Auth:

- human reviewer, curator, admin.

Filters:

- `state`
- `review_type`
- `claimable`
- `artifact_id`

Review task creation:

- `system_core` creates `ReviewTask` after the selected artifact bundle reaches `machine_validated`.
- The parent human-review `WorkPacket` is opened/accepted as dependency provenance by `system_core`.
- Human reviewers never claim the human-review `WorkPacket` directly.
- Reviewers claim only `ReviewTask`.

### `POST /v1/review-tasks/{review_task_id}/claim`

Purpose: claim a human review task.

Auth:

- human reviewer.

Guards:

- reviewer trust and role eligibility;
- same scope;
- no disqualifying conflict from specs `005` and `011`;
- no active lease.

Response follows single-lease claim pattern.

### `POST /v1/review-tasks/{review_task_id}/heartbeat`

Same heartbeat contract as planning tasks, single-lease surface.

### `POST /v1/review-tasks/{review_task_id}/release`

Same release contract as planning tasks, single-lease surface.

### `POST /v1/review-tasks/{review_task_id}/reviews`

Purpose: submit human review record.

Auth:

- active human reviewer lease holder.

Request body:

```json
{
  "lease_id": "lease_...",
  "claim_token": "secret-token-returned-once",
  "review": {
    "review_id": "review_energy_001",
    "artifact_id": "art_energy_001",
    "outcome": "approved_for_peer_reviewed",
    "findings": [],
    "recommended_next_state": "peer_reviewed"
  }
}
```

Rules:

- `reviewer_id` in body is optional display/evidence only and must match auth context if present.
- `outcome` and `recommended_next_state` do not directly set artifact state.
- Review record acceptance means valid evidence, not approval by itself.

Side effects:

- derive reviewer/artifact/task/scope from lease;
- reject conflict laundering;
- persist review as accepted or rejected evidence;
- artifact moves to `peer_reviewed` only when accepted review has approval outcome, required review gates pass, and no open blocking findings remain;
- consume lease;
- emit safe events.

## Public Artifact Endpoints

### `GET /v1/public/artifacts/{artifact_id}`

Purpose: read public artifact metadata after visibility and state allow publication.

Public response allowlist:

- artifact ID;
- title;
- subject;
- topic;
- age range;
- language;
- license;
- public label;
- AI assistance disclosure;
- known limitations;
- review state summary;
- validation state summary;
- approved safe provenance summary.

Response excludes:

- draft or private artifacts;
- raw generated files until spec `010` publication rules pass;
- runner IDs unless safe public provenance allowlist permits them;
- provider/model/local config;
- reviewer private notes;
- internal audit details.

If artifact is not public or not visible, return `404` rather than leaking existence.

## Event Endpoints

### `GET /v1/events`

Purpose: operator/debug read of safe state transition events.

Auth:

- curator, admin, or system actor.

Filters:

- `entity_type`
- `entity_id`
- `request_id`
- `actor_type`
- `action`
- `created_after`
- `created_before`

Events follow spec `005` and `004` allowlists only. No raw rejected payloads, claim tokens, local paths, provider data, prompt text, stack traces, or student PII.

### `GET /v1/events/stream`

Purpose: read-only real-time stream of safe state transition events.

Auth:

- curator, admin, system actor, or test harness actor.

Transport:

- Server-Sent Events.
- HTTPS required outside loopback.
- Supports `Last-Event-Id` or cursor query parameter.
- Does not accept JSON command messages.
- Does not accept mutation payloads.

Each event is the same safe projection as `GET /v1/events`.

The stream must close on auth expiry, actor revocation, scope change, idle timeout, or server shutdown. Reconnection resumes from committed event cursor and never replays mutations.

## Health Endpoint

### `GET /v1/health`

Purpose: deterministic service health.

Response:

```json
{
  "status": "ok",
  "service": "lessonforge_api",
  "schema_version": "1.0"
}
```

Health response must not include environment variables, database URLs, provider keys, loaded model names, host paths, process command lines, or dependency graph details.

## Endpoint Field Allowlists

Every endpoint must be backed by a typed request and typed response. Map types are allowed only where the value schema is a closed enum or schema-defined object.

Endpoint request allowlists:

| Endpoint group | Allowed submitted authority fields |
|---|---|
| Request intake | request fields from spec `006`; no actor authority fields |
| Request moderation report | moderation lease credentials plus `RequestModerationReport` safe schema; no provider raw response or actor authority fields |
| Claim/heartbeat/release | lease ID, claim token, requested lease duration where applicable; claim replay excludes token |
| Proposal submit | lease credentials plus `ProposedTaskGraph` schema from spec `007` |
| Verification submit | lease credentials plus `PlanVerification` schema |
| Promotion | safe reason enum only |
| Work output submit | lease credentials plus closed output metadata schema, digest metadata, required spec `013` self-test report evidence for `sandboxed_self_test_python_checker` generation work, and spec `014` code critique/repair/interruption reports for their task types |
| Repair interruption decision | `Idempotency-Key` header plus closed spec `014` continuation decision body; actor and scope derive from auth |
| Validation work claim/heartbeat/release | system-validator actor context, validation work packet ID, lease ID/token for heartbeat/release |
| Validation report | validation lease credentials from dedicated validation-work claim, validator run ID, plus validation report schema |
| Review submit | lease credentials plus review schema |

Any unknown submitted field is rejected before business logic.

## Race, Replay, and Stale Submit Rules

All mutating endpoints execute as transactions over:

- current entity state;
- actor authority;
- lease state where applicable;
- idempotency record;
- durable side effects;
- transition events.

Required race behavior:

- Concurrent claims cannot exceed configured active lease slots.
- Concurrent submit on the same lease results in one accepted submission and replay/conflict for the rest.
- Lease expiry racing with submit is resolved by server transaction time; either submit consumes active lease before expiry commit, or expiry wins and submit returns `410`.
- Promotion racing with promotion returns one accepted decision and idempotent/no-op result for the rest.
- Review approval racing with blocking finding resolution uses the state visible in the transaction; peer review requires no open blocking finding at commit.
- Public reads use committed state only.

Stale submit behavior:

- expired, released, consumed, revoked, or replaced lease returns `410 lease_not_submitting`;
- wrong actor or scope returns `403`;
- wrong entity lineage returns `409 lineage_mismatch`;
- completed/cancelled parent task returns `409 invalid_source_state`;
- no duplicate records or events are created.

## No-Inference and No-Leak API Guardrails

The API implementation must be testable for:

- no model-provider SDK imports in central API crates;
- no provider credential environment variable reads;
- no endpoint fields for provider credential/config submission;
- no prompt execution endpoints;
- no central model moderation endpoints or provider moderation calls;
- no central embeddings/vector search;
- no outbound model-provider calls;
- no URL fetches from request/proposal/artifact fields;
- no raw rejected values in errors, events, logs, or public responses;
- no lease token logging or re-display after claim response.

## Acceptance Tests

### API-001: Valid request creates request moderation task

Call `POST /v1/requests` with the spec `001` request.

Expected:

- `201`.
- Response includes public status `requested`.
- Response includes internal state `moderation_pending` for authorized test actor.
- Exactly one request moderation task is created.
- No planning task is created before moderation passes.
- No work packets or artifacts are created.

### API-001A: Accepted request moderation creates planning task

Claim request moderation task and submit `decision=allow_mvp_planning` through the active lease.

Expected:

- Moderation report is accepted.
- Request advances to `planning_open`.
- Exactly one planning task is created.
- Replay returns existing result without duplicate moderation report, planning task, or events.

### API-001A.1: Planning retry exhaustion fails request without materialization

Expire or release all active planning leases without an accepted proposal until `max_claim_attempts=3` is exceeded.

Expected:

- Planning task becomes `cancelled`.
- Request advances to terminal internal state `planning_failed`.
- Safe event reason code is `planning_abandoned_retry_limit`.
- No proposed graph, selected plan, work packet, artifact, or review task is created.
- Later planning claims and submissions are rejected replay-stably with safe conflict codes.

### API-001B: Rejected or forged moderation blocks planning

Submit moderation report with `reject_request`, `quarantine_request`, stale lease, wrong request ID, wrong actor, contradictory categories, raw provider response, provider credential, local path, or unknown field. Also submit `allow_mvp_planning` with `category_flags=["none"]` for request text that matches deterministic inappropriate-content heuristics.

Expected:

- Deny/quarantine decisions create no planning task.
- Forged, stale, cross-lineage, contradictory, heuristic-overridden, or unsafe moderation reports are rejected.
- Raw provider response and unsafe values do not appear in error/log/event.

### API-002: Request endpoint rejects unknown and forbidden fields

Submit request with attachment URL, prompt field, provider key, model field, and unknown field.

Expected:

- `400` or `422` safe error.
- No raw forbidden value in error/log/event.
- No request moderation task or planning task is created.

### API-003: Planning claim allocates bounded slot and returns token once

Claim a planning task with two distinct eligible runners, then try a third and a duplicate actor claim.

Expected:

- First two claims succeed with slots `0` and `1`.
- Third claim returns `409 planning_claim_slots_full`.
- Duplicate actor returns `409 actor_already_holds_active_planning_slot`.
- Claim token is returned only in claim response and not in later reads/events/logs.
- Idempotent replay of a successful claim returns lease metadata without `claim_token`.

### API-004: Heartbeat rejects payload mutation

Call heartbeat with valid lease and extra proposal/request fields.

Expected:

- Unknown fields rejected safely.
- No request, planning task, proposal, or lease authority data changes except allowed heartbeat liveness on valid heartbeat.

### API-005: Proposal submit validates and creates verification task

Submit valid proposal through active planning lease.

Expected:

- Proposal reaches `schema_policy_validated`.
- Planning fanout closes.
- Exactly one plan verification task is created.
- Replay returns existing result without duplicate verification task or events.

### API-006: Rejected proposal does not close fanout

Submit malformed or policy-rejected proposal through one active planning lease while another eligible claim remains.

Expected:

- Proposal is `schema_rejected` or `policy_rejected`.
- Planning task does not become `completed`.
- Other active lease is not revoked for success.
- No verification task or work packets are created.

### API-007: Verification submit gates promotion

Try promotion before verification, then submit independent no-blocking verification and retry.

Expected:

- Pre-verification promotion fails.
- Verification moves proposal to `verified_for_mvp_promotion`.
- Promotion succeeds only after verification.

### API-008: Promotion is transactional and idempotent

Promote verified proposal twice and simulate duplicate concurrent promotion.

Expected:

- Exactly three work packets exist.
- Replays return original decision or no-op.
- No duplicate `(proposal_id, source_local_task_id)`.

### API-009: Work-packet stale submit is rejected

Claim work packet, expire/release/revoke/consume lease, then submit.

Expected:

- `410 lease_not_submitting`.
- No output, artifact, review, or validation state changes.

### API-010: Runner validation cannot machine validate

Submit `validation_report.json` through runner work output or unauthorized endpoint claiming pass.

Expected:

- Report is rejected or stored only as untrusted evidence.
- Artifact remains `draft_generated`.
- Trusted validation endpoint requires `system_validator`.
- Trusted validation endpoint also requires active validation work-packet lease and server-derived artifact lineage.

### API-010A: Validation work has one trusted path

Try to claim `phase=mechanical_validation` through the generic work-packet claim endpoint, claim it through the dedicated validation-work endpoint, then submit trusted validation without and with that validation lease.

Expected:

- Generic claim returns `409 work_packet_not_claimable_here`.
- Dedicated validation-work claim succeeds only for `system_validator` and returns the claim token once.
- Validation report without active validation lease is rejected.
- Valid system-validator submission consumes the validation lease, creates one trusted report, updates artifact state, and marks validation work packet accepted/completed.
- Replay creates no duplicate report, transition, or review task.

### API-011: Human review cannot be forged

Submit review with forged `reviewer_id`, wrong actor, stale lease, or conflict actor.

Expected:

- Review is rejected.
- Artifact does not become `peer_reviewed`.
- Safe event records reason code.

### API-011A: Human review has one claim path

Try to claim `phase=human_review` through generic work-packet claim, then claim the derived review task.

Expected:

- Generic work-packet claim returns `409 work_packet_not_claimable_here`.
- Review task claim is the only human-review lease path.
- Review submission derives artifact/reviewer/scope from review-task lease.

### API-012: Public artifact endpoint does not leak drafts or private artifacts

Read public artifact before public visibility/state allows it.

Expected:

- `404`.
- No existence leak, internal state, reviewer notes, runner details, provider data, or local paths.

### API-013: Error and event no-leak

Send fake secrets, prompt text, local paths, URLs, provider config, and student PII through every mutating endpoint.

Expected:

- Unsafe values are rejected or redacted according to specs `002` and `004`.
- API errors, logs, events, and public responses contain only safe codes/classifications.

### API-014: No-inference API surface

Inspect API routes, schemas, dependency graph, environment reads, and outbound network mocks.

Expected:

- No provider SDK, provider credential env read, model route, prompt route, embedding/vector table, or outbound model-provider call exists in central API.

### API-015: Trusted actor headers cannot be spoofed

Send client-controlled reverse-proxy actor headers over direct client connection and with body-supplied actor IDs.

Expected:

- Spoofed actor headers are stripped or rejected.
- Body-supplied actor IDs are non-authoritative and cannot change actor context.
- Test actor injection fails startup outside loopback/test profile.

### API-016: Read, poll, public, and SSE availability limits apply

Flood list endpoints, runner polling endpoints, cursor replay, public artifact `404` lookups, and SSE stream connections.

Expected:

- Per-actor/per-IP read limits apply.
- SSE connection caps, idle timeouts, max duration, and backpressure limits apply.
- Cursor replay is throttled.
- Public enumeration receives safe `429` without existence leakage.

## Security Checklist Coverage

Applicable categories from `002`:

- Central-Core Boundary Checks: API exposes deterministic workflow commands only.
- Data Classification Checks: endpoint fields, errors, events, and public responses follow spec `004`.
- Request Intake and Prompt-Injection Checks: request endpoint accepts structured request only and prompt-like text is inert.
- SSRF/Outbound URL Checks: no URL fetch/import endpoint exists.
- Identity, Authorization, and Replay Checks: auth context, leases, idempotency, stale submit, and race behavior are specified.
- Runner Submission Trust Checks: runner payloads remain claims until deterministic validation and state guards pass.
- Plan Verification Gate Checks: verification endpoints are separate from human review and gate promotion.
- Artifact and Generated-Code Checks: artifact/validation endpoints do not execute or publish raw bundles in this spec.
- Human Review and Publication Checks: review authority and public allowlists are specified for MVP.
- Logging, Error, and Audit Checks: redacted error/event contract is specified.

No applicable non-deferrable checklist item is deferred for central API behavior through the MVP surfaces.

## Review Checklist

Reviewers should fail this spec if:

- Any endpoint accepts provider credentials, provider config, model names, executable prompts, local auth paths, or provider base URLs.
- Actor identity can be supplied by request body instead of auth context and server-derived lineage.
- Reverse-proxy actor headers or test actor injection can be enabled on untrusted/non-loopback deployments.
- Lease tokens appear in logs, events, list responses, read responses, or replay responses.
- Idempotency fingerprints persist raw or hashed secret values.
- Mutating endpoints lack idempotency, source-state guards, side effects, or stale-submit behavior.
- Request/proposal/artifact URLs can trigger central fetches.
- Runner-submitted validation can raise artifact state.
- Validation or human review can be claimed through two independent API paths.
- Plan verification can count as human review.
- Human review can be forged by submitted reviewer IDs.
- Public endpoints leak draft/private existence or unsafe provenance.
- Read/list/poll/SSE/public endpoints lack availability limits.
- Error responses can contain raw rejected values or stack traces.
- The central API exposes semantic search, prompt execution, embeddings, model-provider calls, or semantic decomposition.
