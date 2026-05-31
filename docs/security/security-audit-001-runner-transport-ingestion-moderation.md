# Security Audit 001: Runner, Transport, Ingestion, and Moderation Hardening

Status: Passed adversarial review
Date: 2026-05-30

## Scope

This audit covers the passed spec set with focus on:

- malicious code execution through runners;
- runner sandboxing and local workspace isolation;
- central API transport, MITM, redirects, proxy interception, and token leakage;
- request ingestion and prompt injection;
- compromised runner submissions and cross-lineage evidence;
- inappropriate request or artifact content;
- moderation architecture without violating the deterministic-core boundary.

## Threat Model Summary

Primary attacker-controlled inputs:

- public request fields;
- request constraints that include prompt-injection text;
- runner configuration and capability summaries;
- runner-submitted proposals, moderation reports, verification reports, artifact references, and artifacts;
- generated `checker.py`;
- review and critique payloads;
- public metadata requests.

Primary assets:

- central API tokens and claim tokens;
- runner provider credentials and local secrets;
- contributor/requester private data;
- artifact integrity and public publication labels;
- central state-machine and review authority;
- runner host filesystem and local execution environment.

Critical invariants:

- central backend remains deterministic and never calls model or moderation providers;
- untrusted task content cannot trigger runner shell/tool/code execution;
- runner and validator credentials cannot leak through logs, artifacts, API payloads, redirects, proxies, or public metadata;
- request planning is blocked until request moderation passes;
- generated artifact publication is blocked by deterministic validation and human review, with obvious inappropriate content rejected by validator heuristics;
- compromised runners cannot forge identity, lineage, validation, moderation, review, or promotion authority.

## Audit Findings and Spec Hardening

### Finding A: Request moderation was implicit, not a blocking gate

Risk:

An inappropriate request could pass deterministic shape checks and become a planning task before content moderation happened.

Hardening:

- Added a request moderation task/report gate before planning.
- Planning task creation now requires accepted moderation evidence.
- Positive moderation evidence cannot override deterministic central inappropriate-content heuristics.
- Provider-backed moderation is allowed only outside the central core.
- Forged, stale, cross-lineage, or raw-provider moderation reports are rejected.

Affected specs:

- `001`
- `002`
- `005`
- `006`
- `008`
- `009`
- `012`

### Finding B: Runner sandbox policy was too implicit

Risk:

An attacker could attempt to make a runner execute commands via request text, model output, proposed graph fields, artifact text, or tool-call-shaped JSON.

Hardening:

- Added explicit runner sandbox and tool-execution checks.
- MVP runners require `allow_tool_execution=false`.
- Runners must not execute shell commands, scripts, package managers, browser automation, Docker/VM control, SSH agent operations, filesystem operations requested by content, or model tool/function calls.
- Future tool execution requires a separate spec with allowlisted commands, argument schemas, mounts, network policy, resource limits, audit events, and security review.

Affected specs:

- `002`
- `009`
- `012`

### Finding C: Non-loopback runner transport needed stronger trust policy

Risk:

Generic HTTPS validation reduces MITM risk, but non-loopback runner deployments need explicit trust posture to prevent accidental token disclosure to an intercepted or wrong endpoint.

Hardening:

- Non-loopback runner deployments must configure a TLS trust policy: pinned private CA, pinned certificate/SPKI hash, or a later reviewed mTLS profile.
- Platform trust without a pin is local/deferred.
- TLS peer name must match `central_api_base`.
- Unsafe origins, failed TLS, redirects, ambient proxies, and missing pins fail before tokens are sent.

Affected specs:

- `009`
- `012`

### Finding D: Artifact inappropriate-content screening needed an explicit deterministic check

Risk:

Generated public text could include obviously inappropriate content and rely only on later human review.

Hardening:

- Added `obvious_inappropriate_content_heuristic` to MVP validation plan.
- Validator checks public text files and manifest public text fields.
- Check is deterministic, conservative, and provider-free.
- Matched unsafe text is not persisted in reports/logs/public metadata.

Affected specs:

- `001`
- `007`
- `009`
- `010`
- `012`

## Deferred Risks

The following remain intentionally deferred and require future specs before implementation:

- provider-backed planner/generator modes;
- provider-backed moderation runner implementation details;
- model tool/function calling;
- browser/web-fetch runners;
- mTLS runner auth profile;
- artifact model-moderation beyond deterministic heuristics;
- curator override for moderation decisions;
- public artifact download serving beyond current metadata restrictions.

## Review Checklist

Reviewers should fail this audit if:

- request planning can occur without accepted request moderation evidence;
- central core can call OpenAI or any model/moderation provider;
- runner task content can trigger local shell/tool/code execution;
- non-loopback runner tokens can be sent without explicit TLS trust policy;
- a valid moderation runner can mark obviously unsafe request text as safe and create a planning task;
- generated public text can bypass deterministic inappropriate-content heuristics;
- forged runner moderation/validation evidence can create trusted state;
- raw provider moderation responses, provider credentials, prompts, local paths, or unsafe excerpts can be persisted centrally.
