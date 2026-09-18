# credd — Credential Daemon Design Proposal

A single daemon combines a Vault-like secret store, a leaked-secret monitor, and a Hermes-style execution proxy for agent-run commands — with multi-human approval gating every critical operation.

**Draft, 18 September 2026**

## Threat Model

On a developer machine, credd assumes the agent may be prompt-injected and may run arbitrary code as the developer's own user. It does not try to hide secrets from a same-UID attacker. Instead it makes every secret use attributable, rate-limited, and revocable, and moves the secrets that actually matter behind a boundary the local user cannot cross: other humans.

In production, credd assumes the host itself may be compromised and is designed so a compromised host can only spend what it has already been leased.

## Architecture: Sealed Core, Disposable Guards

Two processes, not one.

**credd-core** owns the unsealed envelope key and does exactly three things: decrypt a lease, sign an audit event, verify a multisig bundle. It has no network listener and no JSON parser for untrusted input — it speaks a tiny length-prefixed protocol over a socket it owns (policy is signed YAML and is parsed only after Ed25519 verification, so the no-JSON-parser property holds narrowly for the untrusted Unix-socket path). On Linux it runs under a dedicated user with a seccomp profile; on macOS as a launchd daemon under a separate UID.

**Guards** (exec proxy, monitor, MCP server) are separate stateless processes that hold nothing durable and can be restarted at will. A guard RCE gets an attacker exactly the leases that guard currently holds and nothing else — never the root key.

This split trades a small amount of IPC overhead for a hard compromise boundary: parsing untrusted vendor responses and untrusted agent JSON happens far away from the key material.

## Secret Tiers

Every secret is classified at creation:

| Tier | Examples | Read access | Use |
|------|----------|-------------|-----|
| 0 — Local | Dev sandbox keys, scoped test tokens | Humans can read | Agents: use-only |
| 1 — Leased | Cloud roles, vendor keys | Nobody reads the root | Short-lived derived credential only (STS session, scoped vendor sub-key, signed URL) |
| 2 — Ceremonial | Root cloud credentials, signing keys, prod DB masters, store's own recovery material | Quorum required for any operation | Quorum required, including read, rotate, re-tier, policy change |

For Tier 1, credd-core mints the derivative credential; the root secret never leaves the store.

## Multisig for Critical Operations

Tier 2 operations and all policy mutations are canonical request objects — operation, target path, requesting principal, expiry, nonce — and execute only on M-of-N hardware-backed signatures.

- Each approver signs the request hash with a key held in a YubiKey, Secure Enclave or TPM on their own machine, never on the requesting host.
- credd-core verifies the signatures against an approver set that is itself only changeable by quorum, executes once, then burns the nonce.
- Signatures bind to the exact request bytes and carry an expiry, so a compromised requesting host cannot swap the payload after approval or replay it later.
- Tier 2 KEK material is additionally split M-of-N (Shamir) across approver tokens, so the host cannot unseal it even with root.
- Quorum also gates global lease freeze, approver set rotation, and post-incident unseal. Tier 1 rotation does not need quorum: one human, plus a canary request against the new credential before the old one is revoked. Approval routing and reviewer UI are described in Note 1.

## Guards

**Execution proxy.** Prefers wire injection over process injection: for HTTP-based tools it sets HTTPS_PROXY and a per-run CA, and credentials are added at the proxy so the child process never holds them. For tools that require a real credential in-process (SSH, database drivers), it injects a Tier 1 derivative with a lease measured in minutes, runs the child in a PID and mount namespace, and records in the audit event that this run relied on the weaker path so reviewers can see it.

**Leak monitor.** Not a general network sniffer (TLS makes that blind without an on-host CA, which is a bigger risk than the one it solves). Instead it runs as hooks where it can see plaintext: git pre-commit and pre-push, the execution proxy, shell history, clipboard. A hit revokes the lease and, for Tier 1 secrets, opens a rotation ticket automatically.

## Attack Vectors and Defenses

