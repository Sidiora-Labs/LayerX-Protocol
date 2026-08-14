# LayerX Canonical Wire Format (LXC/1)

Status: normative. Version 1. Protocol tag `LXP1`. Binding on the C17 reference implementation in this
repository; the codec is `src/codec/lxp_codec.c`, with public interfaces in `include/layerx/lxp_codec.h`.

The canonical binary form **is** the protocol. An optional JSON/HTTP gateway may exist for convenience,
but MUST NOT define consensus behaviour: it re-encodes into LXC/1, and only the LXC/1 bytes are signed,
hashed, logged, checkpointed and replayed. Two implementations that disagree about a byte disagree
about the protocol.

## 1. Foundational rules

- **R1 — One valid encoding.** For every logical value there is exactly one accepted byte string. Any
  other byte string a permissive parser might map to the same value MUST be rejected.
- **R2 — Schema-driven, not self-describing.** LXC/1 carries no type tags; fields are encoded
  positionally per the schema for the declared `(protocol_version, structure)`. That removes the
  tag-ordering ambiguities which make CBOR/Protobuf canonicalization fragile.
- **R3 — Validate, never normalize.** The decoder MUST reject non-canonical input. It MUST NOT repair,
  reorder, pad, truncate, trim, case-fold or Unicode-normalize anything. A normalizing decoder silently
  creates two encodings of one value and breaks R1.
- **R4 — No trailing bytes.** After decoding a top-level structure the input MUST be exactly consumed;
  one surplus byte is a decode failure.
- **R5 — Bounds before allocation.** Every length is checked against the remaining input and the
  applicable limit (§9) *before* memory is reserved.
- **R6 — Round-trip assertion.** Debug, test and fuzz builds MUST assert `encode(decode(b)) == b` for
  every accepted structure. A round-trip failure is a codec defect, not a test artefact.

## 2. Primitive types

### 2.1 Byte order and integers

All multi-byte integers are **unsigned, fixed-width, big-endian**. There are no signed integers, no
floating-point values, and no variable-width integers in any consensus structure except the `vlen`
length prefix.

| Type | Width | Used for |
|---|---|---|
| `u8` | 1 | closed enums, presence flags, kinds |
| `u16` | 2 | protocol version, activity type |
| `u32` | 4 | network id, counters, layout version |
| `u64` | 8 | sequences, millisecond timestamps, epochs |
| `u128` / `u256` | 16 / 32 | amounts, balances, fees, allowances / perps and funding accumulators |

Amounts are integer base units; decimal placement is asset metadata, never encoded alongside the amount.
Scalars are hashed at full fixed width — a `u128` of value 1 is fifteen zero bytes then `01`.
Minimal-width "big-int" scalars are forbidden: a second encoding path and a worse decoder.

### 2.2 `vlen` — the only variable-width encoding

`vlen` encodes lengths and element counts as unsigned LEB128: little-endian 7-bit groups, high bit =
continuation. Mandatory constraints:

- The encoding MUST be the shortest form of the value. A continuation chain whose final byte is `0x00`
  is non-canonical (the single byte `0x00`, encoding zero, is the sole exception).
- Maximum 4 bytes, range `0 .. 2^28-1`. A fifth continuation byte is a decode failure whatever the
  value.
- The decoded value MUST also satisfy the structure's own size limit (§9).

```
0 -> 00      1 -> 01      127 -> 7f      128 -> 80 01
153 -> 99 01              16383 -> ff 7f 16384 -> 80 80 01
rejected: 80 00 (non-minimal zero), 81 00 (non-minimal one), ff ff ff ff 00 (overlong)
```

C17 note: decode into `uint32_t` with an explicit shift counter. A shift of 28 or more on a 32-bit value
is undefined behaviour, so the 4-byte cap MUST be enforced by the loop bound, never by overflow.

### 2.3 Byte strings and fixed arrays

`bytes ::= vlen(n) || n raw bytes`. Content is opaque: the kernel never interprets, normalizes or
compares it except bytewise. Free-form human text — memos, deliverable descriptions, dispute statements
— travels as `bytes` and is never interpreted by consensus code. That is what keeps a Unicode library
out of the state machine.

