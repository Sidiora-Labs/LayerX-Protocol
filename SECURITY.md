# Security Policy

LayerX handles authorization, accounting, custody commitments, and emergency
exit evidence. Treat any defect that can affect value conservation, authority,
finality, data availability, replay determinism, withdrawal uniqueness, upgrade
safety, or private key handling as a security issue.

## Supported versions

There is no production-certified public release. The development branch receives
security fixes, but availability of source or a passing test suite is not a
deployment recommendation.

## Report privately

Do not open a public issue, discussion, or pull request for a suspected
vulnerability. Use GitHub's private vulnerability reporting flow from the
repository's **Security** tab. Include:

- the affected commit or source revision and component;
- prerequisites and an end-to-end reproduction;
- the violated invariant and plausible impact;
- whether exploitation requires sequencer, guarantor, governance, or custody
  privileges;
- any proposed mitigation, test vector, or proof obligation.

Do not probe public infrastructure, validators, custody deployments, user data,
or third-party systems. Use only systems and assets you own or have explicit
written authorization to test. Do not move funds, publish exploit details, or
retain secrets obtained during research.

The maintainers will validate scope and coordinate remediation and disclosure
through the private report. No bounty, safe-harbor expansion, or response-time
commitment exists unless separately announced in writing.

## Security boundaries

The protocol's complete assumptions and exclusions are normative in the
[threat model](spec/layerx-protocol/docs/threat-model.md). In particular:

- guarantor threshold signatures are economic attestations, not validity proofs;
- the sequencer is a liveness and short-horizon ordering trust role;
- emergency exit depends on the last finalized checkpoint and Paxeer contract
  availability;
- local, sanitizer, fuzz, replay, and proof results do not replace independent
  contract review or a controlled deployment process.

