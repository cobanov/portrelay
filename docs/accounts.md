# Sign in and choose a computer

1. Install PortRelay and choose **Sign in with GitHub**.
2. Check the computer name, then choose **Add computer** on the sign-in page.
3. Sign in with the same GitHub account on your other computers. They appear
   under **Your computers** automatically, without invitations or approval codes.

The first provider is GitHub. Google sign-in is not implemented. No repository,
organization, email, or write permissions are requested. There is no subscription.

For a computer with only SSH access:

```sh
portrelay login --no-open
```

Open the printed link on your phone or laptop and approve the **remote computer's
name**. The agent on that computer completes registration. Then use
`portrelay computers`, `portrelay menu`, `portrelay share`, and `portrelay connect`.
`portrelay logout` signs this computer out.

## Select what to use

Account registration makes computers discoverable. Devices stay private until
**Share device** is selected. Linux keyboard/mouse receiving still needs
**Enable receiving control**, then **Allow keyboard & mouse** for the selected
computer. On the sending computer choose **Control [name]**; **Esc** returns.

**Remove computer** removes the selected computer from your account, signs it
out, withdraws its permissions, and closes its connections on the next membership
check, normally within 15 seconds. Eject borrowed disks before removal or logout.
If the account service cannot be reached for 90 seconds, account-managed trust
pauses and active connections close. A restarted agent requires a fresh check.

Account discovery does not provide an internet data relay. Device traffic still
uses encrypted peer connections: a reachable LAN/VPN such as Tailscale, or the
existing explicitly configured QUIC relay. Account sign-in needs internet access.
See [network configuration](linux-alpha.md).

**Advanced: pair without an account** retains the direct invitation workflow for
independent or offline use. Those manually paired computers are managed locally.

## What the service stores

The public GitHub numeric identity and username, computer names, OS names, public
cryptographic identities, connection addresses, registration/last-seen times, and
hashed per-computer credentials. Connection addresses may include LAN, VPN, public
IP, or relay addresses. Your computer's private key stays on that computer.

GitHub access tokens are used only to read the profile during sign-in and are not
stored. The service never receives USB traffic, keystrokes, mouse input, files,
clipboard content, or screen video. Removing a computer deletes its registration;
account identity metadata remains. Sign-in links and temporary browser state
expire after ten minutes. No third-party analytics are included.

The hosted registry is **https://portrelay-account.cobanov.dev**. Its source and
self-hosting instructions are in [account/](../account/README.md).
