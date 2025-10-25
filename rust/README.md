# Rust

## Crates

- `vbare-gen`: Code generator that parses `.bare` schemas and emits Rust types
- `vbare-compiler`: Build-script helper that processes a directory of schemas
- `vbare`: Runtime traits for versioned data with helpers to serialize/deserialize across versions
    - [crates.io package: `vbare`](https://crates.io/crates/vbare)

## Quick Start

**Step 1: Add dependencies in your `Cargo.toml`:**

```toml
[dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_bare = "0.5"
vbare = "0.0.3"

[build-dependencies]
anyhow = "1"
vbare-compiler = "0.0.3"
```

**Step 2: In `build.rs`, process your `.bare` schema files directory and generate the modules:**

```rust
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schemas = Path::new("schemas");
    vbare_compiler::process_schemas(schemas)?;
    Ok(())
}
```

** Step 3: In your `lib.rs` or `mod.rs`, include the auto-generated module:**

```rust
// Bring generated schemas into this crate
pub mod schemas {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/combined_imports.rs"));
}
```

**Step 4: Implement versioning (example with owned data):**

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

## License

MIT
