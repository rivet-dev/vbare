use std::{fs, path::Path};
use indoc::formatdoc;

/// A simple build script compiler for processing schema files.
///
/// This is a simplified version that can be extended with actual schema
/// processing logic. Currently it generates module declarations for
/// schema files found in a directory.
pub struct SchemaCompiler;

impl SchemaCompiler {
    /// Process schema files and generate Rust code.
    ///
    /// This function will:
    /// 1. Find all schema files in the given directory
    /// 2. Process each schema file (currently just generates module declarations)
    /// 3. Create a combined imports file
    pub fn process_schemas(
        schema_dir: &Path,
        out_dir: &Path,
        processor: impl Fn(&Path) -> Result<String, Box<dyn std::error::Error>>
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut all_names = Vec::new();

        for entry in fs::read_dir(schema_dir)?.flatten() {
            let path = entry.path();

            if path.is_dir() {
                continue;
            }

            let bare_name = path
                .file_name()
                .ok_or("No file name")?
                .to_str()
                .ok_or("Invalid UTF-8 in file name")?
                .rsplit_once('.')
                .ok_or("No file extension")?
                .0;

            // Process the schema file using the provided processor
            let content = processor(&path)?;

            // Write the generated content
            fs::write(out_dir.join(format!("{bare_name}_generated.rs")), content)?;

            all_names.push(bare_name.to_string());
        }

        // Generate combined imports file
        let mut mod_content = String::new();
        mod_content.push_str("// Auto-generated module file for schemas\n\n");

        for name in all_names {
            mod_content.push_str(&formatdoc!(
                r#"
                pub mod {name} {{
                    include!(concat!(env!("OUT_DIR"), "/{name}_generated.rs"));
                }}
                "#,
            ));
        }

        let mod_file_path = out_dir.join("combined_imports.rs");
        fs::write(&mod_file_path, mod_content)?;

        Ok(())
    }

    /// Process schemas for use in a build script.
    ///
    /// This handles the standard build script setup:
    /// - Gets OUT_DIR from environment
    /// - Sets up cargo:rerun-if-changed
    /// - Processes the schemas
    pub fn process_schemas_for_build_script(
        schema_dir: &Path,
        processor: impl Fn(&Path) -> Result<String, Box<dyn std::error::Error>>
    ) -> Result<(), Box<dyn std::error::Error>> {
        let out_dir = std::env::var("OUT_DIR")?;
        let out_path = Path::new(&out_dir);

        println!("cargo:rerun-if-changed={}", schema_dir.display());

        Self::process_schemas(schema_dir, out_path, processor)
    }

    /// A simple example processor that generates a stub module.
    /// Replace this with your actual schema processing logic.
    pub fn example_processor(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid file name")?;

        Ok(format!(
            "// Generated from: {}\n\
             // TODO: Add actual schema processing logic here\n\
             \n\
             pub struct Generated {{\n\
             \tpub name: String,\n\
             }}",
            file_name
        ))
    }

    /// Process BARE schema files and generate Rust code using bare_gen.
    pub fn bare_processor(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let tokens = bare_gen::bare_schema(path);
        let ast = syn::parse2(tokens)?;
        let formatted = prettyplease::unparse(&ast);
        Ok(formatted)
    }

    /// Convenience function for processing BARE schemas in a build script.
    pub fn process_bare_schemas(schema_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        Self::process_schemas_for_build_script(schema_dir, Self::bare_processor)
    }
}