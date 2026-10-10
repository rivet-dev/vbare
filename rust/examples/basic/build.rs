use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let imports = Path::new("../../../fixtures/tests/imports/valid");
    vbare_compiler::process_schemas(imports)?;
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    std::fs::rename(
        out.join("combined_imports.rs"),
        out.join("import_fixtures.rs"),
    )?;

    let schemas = Path::new("schemas");
    vbare_compiler::process_schemas(schemas)?;
    Ok(())
}
