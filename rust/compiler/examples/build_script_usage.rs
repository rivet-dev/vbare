// Example of how to use the compiler library in a build.rs file

use compiler::SchemaCompiler;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Using the simple example processor
    let schema_dir = Path::new("schemas");
    SchemaCompiler::process_schemas_for_build_script(
        schema_dir,
        SchemaCompiler::example_processor
    )?;

    // Example 2: Using a custom processor
    let custom_processor = |path: &Path| -> Result<String, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;

        // Your custom processing logic here
        // For example, you might parse the schema and generate Rust types

        Ok(format!(
            "// Generated from: {:?}\n\
             // Original content length: {} bytes\n\
             \n\
             pub struct CustomGenerated {{\n\
             \tpub data: Vec<u8>,\n\
             }}",
            path.file_name().unwrap(),
            content.len()
        ))
    };

    SchemaCompiler::process_schemas_for_build_script(
        schema_dir,
        custom_processor
    )?;

    Ok(())
}