This is the section that decides whether the design is real. Each vector below names the concrete mechanism an attacker uses, the control that blocks or bounds it, and what is left over.

| # | Vector | Primary control | Residual exposure |
|---|--------|----------------|-------------------|
| 1 | Arbitrary code execution as the developer's user | No readable root secret on the host; derived credentials with minute-scale TTL | TTL-bound, fully logged |
| 2 | Caller impersonation / PID reuse | pidfd-pinned peer credentials, executable hash, LSM or SVID backing | Advisory only on unhardened dev machines |
| 3 | Disk theft, cold boot, backup exfiltration | Envelope AEAD, KEK sealed to TPM/Enclave/KMS, Tier 2 split M-of-N | Tier 0 exposed if login keychain is compromised |
| 4 | Guard RCE via untrusted JSON | Separate UID, seccomp allowlist, netns egress allowlist, no KEK in guard | Leases that guard currently holds |
| 5 | Leakage through the child process | Wire injection; fd/memfd delivery; hidepid, no core dumps, normalized output filter | In-process path for SSH and DB drivers |
| 6 | Exfiltration through the model | Out-of-band log scanning with normalization | Semantic paraphrase of a secret |
| 7 | Audit tampering | Ed25519 hash chain, WORM sink in a separate account | Events not yet shipped at compromise time |
| 8 | Policy or binary tampering | Signed policy, quorum on change, signature-verified plugin load | Compromise of the signing quorum |
| 9 | Upstream MITM / SSRF | Domain allowlist, pinned roots, no proxy-supplied base URLs | None material |
| 10 | Single malicious insider | M-of-N quorum on Tier 2 and policy | Collusion of M approvers |

### 01 — Arbitrary code execution as the developer's user

A prompt-injected agent runs whatever it likes under the developer's UID. It can ptrace an allowed process, preload a shared library into it, read /proc/<pid>/environ, or simply re-exec the binary credd's policy allows with hostile arguments. No same-UID mechanism stops this, so credd does not pretend to.

What the design does instead:

- **Nothing worth stealing sits at rest.** Tier 1 and Tier 2 roots never materialize on the host. The attacker can obtain a derived credential, not the credential.
- **Derivatives are short.** STS sessions and vendor sub-keys are minted with minute-scale TTLs, so the theft window is bounded.
- **Every mint is attributed.** The audit event carries the caller's executable hash, parent chain, cgroup, and the guard used, which is what turns silent theft into a detectable anomaly.
- **Mint-rate anomaly detection.** A principal that suddenly mints ten times its baseline gets throttled and flagged, and a flagged principal's Tier 1 access can be frozen without quorum.
- **Host hardening is documented, not assumed.** credd-core runs under its own UID so cross-UID ptrace requires root; the installer sets kernel.yama.ptrace_scope=1 on Linux and ships the guards with the macOS hardened runtime and no get-task-allow entitlement.

Residual: an attacker who is present right now can spend what the policy already permits, with every request in the log, until the lease expires.

### 02 — Caller impersonation and PID reuse

Process identity over a Unix socket is a race unless it is pinned. credd reads peer credentials with SO_PEERCRED (LOCAL_PEERCRED on macOS) at accept() time, then immediately opens a pidfd for that PID so a recycled PID cannot be substituted mid-handshake. The executable is hashed through /proc/<pid>/exe held as an O_PATH descriptor, not by re-reading the path, which closes the swap-the-file-after-check window.

That still only proves which binary, not that the binary is honest. So the trust level is explicit:

- **Dev machine, no LSM**: process identity is advisory. It is used for attribution and for policy convenience, and the security story rests on tiering and TTLs, not on identity.
- **Dev machine with SELinux/AppArmor, or macOS with a code-signature check**: identity becomes meaningful, because the LSM prevents the injection paths that would forge it.
- **Production**: process identity is not used at all. Callers present a SPIFFE X.509 SVID over mTLS, and policy binds to the SPIFFE ID.

### 03 — Secrets at rest, memory, and stolen hardware

