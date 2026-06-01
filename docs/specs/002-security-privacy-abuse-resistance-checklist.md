# 002. Security, Privacy, and Abuse-Resistance Checklist

Status: Passed adversarial review  
Roadmap: `docs/specs/000-spec-roadmap.md`  
Depends on: `docs/specs/001-mvp-slice-acceptance-tests.md`

## Purpose

This checklist gates specs `003` through `011`. It defines cross-cutting security, privacy, abuse-resistance, and provider-boundary checks that every later implementation spec must satisfy before it can be considered implementation-ready.

This document is not a complete security architecture. It is a review instrument. Later specs must either satisfy each applicable check or explicitly defer it with a reason that does not weaken the MVP slice.

## Review Rule

For every later spec, reviewers must classify each checklist item as:

- `satisfied`: the spec contains concrete requirements or tests that satisfy the item.
- `not_applicable`: the item does not apply to the spec surface, with a one-sentence reason.
- `deferred`: the item is intentionally deferred outside the current implementation slice, with a linked follow-up spec or milestone.
- `failed`: the spec omits or contradicts the item.

A spec fails review if any applicable item is `failed`.

Some checks are non-deferrable for specs that touch their surface. Central-core boundary checks, secret/forbidden persistence checks, prompt execution/storage bans, provider credential leakage checks, identity/authorization checks, and runner-controlled status-escalation checks must be `satisfied` or `not_applicable`; they must not be `deferred` for any spec that defines an endpoint, table, log, persisted object, runner submission, validation path, promotion path, public response, artifact surface, review surface, or publication surface.

## Central-Core Boundary Checks

Later specs must preserve these central-core boundaries:

- The central core must not import model-provider SDKs.
- The central core must not read provider credential environment variables.
- The central core must not call model-provider domains.
- The central core must not run embeddings, vector search, model moderation, model scoring, model verification, or semantic deduplication in the MVP.
- The central core must not store executable prompt templates, raw model prompts intended for execution, provider-specific prompt fragments, or runner-local prompt files.
- The central core must not perform request-to-task semantic decomposition.
- The central core must not semantically merge conflicting plans.
- The central core must not select a volunteer's exact model/provider.
- Any future project-funded inference service must be specified as a separate non-core runner or service with explicit boundaries.

Required adversarial tests:

- A static dependency check fails if `crates/lessonforge_api`, the central-core package, the central API dependency graph, or any shared package imported by the central core includes OpenAI, Anthropic, Gemini, Ollama, LangChain, LlamaIndex, model-provider SDKs, inference frameworks, embedding/vector clients, provider adapters, or equivalent inference libraries.
- A configuration check fails if `crates/lessonforge_api` requires provider credential environment variables.
- A schema or migration check fails if MVP central-core tables include embedding/vector fields.
- An endpoint or integration test fails if core request intake creates semantic work packets without a runner proposal.

## Data Classification Checks

Later specs must classify fields and records as one of:

| Class | Meaning | Handling |
|---|---|---|
| Public | Safe for public artifact pages and downloads. | May be returned publicly. |
| Internal | Needed for central workflow operation. | Persist internally; do not publish by default. |
| Private | User/runner/reviewer operational detail. | Persist only when required and redacted from public output. |
| Secret | Credential or credential-equivalent value. | Reject or redact before central persistence. |
| Forbidden | Data the platform must not accept for the MVP. | Reject, quarantine, or strip according to policy. |

Secret and forbidden values include:

- API keys.
- OAuth tokens.
- Browser cookies.
- Codex `auth.json` contents or paths.
- Passwords.
- Provider account identifiers that can identify a private account.
- Provider base URLs unless a later reviewed spec creates an explicit safe disclosure field.
- Local filesystem paths outside safe workspace metadata.
- Local prompt templates.
- Exact private quota/account details.
- Student personally identifying information.
- Named student records or grading data.

Required adversarial tests:

- Fake secrets submitted through runner capability summaries, proposed task graphs, artifact manifests, validation reports, critique reports, review notes, and malformed payloads are rejected or redacted before any central persistence, log, error response, or public output.
- Public artifact metadata contains only approved public fields.
- Error responses contain structured codes and safe field names, not raw rejected secret values.

## Request Intake and Prompt-Injection Checks

Teacher and contributor requests are untrusted content.

Later specs must require:

