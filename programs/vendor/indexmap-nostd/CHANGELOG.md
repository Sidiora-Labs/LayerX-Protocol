# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Version 0.4.0

### Added

- Added `IndexMap::get_key_value` method.
- Added `IndexMap::get_full` method.
- Added `IndexMap::get_index_of` method.
- Added `IndexMap::{get_index, get_index_mut}` methods.
- Added `IndexMap::insert_full` method.
- Added `IndexSet::get_full` method.
- Added `IndexSet::get_index_of` method.
- Added `IndexSet::get_index` method.
- Added `IndexSet::insert_full` method.
- Added `Index<usize>` and `IndexMut<usize>` impls to `IndexMap`.
- Added `Index<usize>` impl to `IndexSet`.

## Version 0.3.0

### Added

- Support for `serde` (de)serialization using the `serde` crate feature.
    - This also includes sequence based (de)serialization as is also supported
      in the original `indexmap` crate via `serde_seq` submodule.

## Version 0.2.0

### Added

- `Index{Map,Set}::with_capacity` constructor API.
- `Index{Map,Set}::reserve` API.

## Version 0.1.0

Initial release of the crate.