Hashes, identifiers, keys and signatures are fixed-width with **no** length prefix: `hash32`, `did_ref`
(32), `account_id` (32), `asset_id` (32), `grant_id` (32), `key_id` (32), `idem_key` (16), `ed25519_pub`
(32), `ed25519_sig` (64), `secp256k1_pub` (33, compressed), `secp256k1_sig` (65, `r||s||v`, low-`s`
only).

### 2.4 Strings

Consensus-visible strings (`str`) are `vlen(n) || n bytes` restricted to printable ASCII `0x21..0x7E`:
no space, NUL, DEL or any byte `0x80..0xFF`; no leading or trailing separator characters; length ≥ 1,
because the empty string is expressed by an absent optional field and never by a zero-length `str`.

The restriction is deliberate: Unicode normalization differs across libraries and versions, and a
normalization difference between two nodes is a consensus fork. Only DID strings, asset symbols and
governance parameter names use `str`; everything else is a hash or opaque `bytes`.

### 2.5 Booleans, enums, optionals

- `bool` is `u8` with **only** `0x00` and `0x01` valid; `0x02..0xFF` is a decode failure.
- Enums are `u8` or `u16` over a **closed** value set declared per version. Unknown values are rejected:
  there is no unknown-enum passthrough.
- Optionals are a `u8` presence byte (`0x00`/`0x01`) followed by the value iff presence is `0x01`.
  Absent means the bytes are *not there*; encoding a zero-filled placeholder violates R1. Where presence
  is implied by another field — `authority.ref` is present exactly when `authority.kind != 0x00` — no
  separate presence byte is used and the implication is normative.

### 2.6 Arrays, maps and sets

```
array<T> ::= vlen(n) || T[0] || ... || T[n-1]        /* semantic order preserved */
map<K,V> ::= vlen(n) || (K,V)[0] || ... || (K,V)[n-1]
set<K>   ::= vlen(n) || K[0] || ... || K[n-1]
```

Array order is meaning (transfer legs are authorized in order). Map and set entries MUST be in
**strictly ascending bytewise lexicographic order of the encoded key**: comparison is `memcmp` over
encoded key bytes, and where keys differ in length the shorter is smaller when the common prefix is
equal. Equal keys are a decode failure — duplicates are never merged and never last-wins. Non-ascending
order is a decode failure even when all keys are distinct.

The decoder MUST verify ordering in its single pass by retaining the previous key, and MUST NOT sort —
sorting on decode reintroduces multiple encodings of one value.

## 3. Domain separation

Every hash the protocol computes is domain-separated. For an ASCII tag `T` (1..31 bytes) and body `B`:

```
H(T, B) = SHA-256( u8(len(T)) || T || B )
```

The tag length prefix makes the construction injective — no tag can be a prefix of another tag plus a
body. Bare `SHA-256` over structure bytes is forbidden protocol-wide, and every tag starts with `LXP1/`,
so a version bump changes every preimage and no digest can be carried across versions.