- Deterministic request intake checks only.
- Request text is stored as data, not executed as instruction.
- Prompt-injection content cannot change central policy.
- Prompt-injection content cannot create work packets directly.
- Prompt-injection content cannot bypass human review gates.
- Attachments require deterministic file, size, type, and license checks before use.

Required adversarial tests:

- A request containing instructions to ignore policy, create credential tasks, add arbitrary prompts, or mark output classroom-ready does not change central behavior.
- A request containing student names, emails, phone numbers, or school records is rejected or quarantined by deterministic PII rules.
- A request with unsupported subject, language, artifact type, or license preference is rejected or held for deterministic moderation.

## Content Moderation and Age-Appropriateness Checks

Later specs that touch public request intake, planning eligibility, generated text, artifact validation, review, or publication must require an explicit moderation gate.

The central core boundary remains strict:

- The central core must not call model moderation APIs directly.
- The central core must not import moderation provider SDKs.
- Moderation performed by a model provider, including OpenAI moderation, belongs in a runner or separate non-core moderation service.
- Moderation evidence is untrusted input until the central core validates schema, actor authority, lease lineage, idempotency, and policy result.

MVP request moderation:

- Accepted request intake creates a moderation task before any planning task is claimable.
- Planning task creation is allowed only after accepted moderation evidence says the request is allowed for the MVP audience and subject.
- Deterministic central inappropriate-content heuristics can reject or quarantine obvious unsafe requests before or during moderation-result acceptance.
- A positive moderation report cannot override deterministic central deny heuristics.
- If moderation evidence is unavailable, stale, cross-lineage, or failing, the request remains non-plannable, is rejected, or is quarantined by deterministic central policy.
- Human/curator override is deferred unless a later spec defines role, quorum, safe reason codes, and audit behavior.

MVP artifact moderation:

- Public text artifacts must pass deterministic static safety checks before `machine_validated`.
- The MVP does not add provider-backed artifact moderation evidence; inappropriate-content detection beyond deterministic heuristics is covered by human review and future moderation specs.
- A future artifact moderation report cannot replace deterministic validation or human review.
- A failing, missing, or cross-lineage future artifact moderation report must block public promotion and route to rejection or quarantine.

Provider-backed moderation rules:

- Provider-backed moderation may use a current moderation endpoint such as OpenAI `omni-moderation-latest` only from a non-core runner/service.
- Provider credentials, raw provider responses, request text beyond the moderated payload, and provider account details must not be sent to or persisted by the central core.
- Central records store only safe moderation category enums, pass/fail decision, redacted reason codes, provider-family bucket if allowed, and timestamps.
- Moderation prompts or policy text cannot be supplied by the user, planner, generator, or artifact manifest.

Required adversarial tests:

- A request containing sexual, violent, self-harm, hateful, illegal, or age-inappropriate instructions cannot create a planning task.
- Prompt injection that tells the moderator to ignore policy is treated as content, not instruction.
- A runner submits a forged positive moderation report for a different request or artifact and central rejects it.
- A valid moderation actor submits `allow_mvp_planning` for request text that matches deterministic unsafe-content heuristics and central still rejects or quarantines it.
- A moderation report with raw provider response, provider account data, prompt text, local paths, or secrets is rejected or redacted before persistence.
- An artifact containing inappropriate public text cannot become `peer_reviewed` even if manifest, validation, or review recommendation claims it is acceptable.

## Outbound Network and SSRF Checks

Later specs that touch attachments, license references, source context, metadata imports, previews, link checking, artifact fetching, or any central-core outbound network behavior must require one of these two positions:

- The MVP central core does not fetch attacker-controlled URLs at all.
- A later reviewed fetch subsystem explicitly defines allowlisted behavior.

An allowlisted fetch subsystem must require:

- Allowed schemes are explicit and exclude local file, shell, data, FTP, and other non-HTTP(S) schemes unless separately reviewed.
- Allowed hosts or domains are explicit when feasible.
- Private IP ranges, loopback, link-local addresses, multicast, broadcast, local hostnames, Unix sockets, and cloud metadata addresses are blocked.
- DNS rebinding protections are defined.
- Redirects are limited and revalidated after every hop.
- Request timeout, response size limit, content-type handling, and download storage limits are defined.
- Safe MIME handling applies before content is stored, previewed, or served.
- Central-core URL fetching does not carry runner credentials, user cookies, provider credentials, or internal service credentials.

Required adversarial tests:

- User-supplied URLs to `localhost`, `127.0.0.1`, `::1`, private IP ranges, and link-local metadata IPs are rejected.
- Redirects to private or link-local addresses are rejected after redirect resolution.
- Non-HTTP(S) schemes such as `file:`, `data:`, `ftp:`, and shell-like schemes are rejected.
- Oversized or slow responses are stopped by deterministic size and timeout limits.
- MIME spoofing does not cause unsafe storage, preview, or serving behavior.

## Identity, Authorization, and Replay Checks

Later specs that define endpoints, leases, submissions, reviews, or promotion must require:

- Actor classes are explicit: public requester, authenticated requester, runner, verifier runner, human reviewer, curator/admin, and system validator.
- Endpoint authorization matrices define which actor classes may call each endpoint.
- Actor identifiers in payloads are bound to authenticated sessions, API tokens, lease tokens, or local development test identities; they are not trusted from body fields alone.
- Lease-bound submissions require an active lease for the same actor, entity, and task.
- Verification reports used for promotion require a deterministic-core-created verification task, active lease, same proposal, current verifier runner, verifier/planner independence, and no blocking findings.
- Review submissions require reviewer role authority and an active review claim where claims are used.
- Idempotency keys and replay behavior are defined for mutating endpoints.
- Stale, replayed, wrong-actor, wrong-task, wrong-proposal, and expired-lease submissions are rejected with structured safe errors.
- Session secrets, API tokens, and lease tokens are classified as secret and must not appear in logs, errors, audit records, or public output.

Required adversarial tests:

- A caller forges `runner_id`, `reviewer_id`, `curator_id`, or `admin_id` in a request body.
- A runner submits to a task it has not claimed.
- A verifier submits a report for the wrong proposal or without an active verification lease.
- A planner tries to self-verify its own proposal.
- A reviewer without the required role attempts to approve or promote an artifact.
- A stale lease holder submits after lease expiry or after replacement claim.
- A mutating request is replayed with the same idempotency key and cannot duplicate state changes.
- A mutating request is replayed with altered payload under the same idempotency key and is rejected.

## Runner Submission Trust Checks

All runner submissions are untrusted until accepted by deterministic checks.

Later specs must require:

- Capability and trust are separate fields.
- Runner capability summaries are redacted allowlists, not full local config.
- Runner IDs do not imply human reviewer authority.
- Runner-submitted status claims cannot raise central status by themselves.
- Runner-submitted validation reports cannot replace trusted deterministic validation unless a later spec explicitly defines a reproducible acceptance path.
- Runner-submitted critique reports cannot approve plans, artifacts, or human review states.
- The same runner must not satisfy independence gates where distinct planner/verifier/reviewer roles are required.
- Same-operator or linked-identity laundering must be denied or routed to independent review where later trust rules can identify the link.

Required adversarial tests:

- A runner tries to submit full provider config, credentials, base URL, local auth path, or prompt template in capability summary.
- A planner tries to self-verify its own proposal.
- A worker tries to mark an artifact `machine_validated`, `peer_reviewed`, or `classroom_ready` through manifest fields.
- A runner submits a positive critique report and attempts to promote an artifact without human review.
- A worker submits `validation_report.json` claiming deterministic success through an artifact bundle, artifact metadata, or unauthorized validation endpoint; the artifact remains `draft_generated` until the trusted deterministic validator path succeeds.
- The same actor/operator, or identities linked by later trust rules, attempts to plan, verify, generate, review, and promote the same artifact; promotion is denied or routed to independent review where independence is required.

## Runner Sandbox and Tool-Execution Checks

Runners are attacker-adjacent execution surfaces because they consume untrusted request text, central work packets, and potentially model outputs.

Later specs that define runner behavior must require:

- Runner task content is data, not executable instruction.
- Runner implementations must not execute shell commands, scripts, tool calls, browser automation, Docker/VM control, SSH agents, package managers, or filesystem operations requested by user content, model output, proposed task graph fields, artifact content, or central work packet text unless a later tool-execution spec explicitly allows a closed command.
- MVP dummy generation runners may self-test generated `checker.py` only when a central work packet carries an explicit task-scoped sandbox execution policy from spec `013`.
- Future provider-backed runners disable model tool/function calling by default.
- If future runners need tool execution, that tool surface must have an allowlisted command set, argument schema, network policy, filesystem sandbox, resource limits, audit events, and targeted security review before implementation.
- Runner workspaces are per-claim and least-privilege. They must not expose the user's home directory, SSH keys, browser profiles, Codex auth files, shell history, Docker socket, cloud credentials, provider credentials except the local provider credential needed by that runner, or central API tokens beyond the in-memory token for the current process.
- Generated code may be executed only by the validator boundary defined by the artifact validation spec or by a runner self-test sandbox explicitly authorized by task-scoped policy from spec `013`.