Each secret version is sealed with its own data key under an AEAD (XChaCha20-Poly1305), with the path, version, and tier bound in as additional authenticated data so a ciphertext cannot be moved between paths or downgraded to a lower tier. Data keys are wrapped by a key-encryption key that never exists in plaintext outside the core process.

Where the KEK is sealed depends on the host: TPM 2.0 with a PCR policy, the Secure Enclave, or cloud KMS with an instance role. Tier 2 material goes further — its KEK is Shamir-split across approver hardware tokens, so a stolen laptop or a rooted server holds nothing that can be unsealed locally at any privilege level.

In memory, credd-core locks pages with mlock to keep them out of swap, zeroizes buffers after use, disables core dumps (RLIMIT_CORE 0 and PR_SET_DUMPABLE 0), and refuses to start on a host with an unencrypted swap device unless explicitly overridden. Plaintext lives in the core's address space for the duration of an operation and nowhere else.

The honest caveat: on a dev machine Tier 0 unseals automatically at login from the OS keychain. Anything running as that user after login can ask for those secrets, so Tier 0's protection is exactly the keychain's, no more. That is why sandbox keys are the only things allowed in Tier 0.

### 04 — Guard compromise

Guards are where untrusted bytes arrive: agent-supplied JSON on one side, vendor responses on the other. They are assumed to be exploitable and are built to be cheap to lose.

- Each guard runs under its own UID with a seccomp-bpf syscall allowlist and no capabilities.
- The filesystem view is read-only except for its own socket; there is no writable path a payload could drop a persistence hook into.
- Network egress runs in a dedicated namespace with a destination allowlist, so a compromised guard cannot open a socket to an attacker's collector.
- A guard never holds the KEK, never sees a Tier 2 secret, and cannot read the store. It asks the core for a lease and gets back a derivative.
- Guards are stateless. A crash, a restart, or a forced kill loses nothing, and the supervisor restarts from a clean image on any abnormal exit.

Residual: the leases that guard is holding at the moment of compromise, for the remainder of their TTL.

### 05 — Leakage through the executed child

This is the vector most credential brokers get wrong. If you hand terraform, aws, or gh a real secret, it will eventually write it into a debug log, a crash dump, a state file, or an error message, and /proc/<pid>/environ is readable by the same UID no matter what the parent scrubs.

The default path avoids handing it over at all. For HTTP-based tools the exec proxy sets HTTPS_PROXY and a per-run CA whose private key stays in the guard and whose certificate is valid for minutes; credentials are attached on the wire, and the child holds nothing. The proxy also constrains the child to the destinations its profile allows, which incidentally blocks a compromised tool from calling anywhere else.

When a real in-process credential is unavoidable — SSH keys, database drivers, signing tools — the design degrades deliberately and says so:

- Delivery is by file descriptor or memfd, or an O_TMPFILE unlinked file on tmpfs, rather than an environment variable, wherever the tool supports it.
- The child runs in fresh PID, mount, and IPC namespaces with /proc mounted hidepid=2, so sibling processes cannot enumerate it.
- Core dumps are disabled for the child and its descendants, and the tmpfs mount is torn down when the process group exits.
- The lease is minutes long and is revoked at exit regardless of TTL remaining.
- The audit event is tagged weak_path=true, so a reviewer can query exactly which runs relied on in-process injection rather than discovering it during an incident.
- Output redaction normalizes before matching — base64, hex, percent-encoding, JSON string escapes, and whitespace-split forms — because a filter that only greps the literal string is decorative.

### 06 — Exfiltration through the model

An injected agent can try two things: persuade the model to echo a secret it was given, or paste a config file whose secret the fingerprint set does not know.

Detection is out-of-band. The leak monitor tails operation logs and applies three detectors in series: known-secret fingerprints (salted prefix match after normalization), gitleaks-style pattern rules for vendor key shapes, and a high-entropy-string check with an allowlist for known-benign blobs like lockfile hashes. A hit revokes the associated lease and raises an audit event, rather than silently redacting — silent redaction teaches agents to retry with an encoding that survives.

