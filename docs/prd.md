# Product Requirements Document

## 1. Problem Statement
The toolchain for secret management across developer environments, continuous integration pipelines, and containerized clusters is heavily fragmented. Teams assemble bespoke stacks of centralized secret stores, file encryption utilities, local credential helpers, and repository leak scanners. This fragmentation introduces security vulnerabilities—specifically around AI agents that may execute arbitrary code or prompt inject on developer machines, as well as general key management sanity in containerized deployments. There is no unified system that protects credentials locally, bounds the blast radius of compromised agents, enforces multi-human approval for critical operations, and prevents secrets from leaking into child processes or external proxies.

## 2. Goals and Success Metrics
- **Consolidation**: Replace disparate secret management and leak detection tools with a single unified credential management system.
- **Agent Containment**: Ensure compromised agents running locally cannot extract root secrets and can only consume bounded, attributed derivative credentials.
- **Cryptographic Assurance**: Guarantee that all critical operations and policy changes require verifiable multi-human quorum.
- **Success Metrics**: Success is measured qualitatively by successful containment and adoption across environments.

## 3. User Roles
- **Developer**: Interacts with local sandbox keys and tiered secrets for daily development, utilizing the local execution proxy and out-of-band log monitoring.
- **Security Engineer**: Defines global policies, configures leak monitoring, reviews audit trails, and participates in multi-human quorum for ceremonial secrets.
- **System Administrator / Operator**: Deploys the system to containerized clusters, manages the physical deployment lifecycle, and monitors overall health.
- **Target Audience**: The product is built for teams and businesses, meaning shared quorum workflows, team topology, and collaborative approval are primary operational modes.

## 4. User Stories
- **As a Developer**, I want my command-line tools to securely receive credentials without environment variables exposing them to child processes.
- **As a Developer**, I want my agent operations monitored via out-of-band log tailing, so that any exfiltrated secrets in logs trigger immediate lease revocation.
- **As a Security Engineer**, I want all access to production signing credentials to require a multi-human hardware-backed signature, so that a single compromised machine cannot leak the key.
- **As a Security Engineer**, I want out-of-band log scanning to detect and revoke leases for suspected secrets without requiring inline egress blocking, to keep the hot path fast.
- **As an Operator**, I want a cryptographically verifiable, append-only audit trail shipped off-host, so that post-compromise tampering is impossible.

## 5. Core Workflows
- **Secret Retrieval via Proxy**: A local tool makes a request; the local credential system intercepts it, mints a short-lived derivative credential, attaches it to the wire payload, and forwards the request without exposing the root secret to the calling process.
- **Multi-human Approval**: A user requests a change to policy or access to a ceremonial secret. The request is broadcast. Designated quorum members sign the exact request hash using hardware tokens. The core system verifies the signatures against the current policy before executing the request.
- **Out-of-band Log Monitoring**: A guard tails the operation logs of agents or containers, scanning for secret fingerprints, pattern matches, and entropy checks. If a leak is detected, the associated lease is revoked and an audit event is raised.
- **Secret Import**: Users can provide flat key-value pair files to provision secrets into the store, but no direct integration with external networked secret stores is supported.
- **Bootstrapping and Recovery**: The system supports both strict multi-party cryptographic ceremonies for production initialization and simpler single-admin configuration files for testing environments. Using the single-admin configuration in production generates explicit administrative warnings.

## 6. Entity Lifecycle
- **Secrets**: Classified at creation into Local, Leased, or Ceremonial tiers. Re-tiering requires quorum. 
- **Leases**: Short-lived, derived credentials minted upon request. Expire based on a defined time-to-live. Can be revoked immediately upon detection of anomalous behavior in logs.
- **Audit Events**: Generated for every credential use. Cryptographically chained and signed immediately, then continuously shipped to an external append-only sink.

## 7. Integration Needs
- **Hardware Security Modules**: For sealing root keys and signing quorum requests.
- **Append-only Remote Storage**: For receiving and storing the continuous stream of hash-chained audit events.
- **Workload Identity Frameworks**: For process identity and mutual authentication in production deployments.
- **Logging Frameworks**: To tail agent and container operations for out-of-band secret detection.

## 8. Out of Scope
- Centralized fleet-wide secret distribution and dynamic identity infrastructure (the system focuses on the edge/node credential boundaries).
- General-purpose network sniffing or man-in-the-middle inspection of all host traffic (monitoring is restricted to known plaintext boundaries).
- Silent redaction of detected secrets (the system strictly revokes leases to avoid evasion learning).
- Complete evasion prevention for semantic paraphrasing of secrets by a language model.
- Inline vendor proxies, inline egress scanning, and spend caps (monitoring relies on out-of-band log tailing instead).

## 9. Constraints
- **Zero External Dependencies**: The system runs as a single binary without relying on external orchestration or database clusters.
- **Self-Hosted Only**: The product is strictly self-hosted open-source software. There is no managed or hosted offering.
- **Open-Source License**: The software is licensed under a permissive open-source license.
- **Platform Support**: Primary targets must support process namespace isolation and containerization to guarantee full security boundaries. Environments lacking native namespace capabilities are treated as convenience targets with explicitly reduced isolation guarantees.
- **Local Isolation**: The architecture strictly separates the privileged core from unprivileged, stateless guards that parse untrusted input.
- **Fallback Execution**: For tools that cannot support wire injection, the system degrades safely to namespace-isolated process injection while clearly flagging the action in the audit log.

## 10. Open Questions
- **Approver UI Modality**: Is the approval interface exclusively text-based or graphical? *Default assumption: the approver interface is exclusively text-based, with pluggable notifiers alerting users to pending requests.*

## 11. Glossary
- **Envelope encryption**: A practice of encrypting plaintext data with a data key, and then encrypting the data key with another key.
- **KEK**: Key Encryption Key, a key used to encrypt other keys for protection at rest.
- **DEK**: Data Encryption Key, a key used to encrypt the actual data payload.
- **Secret sealing**: The process of wrapping a key with hardware-backed encryption or a KMS to protect it.
- **Lease**: A time-bound grant of access to a secret or derived credential.
- **AEAD**: Authenticated Encryption with Associated Data, ensuring both confidentiality and authenticity.
- **FROST**: Flexible Round-Optimized Schnorr Threshold signatures for creating a valid signature without reassembling the key.
- **Shamir Secret Sharing**: A cryptographic algorithm distributing a secret into parts such that a specified quorum is required to reconstruct it.
- **TPM**: Trusted Platform Module, a dedicated microcontroller designed to secure hardware through integrated cryptographic keys.
- **Secure Enclave**: A hardware-based key manager isolated from the main processor.
- **seccomp-bpf**: A Linux kernel feature filtering the system calls a process can make.
- **Privilege separation**: Dividing a program into parts to limit the privileges of the most exposed components.
- **SPIFFE**: Secure Production Identity Framework for Everyone, providing a standardized identity control plane.
- **mTLS**: Mutual TLS, where both parties authenticate each other via certificates.
- **pidfd**: A file descriptor that refers to a specific process, preventing PID reuse races.
- **WORM storage**: Write Once Read Many storage, an append-only system preventing data modification or deletion.
- **Hash chain**: A sequence of records where each is cryptographically linked to the previous one.
- **Wire injection**: Injecting credentials directly into network traffic (e.g., via proxy headers) without exposing them to the process.
- **Quorum**: The minimum number of independent approvals required to perform a critical operation.
- **Egress scanning**: Inspecting outbound traffic for credentials or sensitive patterns before it leaves the boundary.
