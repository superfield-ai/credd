# credd

A credential daemon that combines a sealed secret store, an execution proxy, an AI vendor proxy, a leaked-secret monitor, and multi-human approval — in a single self-hosted binary with zero external service dependencies.

credd replaces the stack most teams cobble together (Vault + sops + aws-vault + gitleaks + manual rotation runbooks) with one daemon that has a coherent security model from a developer's sandbox keys to a production cluster's signing credentials.

## Architecture

Two processes, not one.

**credd-core** owns the unsealed key material and does three things: decrypt a lease, sign an audit event, verify a multisig bundle. It has no network listener and no JSON parser for untrusted input — it speaks a tiny length-prefixed protocol over a Unix socket it owns.

**Guards** (AI proxy, exec proxy, leak monitor, MCP server) are separate stateless processes that hold nothing durable. A guard compromise gets an attacker only the leases that guard currently holds — never the root key.

```
┌─────────────────────────────────────────────────────────┐
│                credd-core (sealed, privileged)           │
│  Envelope key · AEAD store · audit signing · quorum     │
│  No network · No JSON parser · Dedicated UID · seccomp  │
└──────────────────────────┬──────────────────────────────┘
                           │ Unix socket (length-prefixed)
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  AI Proxy    │  │  Exec Proxy  │  │ Leak Monitor │  ...
│  Guard       │  │  Guard       │  │ Guard        │
│  (stateless) │  │  (stateless) │  │  (stateless) │
└──────────────┘  └──────────────┘  └──────────────┘
```

Every secret is classified into one of three tiers (Local, Leased, Ceremonial) with distinct security properties, and every use is attributed, rate-limited, and logged in a hash-chained, Ed25519-signed audit trail shipped to an append-only sink.

---

## How credd Compares

### vs. HashiCorp Vault

Vault is the industry standard for centralized secret management. It does many things well: dynamic secrets, lease-based access, pluggable auth backends, a rich policy language. credd is not trying to replace Vault in the data center.

Where credd diverges:

| | Vault | credd |
|---|---|---|
| **Deployment** | Requires its own HA cluster (often with Consul), an ops team, and ongoing infrastructure cost | Single binary, runs on a laptop or a production node, zero external services |
| **License** | BSL 1.1 (no longer open source) | Open source |
| **Dev-machine story** | Vault Agent or `vault` CLI with env-var injection; no execution proxy, no egress scanning | Native exec proxy with wire injection, AI proxy with egress scanning, leak monitor — all local |
| **Credential injection** | Env vars, file templates, or API calls — the child process holds the secret | Wire injection by default (HTTPS_PROXY + per-run CA); child never holds the credential |
| **Multi-human approval** | Enterprise-only Control Groups; Shamir unsealing is init-time only | M-of-N hardware-backed signatures on every Tier 2 operation and policy change, included in the open-source core |
| **Audit chain** | HMAC-hashed audit log to file/syslog; no cryptographic hash chain, no signed events | Ed25519-signed hash chain shipped to a WORM sink; gaps are detectable and alertable |
| **AI-agent awareness** | None — designed for services and humans, not for prompt-injected agents that run arbitrary code | Threat model starts from "the agent may be compromised"; spend caps, egress scanning, attribution per principal |

**When to use Vault instead**: You already run Vault, your secrets are centralized across hundreds of services, and you need its plugin ecosystem (database dynamic creds, PKI CA, transit encryption). credd is not a Vault replacement for large-scale centralized secret management.

**When to use credd instead**: You need credential management on developer machines, in CI, or in small-to-medium clusters where running a Vault cluster is overkill. You want wire injection, egress scanning, and multi-human approval without an enterprise license.

---

### vs. Hermes

