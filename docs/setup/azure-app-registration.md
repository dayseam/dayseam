# Azure app registration for Outlook Calendar (DAY-201)

Dayseam's Outlook Calendar connector reaches the Microsoft Graph API
through OAuth 2.0 with PKCE + a loopback redirect — the standard
"native app" flow. Microsoft's IdP will not issue tokens until an
**Azure app registration** exists in a tenant and its `client_id`
has been passed to the Tauri shell at runtime (or baked into a
release build at bundle time).

This document is the reproducible recipe for producing that
`client_id`. It is written to survive Microsoft's inevitable portal
UI reshuffles: every step names the target resource rather than the
exact menu label, so "the page has moved" is still actionable.

## What you are creating

You are creating one app registration of the kind Microsoft calls
*public client* — no secret, no certificate. The registration tells
the Microsoft identity platform two things:

1. *We are a native app that catches the OAuth callback on a fixed
   loopback port.* Microsoft runs two distinct OAuth surfaces behind
   the `/common/` endpoint: Entra ID (work / school) tenants honour
   the RFC 8252 loopback wildcard rule — registering
   `http://127.0.0.1` (no port, no path) lets the IdP accept any
   port + any path coming back. Personal Microsoft accounts
   (`@outlook.com`, `@hotmail.com`, `@live.com`) are routed to the
   legacy MSA endpoint at `login.live.com`, which does **not**
   honour that wildcard — every redirect URI must match a registered
   reply URL byte-for-byte, including the port and path. To work
   for both account families, Dayseam binds its loopback listener
   to a fixed port (`53691`) and registers exact URIs for it
   alongside the wildcard hosts. Both `http://127.0.0.1:53691/oauth/callback`
   and the host-only `http://127.0.0.1` reply URLs are listed below
   so an Entra-only deployment still works without the fixed-port
   entry, and an MSA login still works on the strict endpoint.

2. *These are the Graph scopes we expect to request.* We declare the
   delegated scopes up front so a user consenting to a Dayseam login
   sees exactly the permissions the app will ever ask for.
   Over-declaring the scope set produces a consent screen that reads
   "read your calendars, read your mail, ..." — under-declaring
   means the token grant succeeds but Graph returns 403 on the first
   calendar read.

## Prerequisites

- An Azure tenant you can administer. Any of the following works:
  - **Azure Free Account**. The sign-up prompts for a credit card
    but the app-registration portal is in the free tier.
  - **Work / school tenant**. If your employer uses Microsoft 365
    and you have at least the "Application Developer" role (or a
    peer IT admin willing to create a registration on your behalf),
    you can register the app there.
  - **Microsoft 365 Developer Program** sandbox tenant, if you
    qualify. Eligibility tightened in 2024; MSA-only accounts are
    no longer accepted automatically.
- A place to store the resulting `client_id`. The local-dev path is
  the shell environment variable `DAYSEAM_MS_CLIENT_ID` (read at
  every `oauth_begin_login` call); the production release path is
  the same variable set in CI before `cargo tauri build`.

## Step-by-step

### 1. Sign in to the Entra ID portal

Open `https://entra.microsoft.com` and sign in with an account that
belongs to the tenant you want to host the registration. Confirm
the top-right tenant switcher shows the tenant you expect — a
common mistake is registering in the "Microsoft Services" guest
tenant instead of your own.

### 2. Create a new app registration

Navigate to **Applications** → **App registrations** → **New
registration**. Fill the form as follows:

| Field                             | Value                                              |
| --------------------------------- | -------------------------------------------------- |
| Name                              | `Dayseam` (or `Dayseam (dev)` for a dev-only tenant) |
| Supported account types           | *Accounts in any organizational directory (any Entra tenant — multi-tenant) and personal Microsoft accounts* |
| Redirect URI → Platform           | *Mobile and desktop applications*                  |
| Redirect URI → Value              | `http://127.0.0.1:53691/oauth/callback`            |

