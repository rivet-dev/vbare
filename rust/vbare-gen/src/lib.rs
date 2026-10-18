/*!
`vbare-gen` generates Rust types and a zero-copy BARE codec from schema files. Build scripts
usually call it through `vbare-compiler`. Generated code depends only on the `vbare` runtime.

For a schema such as

```bare
type Customer struct {
  name: str
  orders: list<struct {
    orderId: i64
    quantity: i32
  }>
  metadata: map<str><data>
}
```

every user type `Foo` produces four items:

- `Foo`, the owned type for building, storing and encoding values;
- `FooRef<'a>`, a borrowed view whose accessors read fields in place from a verified buffer;
- `FooWire`, the marker type implementing `vbare::Wire` (verification, reads and encoding);
- `FooView`, an alias for `vbare::View<FooWire>`, which owns a verified buffer.

Structs, unions and enums also implement `vbare::Message` (`Foo::view`, `Foo::decode`) and
`vbare::Encode`.

# BARE to Rust mapping

| BARE | Owned | Borrowed |
|---|---|---|
| `uint`, `int` | `u64`, `i64` | same |
| `u8` .. `i64`, `f32`, `f64`, `bool` | same | same |
| `str` | `vbare::Str` | `&'a str` |
| `data` | `vbare::Bytes` | `&'a [u8]` |
| `data[N]` | `[u8; N]` up to 32, `vbare::FixedBytes<N>` above | `&'a [u8; N]` |
| `optional<T>` | `Option<T>` | `Option<TRef>` |
| `list<T>` | `Vec<T>` | `vbare::ListRef` |
| `list<T>[N]` | `[T; N]` | `vbare::ListRef` |
| `map<K><V>` | `BTreeMap<K, V>`, encoded in key order | `vbare::MapRef` |
| `enum` | generated `Copy` enum | same |
| `union` | generated enum | generated enum of borrowed members |
| `struct` | generated struct | generated view struct |

Decoded `Str`, `Bytes` and `FixedBytes` values share the input buffer instead of copying it.
Field names become snake_case and enum values UpperCamelCase. Anonymous structs, unions and enums
are named after where they appear: `orders` above becomes `CustomerOrders`, and union member `i`
of `Foo` becomes `FooMember{i}`.

Explicit enum values and union tags (`A = 5`) follow draft-devault-bare-11: unassigned values
continue from the previous value plus one. Schemas are validated against the spec's invariants
before generation.
*/

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use parser::AnyType;
use proc_macro2::TokenStream;

mod codegen;
mod imports;
// Vendored upstream code keeps its own formatting and lint profile.
#[rustfmt::skip]
#[allow(clippy::all)]
#[path = "bare_proc/parser.rs"]
mod parser;
mod validate;
pub use imports::{module_name, ResolvedSchema, SchemaLoader};

/// The parsed schema representation produced by the vendored parser.
pub mod ast {
    pub use crate::parser::{AnyType, Length, PrimitiveType, StructField};
}

/// The names the generator gives to types, fields and variants, for tools that work with
/// generated code.
pub mod naming {
    pub use crate::codegen::hoist::{hoist, Definition};
    pub use crate::codegen::{rust_enum_variant_name, rust_field_name, rust_union_variant_name};
}

/// Parses and validates a single schema source without resolving `#import`s. Syntax errors carry
/// the line and column of the offending token.
pub fn parse_schema(source: &str) -> Result<BTreeMap<String, AnyType>, String> {
    let schema = parser::parse_string(source, &BTreeMap::new())?;
    validate::validate(&schema, &schema)?;
    Ok(schema)
}

/// Configuration for code generation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct Config {
    /// Derive `serde::Serialize` and `serde::Deserialize` on generated owned types.
    ///
    /// The crate that includes the generated code must depend on `serde` with the `derive`
    /// feature and enable the `serde` feature of `vbare`. The derives are shaped so `serde_bare`
    /// writes exactly the bytes the codec writes. Schemas that `serde_bare` cannot encode that way
    /// are rejected: `uint` and `int` (a plain `u64` or `i64` would be written as fixed-width
    /// integers), enum values and union tags that differ from their declaration index (serde
    /// writes the index), and fixed-length lists longer than 32 elements (serde has no impls for
    /// them).
    pub serde: bool,
}

impl Config {
    /// Sets [`serde`](Config::serde).
    pub const fn with_serde(mut self, serde: bool) -> Self {
        self.serde = serde;
        self
    }
}

/// Generates Rust code for the schema at `schema_path`. Imports resolve relative to the schema's
/// directory.
pub fn bare_schema(
    schema_path: &Path,
    config: Config,
) -> Result<TokenStream, Box<dyn std::error::Error>> {
    let root = schema_path.parent().unwrap_or_else(|| Path::new("."));
    let name = schema_path
        .file_name()
        .map(PathBuf::from)
        .ok_or_else(|| format!("schema path has no file name: {}", schema_path.display()))?;
    SchemaLoader::new(root).generate(&name, config)
}
