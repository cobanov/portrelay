# PortRelay account service

MIT-licensed GitHub OAuth sign-in and computer registry. The same Worker runs
with Cloudflare D1 or a Node 24+ SQLite adapter. Device data stays on encrypted
peer connections and never passes through this service.

## Cloudflare deployment

Create your own D1 database, change the database ID/domain in `wrangler.toml`,
then apply `migrations/0001_accounts.sql` with Wrangler D1 migrations. Set a custom
HTTPS domain with a valid certificate. Register a GitHub App with **no repository,
organization or account permissions**, no events and inactive webhooks. Its
callback is `https://YOUR-DOMAIN/callback/github`. Do not enable wildcard callbacks.

Set `PUBLIC_URL` to that HTTPS origin and save `GITHUB_CLIENT_ID` and
`GITHUB_CLIENT_SECRET` using `wrangler secret put`. Deploy `worker.mjs` using the
configuration. `/health` reports whether the provider is configured, never secrets.
`scripts/register-github-app.py` is the optional loopback manifest setup assistant
for the project's hosted domain. Review and adapt its public URLs before using it
for your own deployment. Secrets pass from GitHub to Wrangler stdin without logs.

## Self-host without Cloudflare

Use Node **24 or later**. There are no npm dependencies.

```sh
export PUBLIC_URL=https://portrelay.example.com
export DATABASE_PATH=/var/lib/portrelay/accounts.sqlite
# Supply GITHUB_CLIENT_ID and GITHUB_CLIENT_SECRET through your secret manager.
node account/server.mjs
```

Proxy HTTPS to `127.0.0.1:8787`. Keep the database directory private and back up the
SQLite database including WAL safely. The server applies migrations once. Host,
port and database path can be set with `BIND`, `PORT`, and `DATABASE_PATH`.
Self-host request limits use the immediate TCP peer, ignoring untrusted forwarded
IP headers. Add per-client limits at your trusted reverse proxy if needed.

Set `PORTRELAY_ACCOUNT_SERVER=https://portrelay.example.com` in the desktop agent's
service environment before starting it. Each account registration pins its server.
HTTPS is required except for explicit loopback development fixtures.

## Security and validation

Enrollment requires a one-use challenge signed by the computer's Ed25519/iroh
identity. The signed payload binds server origin, token hash, identity, address,
name and platform. The computer generates and keeps its bearer credential locally;
the database stores only its SHA-256 hash. GitHub numeric user IDs define accounts.
OAuth state, HttpOnly cookies and S256 PKCE bind the browser login. A separate
CSRF-protected confirmation shows exactly which computer will be added.

Membership is checked every 15 seconds. The agent fails closed after 90 seconds
without a successful membership check. Sign-out and remote removal cancel managed
trust and associated sessions. Manual pairing and device/input permissions remain
separate. This registry is a trusted membership authority, not a zero-trust proof
against a compromised registry administrator. Secure its deployment and credentials.

```sh
npm test --prefix account
python3 tests/account-flow.py target/debug/portrelay
```

Tests use real SQLite, WebCrypto and Rust agents. GitHub HTTP responses are mocked
in the automated suite. `tests/fixture-server.mjs` is loopback-only test code and
must not be deployed. Live GitHub acceptance is recorded separately in validation.

GitHub protocol references:
[web authorization](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps),
[GitHub App manifest](https://docs.github.com/en/apps/sharing-github-apps/registering-a-github-app-from-a-manifest).