| Tag | Body | Produces |
|---|---|---|
| `LXP1/did` | DID `str` bytes | `did_ref` |
| `LXP1/account` | `u8 kind \|\| did_ref \|\| u8 label_len \|\| label` | `account_id` |
| `LXP1/asset` | `u64 chain_id \|\| 20B contract \|\| u8 sym_len \|\| symbol` | `asset_id` |
| `LXP1/session-key-id` / `-bind` | `u8 scheme \|\| pubkey` (`\|\| scope_bytes`) | `key_id`, binding digest |
| `LXP1/payload` | `u16 activity_type \|\| payload_bytes` | `payload_hash` |
| `LXP1/activity-sign` | envelope minus signature | activity signing preimage |
| `LXP1/activity-id` | complete envelope incl. signature | `activity_id` |
| `LXP1/transfer-set` | encoded transfer set | `transfer_set_root` |
| `LXP1/receipt` / `LXP1/event` | encoded receipt / event | receipt, event leaf digests |
| `LXP1/merkle-leaf` / `-node` / `-empty` | leaf bytes / `left\|\|right` / empty | Merkle leaf, node, empty root |
| `LXP1/state-key` | `u8 tree_id \|\| key_bytes` | 256-bit SMT path |
| `LXP1/state-value` / `-leaf` / `-node` | record / `path\|\|value_digest` / `left\|\|right` | SMT value, leaf, node |
| `LXP1/state-root` | `u32 layout_version \|\| tree roots, fixed order` | global `state_root` |
| `LXP1/batch-header` | encoded batch header | `batch_id` |
| `LXP1/batch-sign` | encoded batch header | sequencer signing preimage |
| `LXP1/grant` | encoded grant body | `grant_id`, payer signing preimage |
| `LXP1/oracle-obs` | encoded observation body | oracle signing preimage |
| `LXP1/oracle-root` | Merkle root over accepted observations | `oracle_root` |
| `LXP1/da-chunk` | availability chunk bytes | DA leaf |
| `LXP1/checkpoint` | encoded checkpoint body | `checkpoint_id` |
| `LXP1/guarantor-attest` | `checkpoint_id \|\| u8 attest_flags` | guarantor signing preimage |
| `LXP1/withdrawal-nullifier` | `u32 net \|\| account \|\| asset \|\| u128 amount \|\| u64 gseq` | withdrawal nullifier |
| `LXP1/deposit-nullifier` | `u64 src_chain \|\| 32B src_tx \|\| u32 log_index` | deposit nullifier |
| `LXP1/idempotency` | `did_ref \|\| idem_key` | idempotency tree key |

Adding a tag requires a spec change; reusing a tag for a new body shape is forbidden.

**Merkle trees.** Leaves and nodes use distinct tags, so no internal node can be forged from leaf data.
Trees are RFC-6962 style: for `n > 1` leaves split at the largest power of two below `n`, and **never
duplicate the last leaf** to pad — that admits the two-trees-one-root collision. The empty tree is
`H("LXP1/merkle-empty", "")`; a single-leaf root is that leaf's hash.

## 4. Signing pre-images

A signature is always over `u8(len(T)) || T || B` passed as the *message* to a pure scheme — never over
a pre-hashed digest or untagged structure bytes. Ed25519 (RFC 8032, pure) signs for agents, sequencer
and guarantors; secp256k1 for Paxeer-facing certificates. Ed25519 verification MUST reject non-canonical
`S` (`S >= L`) and small-order keys, secp256k1 MUST reject high-`s`: malleability would yield two
encodings of one authorized activity with different `activity_id`s, breaking R1.

**4.1 Activity** — `T = "LXP1/activity-sign"`:

```
B = magic || protocol_version || network_id || activity_type || actor_did
    || authority.kind || [authority.ref] || account_sequence || not_before_ms || not_after_ms
    || idem_present || [idempotency_key] || fee_limit || payload_hash
    || vlen(payload_len) || payload
```

That is the complete envelope minus the trailing `sig_scheme` and `signature`. The payload appears in
full *and* by hash; the decoder MUST verify `payload_hash == H("LXP1/payload", u16(activity_type)
|| payload)` before signature verification, so receipts and proofs may reference the payload by hash
alone. `activity_id = H("LXP1/activity-id", full envelope)` — computed over the bytes *including* the
signature, so the id commits to the exact authorization presented.

**4.2 Batch header** — `T = "LXP1/batch-sign"`:

```
B = protocol_version || network_id || epoch || batch_number || first_sequence || last_sequence
    || previous_state_root || resulting_state_root || activity_merkle_root || receipt_merkle_root
    || event_merkle_root || data_availability_root || oracle_root || timestamp_ms || sequencer_id
```

`batch_id = H("LXP1/batch-header", B)`. Headers chain by `previous_state_root`; two signed headers
sharing `(network_id, epoch, batch_number)` with different `batch_id`s are an equivocation proof.

**4.3 Grant** (payer authorization consumed by `RECEIVE`) — `T = "LXP1/grant"`:

```
B = protocol_version || network_id || grantor_did || grantee_did || from_account || asset
    || max_per_draw || total_allowance || u8 recurrence_kind || period_ms || per_period_allowance
    || not_before_ms || not_after_ms || purpose_hash || invoice_present || [invoice_id]
    || revocation_sequence
```