"Mobile and desktop applications" is the platform type that lets
the registration accept loopback-bound reply URLs at all (the
*Web* platform would reject every URL Dayseam constructs at
runtime, regardless of port). The exact `http://127.0.0.1:53691/oauth/callback`
URL is what Microsoft's legacy MSA endpoint matches against
byte-for-byte; the `:53691` is Dayseam's fixed loopback port (see
`apps/desktop/src-tauri/src/oauth_config.rs`'s `MICROSOFT_LOOPBACK_PORT`)
and the `/oauth/callback` path is the listener's route. After
registration, add the two extra reply URLs in step 5 to cover
Entra-only deployments + a localhost-host fallback.

Click **Register**. You land on the new app's **Overview** page.
Copy the **Application (client) ID** — that is the value you will
export as `DAYSEAM_MS_CLIENT_ID`. It is a v4 UUID and looks like
`11111111-2222-3333-4444-555555555555`.

### 3. Configure the API permissions

Navigate to **API permissions** → **Add a permission** → **Microsoft
Graph** → **Delegated permissions**. Add:

- `User.Read` — lets the "Connected as \<upn\>" ribbon on the Add
  Source dialog identify which account just signed in.
- `Calendars.Read` — the connector's core permission. DAY-202
  narrows this per-walker if ever needed; the registration declares
  the union.
- `offline_access` — lets Microsoft return a refresh token so the
  connector can stay logged in across app restarts without
  re-prompting the user.

Click **Add permissions**. The permission rows should show state
"Not granted for \<tenant\>" — that is normal for delegated scopes:
each user grants them at consent time.

**Do not** grant admin consent unless you are registering in a work
tenant where admin consent is the agreed policy. The PKCE flow
asks the user to consent interactively; forcing admin consent adds
an extra IT-ticket step for no security benefit on a single-user
desktop install.

### 4. (Optional) configure branding + owners

- **Branding & properties** → upload `apps/desktop/icons/icon.png`
  as the application logo. The consent screen will show it next to
  the Dayseam name. Skip for dev tenants.
- **Owners** — add every maintainer who should be able to rotate
  the registration (e.g. bump redirect URIs if we ever switch off
  loopback). One owner is not enough for a production registration.

### 5. Confirm public-client settings and add the wildcard reply URLs

Navigate to **Authentication**. Verify:

- **Platform configurations** shows *Mobile and desktop
  applications* with the **three** reply URLs below in its list:
  - `http://127.0.0.1:53691/oauth/callback` — exact match required
    by Microsoft's legacy MSA endpoint (`login.live.com`) for
    personal Microsoft accounts.
  - `http://127.0.0.1` — host-only entry that activates the RFC
    8252 loopback wildcard for Entra ID work / school accounts;
    Microsoft accepts any port + path against this base.
  - `http://localhost` — the same wildcard as above on the
    `localhost` host. Useful when a corporate proxy or VPN
    intercepts `127.0.0.1` resolution; harmless otherwise.
  Use the **Add URI** link inside the platform card to register
  any that are missing, then **Save** at the top of the page.
- **Advanced settings → Allow public client flows** is set to
  **Yes**. This is the toggle that lets a PKCE grant complete
  without a client secret; Microsoft sometimes flips it off when
  the "Mobile and desktop applications" platform is not
  registered.
- **Supported account types** matches what you chose in step 2.

Save if you changed anything.

> **Why three URLs?** The single `http://127.0.0.1:53691/oauth/callback`
> is sufficient for personal Microsoft accounts (and the only one
> that works for them, because their endpoint disables wildcard
> matching). The two host-only URLs are the wildcard fallback for
> Entra ID accounts — they let local dev / debugging on a different
> port still complete a flow without a docs update. If you only
> serve corporate Entra users, you can omit the `:53691` entry; if
> you only serve personal accounts, you can omit the wildcard
> entries. Listing all three is the union that "just works" for
> both.

