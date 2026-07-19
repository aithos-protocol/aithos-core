# Security policy

## Supported versions

Until the first stable release, only the latest published pre-release is
supported. Draft wire profiles remain verifiable where the specification says
so, but they may not receive feature backports.

## Reporting a vulnerability

Do not open a public issue for vulnerabilities involving private-key handling,
authorization bypass, signature or replay verification, vault isolation,
provider authentication, relay routing, or secret exposure.

Use GitHub's private vulnerability reporting for the
`aithos-protocol/aithos-core` repository. Include:

- affected package, version, and wire profile;
- a minimal reproducer or conformance vector;
- expected and observed behavior;
- realistic impact and any known exploitation;
- whether public disclosure is already planned.

Innoestate Holdings will acknowledge a complete report, coordinate validation
and remediation, and credit reporters who request attribution. Response-time
commitments will be added when the public security channel is enabled.