`grant_id = H("LXP1/grant", B)`, signed by the payer's authority. Bound in: the debited account, the
asset, both caps, the window, the purpose and the revocation sequence. A grant is a *bounded* authority,
and the kernel treats the on-state grant record — not the presented bytes — as the authority of record
(`threat-model.md` §4.11).

**4.4 Oracle observation** — `T = "LXP1/oracle-obs"`:

```
B = protocol_version || network_id || u8 source_kind || signer_key_id || market_id
    || observation_time_ms || observation_sequence || u128 price || u8 price_exp || u128 confidence
```

Binding `market_id` prevents replaying a price into another market; `observation_sequence` prevents
replaying an old price into a later batch. `price_exp` is a decimal exponent applied by integer
arithmetic only. Accepted observations commit under `LXP1/oracle-root` and become replayable history.

**4.5 Checkpoint certificate** — `T = "LXP1/checkpoint"`:

```
B = protocol_version || network_id || epoch || checkpoint_number || first_batch || last_batch
    || start_state_root || end_state_root || batch_merkle_root || data_availability_root
    || timestamp_ms || u32 guarantor_set_id
```

`checkpoint_id = H("LXP1/checkpoint", B)`. Each guarantor then signs `T = "LXP1/guarantor-attest"`, `B =
checkpoint_id || u8 attest_flags`, bit 0 = replayed every transition and matched every root, bit 1 =
stores the complete activity, receipt, oracle, state-diff and recovery data. A certificate is
`checkpoint_body || set<(guarantor_id, signature)>` per §2.6. Both flags MUST be set for a signature to
count toward threshold — signing without replaying is the slashable act, and the flags make that claim
explicit.

## 5. The activity envelope

Positional schema for `protocol_version = 1`:

| # | Field | Type | Notes |
|---|---|---|---|
| 0 | `magic` | 4B | ASCII `LXA1` |
| 1 | `protocol_version` | `u16` | must be a version the node implements |
| 2 | `network_id` | `u32` | replay-domain separator |
| 3 | `activity_type` | `u16` | high byte = module id, low byte = action |
| 4 | `actor_did` | `did_ref` | 32B |
| 5 | `authority.kind` | `u8` | `00` primary, `01` session key, `02` grant, `03` module capability |
| 6 | `authority.ref` | 32B | present iff kind ≠ `00` |
| 7 | `account_sequence` | `u64` | must equal the actor's `next_sequence` |
| 8,9 | `not_before_ms`, `not_after_ms` | `u64` | inclusive bounds vs batch timestamp; `after` ≥ `before` |
| 10,11 | `idem_present`, `idempotency_key` | `u8`, 16B | `00`/`01` only; key present iff `01` |
| 12 | `fee_limit` | `u128` | maximum fee the actor authorizes |
| 13 | `payload_hash` | 32B | `H("LXP1/payload", u16 type \|\| payload)` |
| 14 | `payload_len` | `vlen` | ≤ `LX_MAX_PAYLOAD` |
| 15 | `payload` | bytes | module-defined positional schema |
| 16 | `sig_scheme` | `u8` | `01` = ed25519 |
| 17 | `signature` | 64B | over the §4.1 preimage |

Module ids: `01` asset, `02` escrow, `03` budget, `04` stream, `05` service, `06` perps, `07`
governance, `08` bridge, `09` oracle, `0A` identity — so `asset.SEND` is `0x0101` and `asset.RECEIVE`
is `0x0102`.

The `service` module carries the complete agent work lifecycle — task commitments, tool execution
attestations, deliveries, acceptances and disputes — as first-class ordered and attested activities,
encoded like any other. They carry no direct monetary effect; any value they imply moves through
402LXP transfers.

## 6. Version tagging and forward compatibility

- `magic` identifies the structure family, `protocol_version` the schema; a node MUST reject a version
  it does not implement rather than guess. Every domain tag embeds `LXP1`, so bumping to `LXP2` changes
  every digest by construction and cross-version digest confusion is impossible.
- **There are no unknown-field skips and no extension points.** Unknown bytes are rejected. A permissive
  "ignore what you don't understand" rule would let two node versions accept the same bytes and compute
  different state.
