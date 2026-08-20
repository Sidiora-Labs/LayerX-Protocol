# `indexmap-nostd`

A `no_std` compatible [`indexmap` crate] (re)implementation.

**Note:** The [`indexmap` crate] already supports to be compiled in
`no_std` environments and it uses [`hashbrown` crate]'s `HashMap` under the
hood which still requires some sort of randomized initialization.
However, some embedded platforms simply cannot provide ways to randomly
seed hash maps and similar data structures making code that depends on it
susceptible to users (or attackers) that control inputs to those hash maps.

Therefore `indexmap-nostd` is a (re)implementation of the [`indexmap` crate]
that replaces the internal use of `HashMap` with `BTreeMap`.

## Advantages

This crate and its data structures can be used in any embedded `no_std`
environment without the need to provide random seeds for `HashMap` initialization.

## Disadvantages

- The current implementation of `indexmap-nostd` focuses on being easy to
  maintain simple code which trades off efficiency compared to the original
  [`indexmap` crate].
  An example of performance regression is that now inserted keys are duplicated.
- Due to the above point some methods now require additional where bounds.
  For example `IndexMap::insert` now requires `K: Clone`.
- We are primarily interested in getting this `no_std` compatible implementation
  to be working for the [`wasmparser` crate]. This means that we primarily
  provide a subset of the features and API of the original [`indexmap` crate]
  and might not be interested in adding features that we do not need for
  this use case that are also hard to implement or maintain.

[`indexmap` crate]: https://www.crates.io/crates/indexmap
[`wasmparser` crate]: https://www.crates.io/crates/wasmparser-nostd
[`hashbrown` crate]: https://www.crates.io/crates/hashbrown