## Using the `client_id`

### Local development

Export the id and (re)launch the Tauri dev shell:

```bash
export DAYSEAM_MS_CLIENT_ID=11111111-2222-3333-4444-555555555555
pnpm tauri dev
```

The runtime resolver reads `DAYSEAM_MS_CLIENT_ID` on every
`oauth_begin_login` call, so you can change the value and re-trigger
the login without restarting the app — handy for bouncing between
a dev and a work registration.

### CI / release builds

Dayseam's release workflow sets `DAYSEAM_MS_CLIENT_ID` as a GitHub
Actions secret and passes it through as an environment variable
ahead of `cargo tauri build`. The Rust
`oauth_config::resolve_microsoft_client_id` function falls back to
`option_env!("DAYSEAM_MS_CLIENT_ID")` — a compile-time read — when
the runtime variable is unset, so the shipped binary still carries
a valid id for users who have not configured their shell.

If both layers fall back to the sentinel `UNSET-DAYSEAM-CLIENT-ID`,
`oauth_begin_login` returns `DayseamError::InvalidConfig` with code
`oauth.login.not_configured` before a browser tab ever opens. The
Add Outlook Source dialog renders the error with a pointer back to
this document.

## Troubleshooting

| Symptom                                                         | Likely cause                                                                                                   |
| --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Consent screen shows *"This app is trying to access..."* but every scope is missing from the list | Delegated permissions weren't saved — re-check step 3 and click **Add permissions**.                           |
| Callback page shows *"AADSTS50011: The reply URL specified... does not match"* | Either the **Mobile and desktop applications** platform is missing or its reply URL list is missing `http://127.0.0.1:53691/oauth/callback`. Add the three URLs from step 5. |
| Callback page shows *"AADSTS900971: No reply address provided"* (only for personal Microsoft accounts) | The legacy MSA endpoint (`login.live.com`) cannot match Dayseam's redirect URI byte-for-byte. Confirm `http://127.0.0.1:53691/oauth/callback` is in the reply URL list (step 5). The host-only `http://127.0.0.1` and `http://localhost` entries do **not** apply on this endpoint. |
| Callback page shows *"unauthorized_client"*                     | **Allow public client flows** is off. Toggle it on in step 5.                                                  |
| Dayseam dialog shows `oauth.login.loopback_bind_failed` referencing port `53691` | Another process already binds the loopback port — usually a second Dayseam window mid-login. The error message includes an `lsof -i tcp:53691` hint to find the offender. Close the conflicting process and retry. |
| Dayseam dialog shows `oauth.login.not_configured`               | `DAYSEAM_MS_CLIENT_ID` is unset or equals the placeholder. Export it before launching the app.                |
| Dayseam dialog shows `oauth.login.state_mismatch` immediately   | The browser hit a stale callback URL from an earlier flow. Re-open the dialog and retry; each begin_login mints a fresh state nonce. |
| Token endpoint returns *"invalid_grant"*                        | The PKCE verifier didn't match the challenge; happens if the app was restarted mid-consent. Retry from the start of the dialog. |

## Rotating the `client_id`

Rotating is not a rush job — the registration has no client secret
that could leak, and the same redirect URI registration applies
regardless. Do it when:

- The registration's owner account is departing and you want a
  cleaner audit trail.
- You are migrating from a personal sandbox tenant to a production
  tenant (most common — the dev id was only ever meant for local
  boots).

To rotate:

1. Create the new registration following steps 1–5.
2. Update the `DAYSEAM_MS_CLIENT_ID` secret in CI.
3. Trigger a release build.
4. On users' machines, the next app launch carries the new id. No
   re-consent is required automatically — a user's first sign-in
   against the new id will re-consent, which is the intended UX
   because the registration is a new trust boundary.
5. Delete the old registration **after** every in-flight release
   carrying the old id has aged out.