- New fields require a new `protocol_version` and a new transition-function version. Old decoders and
  old transition functions stay in the binary forever, selected by the version recorded for the batch,
  so historical replay stays byte-identical across upgrades.
- Activation is a governance parameter effective at an epoch boundary; a batch MUST NOT mix versions the
  active version table does not jointly permit.
- The JSON gateway may add fields freely, but MUST fail closed if it cannot reproduce the exact
  canonical bytes the caller signed.

## 7. Rejection catalogue

| Code | Cause |
|---|---|
| `LX_DEC_BAD_MAGIC` | field 0 ≠ `LXA1` |
| `LX_DEC_BAD_VERSION` | unimplemented `protocol_version` |
| `LX_DEC_TRUNCATED` | a length exceeds the remaining input |
| `LX_DEC_TRAILING` | bytes remain after the structure |
| `LX_DEC_VARINT_NONMINIMAL` / `_OVERLONG` | `vlen` not shortest form / longer than 4 bytes |
| `LX_DEC_BOOL_RANGE` | boolean or presence byte not `00`/`01` |
| `LX_DEC_ENUM_UNKNOWN` | value outside a closed set |
| `LX_DEC_ORDER` | map/set keys not strictly ascending |
| `LX_DEC_DUPLICATE` | equal map/set keys |
| `LX_DEC_STR_CHARSET` | byte outside `0x21..0x7E` in a `str` |
| `LX_DEC_LIMIT` | size limit exceeded (§9) |
| `LX_DEC_DEPTH` | nesting deeper than `LX_MAX_DEPTH` |
| `LX_DEC_PAYLOAD_HASH` | `payload_hash` ≠ recomputed value |
| `LX_DEC_SIG_MALLEABLE` | non-canonical `S`, high-`s`, or small-order key |
| `LX_DEC_SIG_INVALID` | signature does not verify |

Every code is a hard failure with no state change, and a batch containing any activity that fails one
is an **invalid batch in its entirety** which guarantors MUST refuse to attest. Decode failure is never
a per-activity failure receipt: a receipt requires a well-formed activity to reference.

## 8. Worked example: an `asset.SEND` activity

Real values. Digests are SHA-256 under the tags above; the signature is a genuine Ed25519 signature over
the §4.1 preimage from seed `H("LXP1/example/seed", "alice-session-key-1")`. Total 386 bytes.

```
off   field             len  bytes
0000  magic               4  4c584131                                                          ; "LXA1"
0004  protocol_version    2  0001
0006  network_id          4  00000539                                                          ; 1337
000a  activity_type       2  0101                                                              ; asset.SEND
000c  actor_did          32  90573f30df195427696f0ead0f3a6b5e343fed24f86f8de4dfb4485826619c88  ; did:lx:alice
002c  authority.kind      1  01                                                                ; SESSION_KEY
002d  authority.ref      32  cc503c0c69403e1c09edf1761322dbf360b5c34f21d94972d5662b3412b3f25d  ; key_id
004d  account_sequence    8  000000000000002a                                                  ; 42
0055  not_before_ms       8  0000019b71b44c00                                                  ; 1767139200000
005d  not_after_ms        8  0000019b76daa800                                                  ; 1767225600000
0065  idem_present        1  01
0066  idempotency_key    16  bb60997fad932902a0aea5263135de57
0076  fee_limit          16  000000000000000000000000000007d0                                  ; 2000
0086  payload_hash       32  e7c6b4bd2eaa5254c6ec7ceb014f5d3600e628dd1758aba4c524a01389398cd0
00a6  payload_len         2  9901                                                              ; vlen(153)
      -- payload begins --
00a8  from               32  8c03d378e522004b394a8495124a257823f70d0eb4f6fcddc79f6f770c077d55  ; alice:main
00c8  to                 32  429a980c04447e2a191ae1fd97fffec157e9a34654f2f54132fb0be717f6e56c  ; bob:main
00e8  asset              32  4bd992dd9fd8a101d283432ece0fbaabaf02681f08e99b3c99b9526b76077695  ; USDX
0108  amount             16  0000000000000000000000000016e360                                  ; 1500000
0118  expires_at_ms       8  0000019b76daa800                                                  ; 1767225600000
0120  context_hash       32  85f39acb54ff0571171b405bcea5ae4781b56894c8077326d940617c84113451  ; invoice-7741
0140  conditions_count    1  00                                                                ; vlen(0)
      -- payload ends --
0141  sig_scheme          1  01                                                                ; ed25519
0142  signature          64  b125c7dd81664882b9a83b146a9cfe0942ae463f0aa3e8200e85d391a889d790
                             9be229c9168713502110f56b158a8a54b7bf91fe7d9a96696ac1b41fb159550b

session pubkey : 3b266dde6bffa5a8dce62c2072a1585232238280cc50b1291c4d9dc7e9f6050d
payload_hash   : H("LXP1/payload", 0101 || payload[153])
activity_id    : 73bdba4d2c698beab8ec41d41e1c517cf93a8f11646cf83572dac1ad7298382d
```