There is no inline proxy blocking or pinned upstream allowlist on this path; bounding relies on short lease TTLs, revocation, and the audit trail.

Residual: a model can be asked to paraphrase or describe a secret in a form no detector matches. The audit trail bounds the damage; nothing prevents semantic paraphrase.

### 07 — Audit tampering

An attacker who gets code execution will try to erase the evidence, and the log is written by a process on the machine they now control. Two properties make that fail loudly rather than silently.

Every event carries a monotonic sequence number and the hash of its predecessor, and the whole record is signed with the core's Ed25519 key. Deleting or editing an event breaks the chain at a point a verifier can name. Events ship continuously to an append-only sink — object storage with a WORM lock, in a separate cloud account, reachable with write-only credentials — so the host cannot delete what has already left. The local buffer exists only to survive network blips.

A verifier job replays the chain on the sink and alerts on gaps, sequence rollbacks, or signature failures. Residual exposure is the handful of events buffered but not yet shipped at the moment of compromise, which is why the shipping interval is seconds rather than minutes.

### 08 — Policy, plugin, and supply-chain tampering

Rewriting policy is cheaper for an attacker than stealing a key, so policy is treated as Tier 2. The policy document is signed, the core refuses to load an unsigned or stale-signature version, and any change — including adding a principal or widening a path glob — is a quorum operation.

Guards and rotation plugins are verified by signature at load time against a build-time trust root. Updates ship with TUF-style metadata so a compromised distribution point cannot serve a rollback or a targeted build, and the binaries are reproducible so the signature means something a third party can check. The approver set itself is a signed document that only the existing quorum can change, which closes the obvious escalation of adding yourself as an approver.

### 09 — Network path attacks

All upstream connections use TLS with pinned roots and a fixed domain allowlist. There is no mechanism for a caller to introduce a new destination at runtime, so DNS poisoning or a hostile proxy configuration cannot redirect a credentialed request. In production, guard-to-core and node-to-node traffic is mTLS with SPIFFE SVIDs rotated hourly.

The leak monitor deliberately does not MITM general host traffic. Installing a root CA on every developer machine to inspect TLS creates a larger attack surface than the leakage it would catch, so the monitor watches only surfaces where plaintext is legitimately available: the execution proxy, git pre-commit and pre-push hooks, shell history, and the clipboard.

### 10 — Insider abuse

Every control above assumes the operator is honest. Multisig is the control that does not. Tier 2 access, policy changes, approver-set changes, and post-incident unseal all require M-of-N signatures from distinct humans on distinct hardware, so a single compromised or malicious operator — including one with root on every production host — cannot reach the material that matters.

Residual: collusion among M approvers, which is a people problem and should be priced as one when choosing M.

## Audit and Availability

Every event is hash-chained and signed by credd-core, then shipped to an append-only remote sink the host cannot delete from. In production the sink is S3/GCS Object Lock (WORM) in a separate account with write-only credentials; on dev machines it is the team's central credd instance (local file sink). Chain gaps are alerts.

Guards fail closed. credd-core unavailability blocks new leases, but leases already issued keep working until expiry — a five-minute outage does not break a deploy in progress. Revocation during a core outage is therefore bounded by the lease's remaining TTL. Production runs two cores per zone against a shared sealed store on KMS.

## Residual Risks

A same-UID attacker can still spend Tier 0 and Tier 1 leases while they remain valid. That window is bounded by the lease TTL, and every use is logged.

Nothing on the host can reach Tier 2 secrets or change policy without other humans saying yes.

## Notes

### Note 1 — Approval routing and reviewer UI

Requests are broadcast to approvers through a pluggable notifier (Slack, PagerDuty, or a CLI inbox); the channel is a convenience and carries no trust, since the signature is over the request bytes and is verified independently. The reviewer sees a diff rather than a summary — for a policy change, the literal YAML diff; for a Tier 2 read, the path, the requesting principal, and the recent audit history for that principal. Approvals are time-boxed, and an approver who signs from the requesting host is rejected.
