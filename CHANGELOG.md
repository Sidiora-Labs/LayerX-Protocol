# Changelog

All notable repository and protocol changes will be recorded here. Protocol
compatibility is defined by the normative KVX specification, canonical wire
encoding, result codes, and versioned transition functions—not by this summary.

## Unreleased

### Added

- Public repository hygiene, contribution, security, support, qualification,
  and continuous-integration documentation.
- A source-available notice that grants no deployment or redistribution rights.
- A root license notice documenting that the development-phase source-available
  terms are temporary and that an open-source release is planned after completion.
- A reproducible repository audit for generated-spec freshness, secret-shaped
  material, private workspace references, large artifacts, and shell syntax.

### Security

- Removed a live validator address and private workspace paths from material
  intended for publication while preserving the production-mutation gate.