Field-by-field notes an implementer must internalise:

- `0x000c` `actor_did` is a 32-byte `did_ref`, never the DID text; the text appears exactly once, in
  the identity-registration payload.
- `0x002c`/`0x002d`: `authority.ref` exists only because `kind` is `01`. Under `kind = 00` bytes
  `0x002d..0x004c` are absent and every later offset shifts down by 32 — encoding 32 zero bytes for a
  primary-key authority is a rejected encoding, not an equivalent one.
- `0x0055`/`0x005d`: the validity window is compared against the *batch* timestamp, the only clock in
  execution, and its span is capped by `PARAM_MAX_VALIDITY_MS`.
- `0x0065`, `0x0076`, `0x0108`: a presence byte precedes an optional value and is never replaced by a
  zero-filled placeholder; `fee_limit` is a full `u128` however small the fee; `amount` is `0x16e360`
  right-aligned in 16 bytes, and bare `16e360` is not a valid alternative.
- `0x0086` is deliberately redundant with the payload that follows: receipts, inclusion proofs and DA
  chunks cite the payload by hash without carrying it, so the decoder MUST verify the redundancy
  rather than trust it.
- `0x00a6` is `99 01`, not `01 99`: LEB128 is little-endian in its 7-bit groups while every scalar is
  big-endian — the most common interop bug, so vectors MUST cover 127/128/16383/16384. `0x0140` is
  `vlen(0)`: empty arrays are encoded, only *optional* fields disappear.
- `0x0142`: verification order is magic/version → structural decode → limits → payload hash →
  signature → semantic admission. Signature verification MUST NOT run before the cheap structural
  rejections.

## 9. Size limits

Enforced at the codec boundary before allocation; all are consensus constants in
`include/layerx/limits.h`, and changing one requires a `protocol_version` bump.

| Constant | Value | Applies to |
|---|---|---|
| `LX_MAX_ACTIVITY` | 65536 B | complete activity envelope |
| `LX_MAX_PAYLOAD` | 65024 B | field 15 |
| `LX_MAX_BYTES_FIELD` | 4096 B | any single `bytes` field |
| `LX_MAX_STR` | 255 B | any `str` |
| `LX_MAX_ARRAY` | 4096 | elements in any array, map or set |
| `LX_MAX_TRANSFER_LEGS` | 64 | legs in one transfer set |
| `LX_MAX_DEPTH` | 8 | structural nesting |
| `LX_MAX_BATCH_ACTIVITIES` | 65536 | activities per batch |
| `LX_MAX_BATCH_BYTES` | 64 MiB | serialized batch |
| `LX_MAX_GUARANTORS` | 256 | signer set in a certificate |
| `LX_MAX_ORACLE_OBS` | 4096 | observations per batch |

Limits are checked with unsigned comparisons against the *remaining* input length. Never compute `offset
+ len` and compare — that addition can overflow `size_t`; write `len > remaining`.

## 10. Conformance

An implementation is conformant when, for the vectors in `tests/vectors/wire/`: every accepted vector
round-trips byte-identically (R6); every rejection vector fails with the exact §7 code; `activity_id`,
`payload_hash` and all §4 preimages match the published bytes including §8; results are identical on
little- and big-endian hosts and on 32- and 64-bit targets; and `fuzz/decode_activity.c` is clean under
ASan, UBSan and MSan.