[Hermes](https://github.com/rubra-ai/hermes) pioneered the execution-proxy pattern for AI agents: sandboxed command execution, an egress proxy (`iron-proxy`) that swaps opaque tokens for real credentials on the wire, dangerous-command approval workflows, and session isolation.

credd's exec proxy and AI proxy are directly inspired by this architecture. The differences are in scope and trust model:

| | Hermes | credd |
|---|---|---|
| **Scope** | Agent execution framework (chat, tools, backends) | Credential daemon only — works with any agent framework, CLI tool, or container |
| **Secret storage** | Delegates to external stores; no built-in sealed store | Built-in AEAD envelope store with tiered access, TPM/Enclave/KMS sealing |
| **Credential lifecycle** | Proxy token → real credential swap at the egress proxy | Full lifecycle: mint short-lived derivatives, enforce spend caps, rotate, revoke, audit |
| **Multi-human approval** | Dangerous-command approval (human-in-the-loop for risky commands) | Cryptographic M-of-N quorum on hardware keys for Tier 2 secrets and all policy changes |
| **Audit** | Application-level logging | Cryptographically signed hash chain with WORM sink |
| **Leak detection** | Not built-in | Integrated monitor: git hooks, proxy egress, shell history, clipboard |
| **Production deployment** | Designed for single-agent instances | Designed to scale: two cores per zone, SPIFFE mTLS between nodes |

**When to use Hermes instead**: You want a complete agent framework with chat integrations (Telegram, Discord, Slack), tool orchestration, and sandboxed backends. Hermes is an agent runtime; credd is infrastructure.

**When to use credd instead**: You need credential management that works for agents *and* for non-agent workloads (CI pipelines, container deployments, developer machines), with cryptographic audit and multi-human approval.

---

### vs. AI Proxy / Gateway Services

Services like **LiteLLM**, **Portkey**, **Cloudflare AI Gateway**, **Kong AI Gateway**, and **OpenRouter** act as reverse proxies between your application and upstream LLM providers. They provide virtual keys, spend caps, rate limits, model routing, and usage dashboards.

| | AI Gateways | credd |
|---|---|---|
| **Deployment** | SaaS or self-hosted proxy server | Local daemon — no cloud dependency, no network hop for credential decisions |
| **Scope** | LLM API traffic only | Any credential: cloud IAM, database, SSH, signing keys, vendor APIs — LLM keys are one use case |
| **Credential model** | Virtual keys that map to real provider keys stored in the gateway | Tiered secret store with AEAD encryption, short-lived derivatives, and wire injection |
| **Security boundary** | You trust the gateway service with your real API keys | Keys sealed to local hardware (TPM/Enclave/KMS); no third party holds them |
| **Egress scanning** | Token counting, cost tracking, some content filtering | Secret fingerprinting, pattern matching, high-entropy detection — blocks requests containing credentials |
| **Audit** | Dashboard and log export | Cryptographically signed hash chain |
| **Exec proxy** | Not applicable — API traffic only | Full execution proxy for shell commands, containers, and CLI tools |
| **Multi-human approval** | Not available | M-of-N hardware-backed quorum |

**When to use a gateway instead**: You need multi-model routing, fallback chains, prompt caching, or a managed dashboard for LLM spend across a large org. These are genuine features credd does not provide.

**When to use credd instead**: You need the LLM proxy to be one part of a broader credential management story — not a separate SaaS you hand your keys to.

---

### vs. Arcade

[Arcade](https://arcade.dev) provides zero-token-exposure credential brokering specifically for AI agents. The agent never sees the real credential; Arcade injects it at execution time and returns only the result. It's MCP-native with OpenTelemetry audit.

| | Arcade | credd |
|---|---|---|
| **Deployment** | Cloud-hosted SaaS | Self-hosted, open source, zero external dependencies |
| **Trust model** | You trust Arcade's infrastructure with your credentials | Credentials never leave your hardware; sealed to TPM/Enclave/KMS |
| **Scope** | AI agent API tool execution | Any credential consumer: agents, CLI tools, containers, CI, services |
| **Credential injection** | API-level brokering (Arcade executes the tool call) | Wire injection (HTTPS_PROXY) + exec proxy with namespace isolation — works for shell commands, not just APIs |
| **Leak detection** | Not built-in | Integrated runtime monitor |
| **Multi-human approval** | Not available | M-of-N hardware-backed quorum |
| **Audit** | OpenTelemetry events | Ed25519-signed hash chain with WORM sink |
| **Cost** | Subscription | Free and open source |

**When to use Arcade instead**: You want managed, zero-ops credential brokering for AI agents and you're comfortable with a cloud trust model. Arcade is polished and works today.

**When to use credd instead**: You need self-hosted credential management, you have non-agent workloads, you need multi-human approval, or your security posture requires that credentials never leave hardware you control.

---

## Status

Early development. See [DESIGN.md](docs/DESIGN.md) for the full design proposal.

## License

TBD
