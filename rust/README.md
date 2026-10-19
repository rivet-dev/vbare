# Rust

## Crates

- `vbare-gen`: Code generator that parses `.bare` schemas and emits Rust types
- `vbare-compiler`: Build-script helper that processes a directory of schemas
- `vbare`: Runtime with the zero-copy codec and versioned framing used by generated code
    - [crates.io package: `vbare`](https://crates.io/crates/vbare)

## Quick Start

**Step 1: Add dependencies in your `Cargo.toml`:**

```toml
[dependencies]
anyhow = "1"
vbare = "0.2.0"

[build-dependencies]
vbare-compiler = "0.2.0"
```

Generated code depends only on `vbare`, which re-exports the `bytes` types it uses.

**Step 2: In `build.rs`, process your `.bare` schema files directory and generate the modules:**

```rust
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schemas = Path::new("schemas");
    vbare_compiler::process_schemas(schemas)?;
    Ok(())
}
```

**Step 3: In your `lib.rs` or `mod.rs`, include the auto-generated module:**

```rust
// Bring generated schemas into this crate
pub mod schemas {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/combined_imports.rs"));
}
```

**Step 4: Encode, decode and view messages:**

Every schema type `Foo` generates an owned `Foo`, a borrowed `FooRef<'a>` whose accessors read
fields in place, and `FooView`, which owns a verified buffer. Decoding verifies the input once and
never copies `str` or `data` bytes.

```rust
use vbare::{Bytes, Encode, Message};

let bytes: Bytes = my_type.encode_to_bytes();

// Zero-copy: verify once, then read fields in place.
let view = schemas::v2::MyType::view(bytes.clone())?;
let name: &str = view.get().name();

// Owned: `Str` and `Bytes` fields share `bytes`.
let owned = schemas::v2::MyType::decode(bytes)?;
```

**Step 5: Implement versioning:**

```rust
use anyhow::{bail, Result};
use vbare::{Bytes, DecodeError, DecodeErrorKind, Encode, EncodeError, Message, Sink, Versioned};

#[derive(Clone)]
pub enum MyTypeVersioned {
    V1(schemas::v1::MyType),
    V2(schemas::v2::MyType),
}

impl Versioned for MyTypeVersioned {
    type Latest = schemas::v2::MyType;

    const LATEST: u16 = 2;
    // UPGRADES[i] converts version i + 1 to i + 2.
    const UPGRADES: &'static [vbare::Converter<Self>] = &[Self::v1_to_v2];
    // DOWNGRADES[i] converts version LATEST - i to LATEST - i - 1.
    const DOWNGRADES: &'static [vbare::Converter<Self>] = &[Self::v2_to_v1];

    fn wrap_latest(latest: Self::Latest) -> Self { Self::V2(latest) }
    fn unwrap_latest(self) -> Result<Self::Latest> {
        match self { Self::V2(x) => Ok(x), Self::V1(_) => bail!("not latest") }
    }

    fn decode_version(version: u16, body: Bytes) -> Result<Self, DecodeError> {
        match version {
            1 => Ok(Self::V1(schemas::v1::MyType::decode(body)?)),
            2 => Ok(Self::V2(schemas::v2::MyType::decode(body)?)),
            _ => Err(DecodeError::new(DecodeErrorKind::UnsupportedVersion, 0)),
        }
    }

    fn encode_version<S: Sink>(&self, _version: u16, out: &mut S) -> Result<(), EncodeError> {
        match self {
            Self::V1(x) => x.encode(out),
            Self::V2(x) => x.encode(out),
        }
        Ok(())
    }
}
```

Then use `decode`/`open`/`seal`, which read and write the 2-byte little-endian version header,
or their `*_body` variants when the version is negotiated separately:

```rust
// Embed the version in the payload header.
let bytes = MyTypeVersioned::seal_to_vec(&latest, 1)?;

// Or write the header and body straight into a buffer you own, such as a frame that already
// holds a length prefix. `sealed_len` is the exact length at the latest version.
frame.reserve(MyTypeVersioned::sealed_len(&latest));
MyTypeVersioned::seal(&latest, 2, &mut frame)?;
let written = MyTypeVersioned::seal_to_slice(&latest, 2, &mut fixed_buffer)?;

// Decode any supported version into the latest type.
let latest = MyTypeVersioned::decode(Bytes::from(bytes))?;

// View the latest version in place; older versions are converted first.
let view = MyTypeVersioned::open(payload)?;

// Pre-negotiated versions: no header.
let latest = MyTypeVersioned::decode_body(body, 1)?;
```

## License

MIT
