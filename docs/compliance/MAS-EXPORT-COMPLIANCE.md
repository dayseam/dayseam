# Mac App Store export compliance (**MAS-7b**)

Normative record for **US export classification** assumptions and **App Store Connect** answers that apply to the **Mac App Store** SKU of Dayseam. It satisfies the **MAS-7b** plan row and must stay aligned with whatever **MAS-8d** (automated Connect upload) asserts in metadata or workflow comments.

**Related:** [`docs/plan/2026-phase-5-mas-app-store.md`](../plan/2026-phase-5-mas-app-store.md) (Block **MAS-7**, **MAS-8d**), [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md) §3 (**Store metadata** column), §22–§23, [`MAS-JIT-ENTITLEMENTS.md`](MAS-JIT-ENTITLEMENTS.md) (JIT / executable memory — separate from export classification), [`MAS-APP-REVIEW-NOTES.md`](MAS-APP-REVIEW-NOTES.md) (**MAS-7c** — App Review paste pack).

**Apple (official):** [Complying with encryption export regulations](https://developer.apple.com/documentation/security/complying-with-encryption-export-regulations) — use alongside this file when answering Connect; questionnaire wording may differ from BIS terms below.

**Disclaimer:** This is an engineering operating document, not legal advice. If product scope changes (new crypto, VPN, region-specific rules), refresh this file and have counsel review before the next store submission.

---

## Cryptography actually shipped in Dayseam

| Surface | Mechanism | Notes |
|---------|-------------|--------|
| **HTTPS to SaaS / self-hosted APIs** | **TLS 1.2+** via **`rustls`** (through **`reqwest`**) to user-configured hosts (GitLab, GitHub, Jira, Confluence, Microsoft identity endpoints, etc.) | Standard **client** TLS for authentication and confidentiality of API traffic. **MAS-6a** / **MAS-6b** document entitlements; no custom cipher suites. |
| **WKWebView / frontend** | System **WebKit** TLS stack for **HTTPS** loads the embedded UI initiates | Production UI is shipped assets + `tauri://` / IPC; any **outbound HTTPS** from the WebView uses the **same platform TLS** class as connector traffic (e.g. dev-server URLs during development). |
| **OAuth 2 / PKCE** | TLS to identity providers; **PKCE** verifier/challenge as required by public clients | No proprietary token crypto beyond what TLS + OAuth specs define. |
| **SQLite (`state.db`)** | **Not** encrypted at rest by Dayseam | Plain SQLite files under the app’s Application Support layout (**MAS-5b1**). |
| **macOS Keychain** | **OS APIs** (`keyring` crate) for PATs / OAuth secrets | Storage and access control are **system** services; Dayseam does not implement its own keystore encryption layer. |
| **Direct SKU only: in-app updater** | **Ed25519** signature verification over the shipped update payload (**minisign** / Tauri updater) | **MAS-3:** updater plugin is **not** registered on **`--features mas`**; MAS users receive updates through the **App Store**, not this path. Crates may still be linked for the direct SKU; verification is **authentication / integrity** on artifacts fetched over **HTTPS**, not a separate user-data encryption product. |

Dayseam does **not** ship: custom block ciphers, VPN or Tor clients, DRM systems, a separate end-to-end encryption layer for arbitrary user files (disk is plain SQLite + filesystem under the sandbox), or classified cryptography.

---

## Regulatory framing (operator summary)

For **US EAR** purposes, Dayseam’s use of cryptography is intended to fall under **License Exception ENC** (**15 CFR §740.17**) — in practice the **mass-market** and **eligible** encryption commodity paths consumer apps use when they rely on **standard** TLS (here **`rustls`**) and **platform** crypto (WebKit, Keychain), not proprietary ciphers. App Store Connect’s questions use Apple’s own wording; map answers back to the **Cryptography actually shipped** table above, not to this paragraph verbatim.

**EU / other jurisdictions:** App Store Connect may ask region-specific questions; answers should remain consistent with the inventory above. Revisit if we add regulated features (e.g. custom tunneling).

---

## App Store Connect — answers to mirror (**MAS-8d** contract)

When filing manually **today** or when **MAS-8d** automates upload, **use the same story** Connect asks for:

1. **Does the app use encryption?** **Yes** — TLS for **HTTPS** client traffic (`reqwest` + **`rustls`**); credentials stored via **macOS Keychain** APIs (OS-provided protection, not a custom Dayseam crypto layer).
2. **Is the app eligible for any exemptions?** **Yes** — encryption limited to **exempt** categories (authentication; confidentiality of communications; digital signature / integrity checks as used by TLS and standard protocols), **not** exempted **solely** because it is “public domain.”
3. **ERN / CCATS:** Not expected for this profile **unless** Apple or counsel instructs otherwise after a material change (custom crypto, new regulated market, etc.).
4. **Documentation URL / notes:** Point reviewers at this file in the open-source tree (release tag) or paste the **Cryptography actually shipped** summary into private App Review notes if required.

**MAS-8d** implementation checklist (for whoever wires upload):

- [ ] Upload job (or runbook) **links or quotes** this document in the PR / workflow README so metadata drift is reviewable.
- [ ] If Connect exposes **`ITSAppUsesNonExemptEncryption`** (or successor fields) in API payloads, set values consistent with **exempt-only** use; if the binary later gains **non-exempt** crypto, **stop** and update this doc first.
- [ ] Keep **TestFlight** / production answers identical unless the **binary** changed.

---

## Optional Info.plist note

Apple’s guidance allows declaring **`ITSAppUsesNonExemptEncryption` = `false`** when the app uses **only** exempt encryption. Dayseam has not added that key globally yet; if App Store Connect still prompts on every build after **MAS-8d**, evaluate adding it via Tauri **`bundle.macOS`** Info.plist merge **in a dedicated change** once legal agrees the inventory above is complete.

---

## Maintenance

When **adding** a new TLS stack, VPN/tunnel, disk-encryption-at-rest product, DRM, or any **non-standard** cryptographic primitive:

1. Update the **Cryptography actually shipped** table in this file.  
2. Update [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md) §3 / §16 cross-links if the **Store metadata** story changes.  
3. Re-run the **MAS-8d** metadata alignment checklist before the next upload.
