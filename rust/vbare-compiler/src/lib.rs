use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Configuration for the vbare-compiler.
///
/// This allows callers to control how code generation behaves, including
/// passing through configuration to `vbare_gen`.
#[derive(Default)]
pub struct Config {
    /// Configuration forwarded to `vbare_gen` for schema codegen.
    pub vbare: vbare_gen::Config,
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
    process_schemas_to_dir(schema_dir, Path::new(&out_dir), config)
}

/// Generate all `.bare` files recursively, using `schema_dir` as the import root.
/// Directory names prefix module names; dots and separators become underscores.
pub fn process_schemas_to_dir(
    schema_dir: &Path,
    out_path: &Path,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={}", schema_dir.display());
    let mut paths = Vec::new();
    collect_schemas(schema_dir, Path::new(""), &mut paths)?;
    paths.sort();
    let mut names = BTreeMap::new();
    for path in &paths {
        let name = vbare_gen::module_name(path)?;
        if let Some(previous) = names.insert(name.clone(), path.clone()) {
            return Err(format!(
                "schemas {} and {} map to the same module {name}",
                previous.display(),
                path.display()
            )
            .into());
        }
    }
    let mut loader = vbare_gen::SchemaLoader::new(schema_dir);
    let mut mod_content = String::from("// Auto-generated module file for schemas\n\n");
    for path in paths {
        let tokens = loader.generate(&path, config.vbare)?;
        let ast = syn::parse2(tokens)?;
        let content = prettyplease::unparse(&ast);
        let module_name = vbare_gen::module_name(&path)?;
        // Preserve existing top-level output filenames for build script consumers.
        let output_name = path
            .with_extension("")
            .components()
            .map(|part| {
                part.as_os_str()
                    .to_str()
                    .ok_or("Invalid UTF-8 in file name")
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("_");
        fs::write(
            out_path.join(format!("{output_name}_generated.rs")),
            content,
        )?;
        mod_content.push_str(&format!(
            "pub mod {module_name} {{\n    include!(concat!(env!(\"OUT_DIR\"), \"/{output_name}_generated.rs\"));\n}}\n"
        ));
    }
    fs::write(out_path.join("combined_imports.rs"), mod_content)?;
    Ok(())
}

fn collect_schemas(root: &Path, relative: &Path, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_schemas(root, &path, paths)?;
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "bare")
        {
            paths.push(path);
        }
    }
    Ok(())
}
