use indoc::formatdoc;
use std::{fs, path::Path};

/// Configuration for the vbare-compiler.
///
/// This allows callers to control how code generation behaves, including
/// passing through configuration to `vbare_gen`.
pub struct Config {
    /// Configuration forwarded to `vbare_gen` for schema codegen.
    pub vbare: vbare_gen::Config,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vbare: vbare_gen::Config {
                use_hashable_map: false,
            },
        }
    }
}

impl Config {
    /// Convenience helper to enable hashable maps in generated code.
    pub fn with_hashable_map() -> Self {
        Self {
            vbare: vbare_gen::Config {
                use_hashable_map: true,
            },
        }
    }
}

/// Process BARE schema files and generate Rust code.
pub fn process_schemas(schema_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    process_schemas_with_config(schema_dir, &Config::default())
}

/// Process BARE schema files and generate Rust code, using the provided config.
pub fn process_schemas_with_config(
    schema_dir: &Path,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let out_path = Path::new(&out_dir);

    println!("cargo:rerun-if-changed={}", schema_dir.display());

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

        let tokens = vbare_gen::bare_schema(
            &path,
            vbare_gen::Config {
                use_hashable_map: config.vbare.use_hashable_map,
            },
        );
        let ast = syn::parse2(tokens)?;
        let content = prettyplease::unparse(&ast);

        fs::write(out_path.join(format!("{bare_name}_generated.rs")), content)?;

        all_names.push(bare_name.to_string());
    }

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

    fs::write(out_path.join("combined_imports.rs"), mod_content)?;

    Ok(())
}