Required adversarial tests:

- A request asks the runner to execute shell commands, read local files, exfiltrate environment variables, install packages, open a browser, or use Docker; runner output remains schema-only and no tool executes.
- A model/provider output contains tool-call JSON, shell commands, or code blocks requesting execution; runner treats it as inert text unless a later reviewed tool spec exists.
- A compromised runner submits output claiming that it executed validation or moderation locally; central ignores the claim unless it arrived through the trusted validator or moderation path.
- Runner logs and retained workspaces do not contain claim tokens, provider credentials, local paths outside workspace root, or raw prompt/model transcripts.

## Plan Verification Gate Checks

Later specs that define plan verification or proposal promotion must require:

- Verification tasks are created by deterministic core only after proposal schema and policy validation pass.
- Verification reports used for promotion are bound to a core-created verification task.
- Verification reports used for promotion require an active lease held by the verifier runner.
- Verification reports must reference the same proposal as the verification task.
- Verification reports must come from the currently leased verifier runner.
- Verification reports must satisfy planner/verifier independence.
- Verification reports with blocking, critical, or major findings cannot satisfy the promotion gate.
- Unsolicited verification reports may be rejected or stored only as advisory evidence; they cannot unlock promotion.

Required adversarial tests:

- A verifier submits a report before a verification task exists.
- A verifier submits without claiming the verification task.
- A verifier submits after lease expiry.
- A verifier submits for the wrong proposal.
- A planner self-verifies.
- A verification report contains blocking findings but attempts to unlock promotion.
- A forged positive verification report attempts to materialize work packets.

## Planning and Promotion Abuse Checks

Later specs that touch planning, validation, promotion, or reconciliation must require:

- Proposed task graphs are schema-valid before policy validation.
- Policy validation rejects unknown task types, forbidden fields, prompt-like fields, credential handling, provider-account handling, student grading, student placement, profiling, hidden network requirements, and missing human review gates.
- Dependency graphs are acyclic and reference only local IDs in the proposal.
- Risk labels from runners are advisory until deterministic policy rules accept or escalate them.
- Work-packet materialization is transactional and idempotent by source proposal and local task ID.
- Conflicting plans are never semantically merged in the central core.
- Reconciliation is performed by a runner or human actor, or by exact deterministic compatibility checks.

Required adversarial tests:

- A malicious planner under-labels a medium-risk plan as low risk.
- A planner omits human review gates.
- A planner includes hidden network or credential-handling requirements.
- Duplicate promotion requests cannot create duplicate work packets.
- Conflicting planner outputs route to reconciliation or human review rather than central semantic merge.

## Artifact and Generated-Code Checks

Later specs that touch artifacts, bundles, validators, or public downloads must require:

- Artifact paths are normalized and restricted to the bundle root.
- Hidden files, path traversal, absolute paths, oversized files, unsupported MIME types, unexpected executable content, executable content outside explicitly allowed MVP artifact types, and executable content that violates deterministic validator rules are rejected or quarantined according to deterministic rules.
- Generated Python checker execution has timeout, memory, process, filesystem, and network restrictions defined by the validator spec.
- `checker.py` is allowed only as an explicit MVP artifact type and only under deterministic validator timeout, filesystem, network, process, and import restrictions.
- HTML/JS simulations are deferred in the MVP and must be rejected or unavailable in the first slice.
- Artifact publication labels derive from central state and review records, not manifest self-claims.
- Artifacts disclose AI assistance, license, validation status, review state, and known limitations.

Required adversarial tests:

- Bundle includes `../secret`, absolute paths, hidden files, oversized files, unsupported MIME types, or external-network code.
- Manifest claims `classroom_ready` or `peer_reviewed` before central review state allows it.
- Checker imports network or process-spawning modules that the MVP validator forbids.
- Public download metadata omits review state or known limitations.

## Public Artifact Serving Checks

Later specs that touch public downloads, previews, artifact pages, object storage, or publication must require:

