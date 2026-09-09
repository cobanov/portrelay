# ADR 0006: Account sign-in and automatic computer membership

Date: 2026-09-09. Status: implemented in alpha.8 / Mac preview.3.

Invitations made the main two-computer setup too difficult. The normal flow is
now install, sign in with GitHub on each computer, and select a registered peer.
The previous invitation workflow remains an advanced offline alternative.

Use a small open-source HTTPS registry, deployed as a Cloudflare Worker/D1 service.
The same handler can be self-hosted with Node 24 and SQLite. The first provider is
GitHub, through a GitHub App with no requested repository permissions. Google is
not implemented. No GitHub access token is stored; stable numeric GitHub IDs define
membership, not mutable names or email addresses.

Each agent proves possession of its existing Ed25519/iroh identity using a signed
one-use challenge. The signed enrollment binds the service origin, computer name,
OS, complete connection address and a hash of a locally generated bearer credential.
OAuth state, HttpOnly cookies, S256 PKCE and an explicit CSRF-protected computer
confirmation bind the browser flow. Credentials stay in the protected user state.
The registry stores only credential hashes and discovery metadata.

Account peers become trusted automatically, but neither USB grants nor keyboard /
mouse receiving permission is granted by account registration. Existing explicit
grants survive migration of the same already-paired identity into an account peer.
All native QUIC device and input traffic keeps its existing authenticated transport.

Membership sync runs every 15 seconds, with faster polling during initial sign-in.
Removal withdraws the deleted computer's credential and peer trust. The agent uses
a monotonic 90-second membership lease; without fresh membership it pauses account
trust and cancels associated sessions. A restart revalidates before trusting cached
account peers. The account service is consequently an availability dependency for
account-managed connections. Independent manual pairing remains available offline.

The registry is a trusted membership authority; compromise of its deployment can
alter membership. It is not a zero-trust replacement for a reviewed end-to-end
account-key synchronization design. Data transport and backend privilege isolation
remain separate. Account discovery does not deploy or imply a public data relay.

See [account protocol and self-hosting](../account/README.md),
[user flow and stored data](accounts.md), and [validation](validation.md).
