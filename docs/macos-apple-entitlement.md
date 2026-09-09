# Apple USB host-controller entitlement request draft

**Draft only. Not submitted.** The Apple Developer account holder must review
this request, fill in their Team ID and contact details, and submit it through
Feedback Assistant. Do not post account credentials in an issue or this file.

Apple Developer Technical Support describes this as a managed entitlement and
provides this submission route in its
[USB host-controller entitlement response](https://developer.apple.com/forums/thread/802495).
This request does not guarantee approval or a delivery date.

- OS: macOS
- Title: Request for Entitlement - com.apple.developer.usb.host-controller-interface
- Problem Area: USB
- Type: Other Bug
- Team ID: [your Apple Developer Team ID]
- Product: PortRelay
- Product website: https://portrelay.cobanov.dev
- Open-source repository: https://github.com/cobanov/portrelay
- Proposed app identifier: dev.cobanov.portrelay

## Proposed description

We are developing PortRelay, a free, MIT-licensed application that lets a user
share selected USB devices between their own trusted computers over a local
network or an encrypted internet connection. Linux and Windows backends exist;
a limited macOS USB export preview is in development.

We request access to com.apple.developer.usb.host-controller-interface to build
the separate macOS receiving role. The intended behavior is to represent a
remote USB device, explicitly selected by the user, as a native USB device on
their Mac so existing applications and drivers can use it. USB/IP transfer
messages are carried inside an authenticated, encrypted QUIC connection; no
raw USB/IP server is exposed to the network.

Each computer has its own identity. A single-use invitation and explicit owner
approval establish trust. Pairing alone does not reveal or authorize devices.
The owner grants access per device and per peer, only one receiver can own a
session, and either user can disconnect. Revocation terminates the active
stream. Hotplug generation checks prevent new hardware from inheriting an old
permission. Device operations run outside the user interface, with bounded
requests and explicit cleanup on process or connection loss.

Our intended distribution is a Developer ID-signed, notarized macOS
application. We do not want users to disable System Integrity Protection or
other platform security features. We are not requesting App Store distribution
in this initial request. We do not claim that our current macOS build implements
a virtual host controller; this entitlement would allow development and
validation of that receiving backend through Apple's supported route.

Please advise on eligibility, development and distribution provisioning, any
additional entitlement requirements, supported macOS versions, and the
appropriate API documentation or sample code for this use case. We can provide
an architecture review, the public source, and a focused test build.