- Strict MIME allowlist for served artifact files.
- `X-Content-Type-Options: nosniff` or equivalent behavior.
- Safe `Content-Disposition` for downloads.
- Separate untrusted artifact origin, attachment-only serving, or equivalent isolation for untrusted content.
- No same-origin executable artifact rendering in the MVP.
- Content Security Policy for any rendered preview.
- Takedown, quarantine, and cache invalidation behavior for unsafe or deprecated artifacts.
- Public labels must derive from central publication state, not artifact manifest claims.

Required adversarial tests:

- HTML, script, SVG-with-script, polyglot, or MIME-spoofed uploads cannot execute in the central app origin.
- A file served with a misleading extension still receives safe MIME and nosniff behavior.
- A quarantined or deprecated artifact is no longer publicly downloadable after cache invalidation.
- A manifest attempts to override public status labels or serving headers and fails.

## Human Review and Publication Checks

Later specs that touch reviews, findings, promotion, or publication must require:

- Human review roles are distinct from runner capabilities.
- Review findings have structured severity and lifecycle.
- AI critique never equals human approval.
- Peer-reviewed promotion requires accepted human review in the MVP.
- Classroom-ready promotion is deferred unless a later reviewed spec defines reviewer qualifications and quorum.
- Conflict-of-interest and independence rules are defined for any required human review gate.
- Public status labels are conservative and derived from central state.

Required adversarial tests:

- A model critique tries to approve an artifact.
- A reviewer with insufficient role attempts to promote an artifact.
- An artifact with open blocking findings attempts to become `peer_reviewed`.
- Public metadata shows `classroom_ready` before configured classroom-ready gates pass.

## Logging, Error, and Audit Checks

Later specs that define logs, audit events, errors, or rejected payload handling must require:

- Logs use structured events and safe field allowlists.
- Error responses use stable codes and safe field names.
- Rejected-payload storage cannot retain secret-like values.
- Audit events record actor, action, entity, previous state, next state, timestamp, and safe reason codes.
- Audit events do not store full private runner config, local paths, credentials, prompt templates, cookies, or student PII.

Required adversarial tests:

- A fake secret in a rejected payload does not appear in logs, audit events, error traces, or operator-facing messages.
- A rejected prompt-injection payload records only safe policy codes and field names.
- State transitions are auditable without storing secret or forbidden content.

## Provider-Terms and Public Framing Checks

Later specs that touch public pages, onboarding, runner configuration, or provenance must require:

- The project is framed as a public commons, not hidden subscription pooling.
- The project does not promise guaranteed fulfillment through volunteer subscriptions.
- Volunteer credentials remain local.
- Exact provider/model disclosure is optional and must not leak private account details.
- Project-owned API execution, if added later, is separate from the deterministic core.

Required adversarial tests:

- Public copy does not invite users to donate quota, pool subscriptions, or make personal accounts available to others.
- Runner onboarding does not ask for credentials to be uploaded to the central platform.
- Provenance metadata cannot reveal auth mode, private base URL, local username, private model account, or exact local config.

## Abuse, Quota, and Availability Checks

Later specs that define public intake, runner polling, task leasing, artifact submission, validation, storage, or review queues must require:

- Per-actor, per-token, and where applicable per-IP rate limits.
- Attachment and artifact size limits.
- Per-actor storage quotas or deterministic storage abuse controls.
- Queue depth limits or backpressure behavior.
- Lease and heartbeat abuse controls.
- Duplicate submission throttles.
- Repeated failed submission throttles or cooldowns.
- Deterministic quarantine/takedown states.
- Abuse report workflow for requests and artifacts.
- Operator-visible abuse metrics that do not expose secrets or forbidden content.

Required adversarial tests:

- Public request flood is rate-limited or backpressured.
- Malicious runner floods eligible/claim/heartbeat/submit endpoints.
- Oversized artifact bundle storms cannot exhaust storage.
- Repeated malformed proposal or artifact submissions trigger deterministic throttling or quarantine.
- Abuse reports can hide or quarantine public artifacts without deleting audit history.

## Checklist Use in Later Specs

Every later spec must include a short section named `Security Checklist Coverage` with:

- Applicable checklist categories.
- Items satisfied directly by the spec.
- Items intentionally deferred.
- Any acceptance tests copied from this checklist.

Reviewers must fail later specs that omit `Security Checklist Coverage`.
