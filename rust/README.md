# Rust Workspace

This workspace contains the code generator and runtime for working with BARE schemas and versioned data, plus a runnable example.

## Crates

- `vbare-gen`: TokenStream code generator that parses `.bare` schemas and emits Rust types deriving `serde` and using `serde_bare` for encoding/decoding.
- `vbare-compiler`: Build-script helper that processes a directory of schemas, writes one Rust file per schema into `OUT_DIR`, and emits a `combined_imports.rs` module to include from your crate.
- `vbare`: Runtime traits for versioned data with helpers to serialize/deserialize across versions and with embedded version headers.
- `examples/basic`: End-to-end example that generates types for three schema versions (v1/v2/v3) and shows migrations between them.

## Quick Start (use in your crate)

1) Add dependencies in your `Cargo.toml`:

```toml
[dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_bare = "0.5"
vbare = { path = "../vbare" } # adjust path as needed

[build-dependencies]
anyhow = "1"
vbare-compiler = { path = "../vbare-compiler" } # or use vbare-gen directly
```

2) In `build.rs`, process your `.bare` schema files directory and generate the modules:

```rust
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schemas = Path::new("schemas");
    // Or `process_schemas_with_config(schemas, &vbare_compiler::Config::with_hashable_map())`.
    vbare_compiler::process_schemas(schemas)?;
    Ok(())
}
```

Note: If you prefer to call the generator directly (as in `examples/basic`), use `vbare-gen` from your `build.rs`, parse the returned `TokenStream` with `syn`, and format with `prettyplease`. In that case add `syn` and `prettyplease` to `[build-dependencies]`.

3) In your `lib.rs` or `mod.rs`, include the auto-generated module that re-exports all generated files:

```rust
// Bring generated schemas into this crate
pub mod schemas {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/combined_imports.rs"));
}
```

4) Implement versioning (example with owned data):

```rust
use anyhow::{bail, Result};
use vbare::OwnedVersionedData;

#[derive(Clone)]
pub enum MyTypeVersioned {
    V1(schemas::v1::MyType),
    V2(schemas::v2::MyType),
}

impl OwnedVersionedData for MyTypeVersioned {
    type Latest = schemas::v2::MyType;

    fn latest(latest: Self::Latest) -> Self { Self::V2(latest) }
    fn into_latest(self) -> Result<Self::Latest> {
        match self { Self::V2(x) => Ok(x), _ => bail!("not latest") }
    }

    fn deserialize_version(payload: &[u8], version: u16) -> Result<Self> {
        Ok(match version {
            1 => Self::V1(serde_bare::from_slice(payload)?),
            2 => Self::V2(serde_bare::from_slice(payload)?),
            _ => bail!("invalid version: {version}"),
        })
    }

    fn serialize_version(self, _version: u16) -> Result<Vec<u8>> {
        Ok(match self {
            Self::V1(x) => serde_bare::to_vec(&x)?,
            Self::V2(x) => serde_bare::to_vec(&x)?,
        })
    }

    fn deserialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        vec![Self::v1_to_v2] // order: v1->v2, v2->v3, ...
    }

    fn serialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        vec![Self::v2_to_v1] // optional: latest->older conversions
    }
}
```

Then use `deserialize`/`serialize` or their `*_with_embedded_version` variants:

```rust
// Decode bytes encoded as version 1 into latest
let latest = MyTypeVersioned::deserialize(&bytes, 1)?;

// Encode latest as version 1
let v1_bytes = MyTypeVersioned::latest(latest).serialize(1)?;

// Or embed version in the payload header (little-endian u16 prefix):
let bytes = MyTypeVersioned::latest(latest).serialize_with_embedded_version(2)?;
let latest2 = MyTypeVersioned::deserialize_with_embedded_version(&bytes)?;
```

## Example

See `rust/examples/basic/` for a full example:
- `build.rs` normalizes the test fixture schemas and runs codegen (mirrors what `vbare-compiler` does).
- `src/lib.rs` includes the generated `schemas` module and implements `OwnedVersionedData` for `AppVersioned` with v1→v2→v3 migrations.
- `tests/migrator.rs` exercises up/down conversions and BARE encoding with `serde_bare`.

Run just the example crate’s tests:

```bash
cargo test -p basic
```

## Workspace

Build and test everything:

```bash
cargo build
cargo test
```

## License

MIT OR Apache-2.0
