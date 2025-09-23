# Rust Workspace

This workspace contains two Rust libraries:

## Libraries

### compiler
A build script compiler for processing schema files. This library provides utilities to:
- Process schema files in a directory
- Generate Rust code from schemas
- Create module declarations for generated code
- Handle build script integration with proper cargo rerun-if-changed directives

**Usage in build.rs:**
```rust
use compiler::SchemaCompiler;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema_dir = Path::new("schemas");

    // Use with a custom processor function
    SchemaCompiler::process_schemas_for_build_script(
        schema_dir,
        |path| {
            // Your schema processing logic here
            Ok(String::from("generated code"))
        }
    )?;

    Ok(())
}
```

### vbare
A versioned data serialization library that provides traits for:
- Versioned data serialization/deserialization
- Automatic version conversion between different schema versions
- Embedded version encoding in payloads
- Support for both borrowed and owned data types

**Features:**
- `VersionedData<'a>` trait for borrowed data
- `OwnedVersionedData` trait for owned data
- Automatic version migration via converter functions
- Embedded version support for self-describing payloads

## Building

```bash
cargo build
```

## Testing

```bash
cargo test
```

## License

MIT OR Apache-2.0