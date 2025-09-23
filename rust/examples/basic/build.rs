use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schemas = Path::new("schemas");
    vbare_compiler::process_schemas(schemas)?;
    Ok(())
}
