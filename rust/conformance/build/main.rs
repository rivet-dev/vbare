//! Generates the candidate, reference and bare-ts code for every conformance schema set.
//!
//! See `src/lib.rs` for the generated module layout.

mod bare_ts;
mod model;
mod reference;
mod rust;

use model::{collect_schemas, Error, FileModel, SchemaSet};
use std::{
    env,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};
use vbare_gen::SchemaLoader;

fn main() {
    if let Err(error) = run() {
        panic!("conformance code generation failed: {error}");
    }
}

fn run() -> Result<(), Error> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out = PathBuf::from(env::var("OUT_DIR")?);
    println!("cargo:rerun-if-changed=schemas");
    println!("cargo:rerun-if-changed=build");
    println!("cargo:rerun-if-changed=../examples/basic/schemas");
    println!("cargo:rerun-if-env-changed=VBARE_EXTRA_SCHEMAS");

    let schemas = manifest.join("schemas");
    let mut sets = vec![
        set("reference", schemas.join("reference"), true, true),
        set("candidate", schemas.join("candidate"), false, true),
        set("spec", schemas.join("spec"), false, true),
        set("rust", schemas.join("rust"), false, false),
        // The fixtures/tests/basic v1 to v3 schemas, in the BARE syntax the example uses.
        set(
            "basic",
            manifest.join("../examples/basic/schemas"),
            true,
            true,
        ),
    ];
    if let Some(extra) = env::var_os("VBARE_EXTRA_SCHEMAS") {
        sets.extend(extra_sets(Path::new(&extra))?);
    }

    let bare_ts_dir = out.join("bare-ts");
    let reference_dir = out.join("reference");
    let candidate_dir = out.join("candidate");
    for dir in [&bare_ts_dir, &reference_dir, &candidate_dir] {
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        fs::create_dir_all(dir)?;
    }

    let mut cand = String::new();
    let mut desc = String::new();
    let mut strat = String::new();
    let mut cases = String::new();
    let mut references = String::new();
    let mut entries = Vec::new();

    for set in &sets {
        let files = collect_schemas(&set.root)?;
        let mut loader = SchemaLoader::new(&set.root);
        let mut models = Vec::new();
        for path in &files {
            let model = FileModel::load(&mut loader, &set.name, path)
                .map_err(|error| format!("{}: {error}", set.root.join(path).display()))?;
            models.push(model);
        }

        writeln!(cand, "pub mod {} {{", set.name)?;
        writeln!(desc, "pub mod {} {{", set.name)?;
        writeln!(strat, "pub mod {} {{", set.name)?;
        writeln!(cases, "pub mod {} {{", set.name)?;
        writeln!(references, "pub mod {} {{", set.name)?;
        for model in &models {
            let module = &model.module;

            // Candidate.
            let tokens = loader.generate(&model.path, Default::default())?;
            let file: syn::File = syn::parse2(tokens)?;
            let candidate_file = candidate_dir.join(format!("{}_{module}.rs", set.name));
            fs::write(&candidate_file, prettyplease::unparse(&file))?;
            writeln!(
                cand,
                "pub mod {module} {{ include!({:?}); }}",
                candidate_file.display().to_string()
            )?;

            // Descriptors, strategies and cases.
            let items = rust::generate(model, set.reference)?;
            writeln!(desc, "pub mod {module} {{ {} }}", items.desc)?;
            writeln!(
                strat,
                "pub mod {module} {{ use proptest::prelude::*; {} }}",
                items.strat
            )?;
            writeln!(cases, "pub mod {module} {{ {} }}", items.cases)?;
            entries.extend(items.entries);

            // Reference.
            if set.reference {
                let copy = reference_dir.join(format!("{}_{module}.bare", set.name));
                let (schema, adapter) = reference::generate(model, &copy.display().to_string())?;
                fs::write(&copy, schema)?;
                let adapter_file = reference_dir.join(format!("{}_{module}.rs", set.name));
                fs::write(&adapter_file, adapter)?;
                writeln!(
                    references,
                    "#[allow(non_camel_case_types, non_snake_case, unused_parens)]\npub mod {module} {{ include!({:?}); }}",
                    adapter_file.display().to_string()
                )?;
            }

            // bare-ts.
            if set.bare_ts {
                let dir = bare_ts_dir.join(&set.name);
                fs::create_dir_all(&dir)?;
                fs::write(
                    dir.join(format!("{module}.bare")),
                    bare_ts::generate(model)?,
                )?;
            }
        }
        for module in [
            &mut cand,
            &mut desc,
            &mut strat,
            &mut cases,
            &mut references,
        ] {
            writeln!(module, "}}")?;
        }
    }

    // Candidate code with serde derives, for `tests/serde.rs`.
    let serde_root = schemas.join("serde");
    let mut serde_loader = SchemaLoader::new(&serde_root);
    let mut serde_cand = String::new();
    for path in collect_schemas(&serde_root)? {
        let module = vbare_gen::module_name(&path)?;
        let tokens = serde_loader.generate(&path, vbare_gen::Config::default().with_serde(true))?;
        let file: syn::File = syn::parse2(tokens)?;
        let serde_file = candidate_dir.join(format!("serde_{module}.rs"));
        fs::write(&serde_file, prettyplease::unparse(&file))?;
        writeln!(
            serde_cand,
            "pub mod {module} {{ include!({:?}); }}",
            serde_file.display().to_string()
        )?;
    }
    fs::write(out.join("serde.rs"), serde_cand)?;

    let generated = format!(
        "pub mod cand {{ {cand} }}\n\
         pub mod desc {{ {desc} }}\n\
         pub mod strat {{ {strat} }}\n\
         pub mod cases {{ {cases} }}\n\
         pub mod reference {{ {references} }}\n\
         pub static ENTRIES: &[crate::harness::Entry] = &[\n{}\n];\n",
        entries.join(",\n")
    );
    fs::write(out.join("generated.rs"), generated)?;
    Ok(())
}

fn set(name: &str, root: PathBuf, reference: bool, bare_ts: bool) -> SchemaSet {
    SchemaSet {
        name: name.to_owned(),
        root,
        reference,
        bare_ts,
    }
}

/// Schema roots under `VBARE_EXTRA_SCHEMAS`: the directory itself if it holds `.bare` files
/// directly, and each subdirectory that holds any. Each root resolves its own imports.
fn extra_sets(dir: &Path) -> Result<Vec<SchemaSet>, Error> {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut sets = Vec::new();
    let mut direct = false;
    let mut subdirs = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            subdirs.push(path);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "bare")
        {
            direct = true;
        }
    }
    subdirs.sort();
    if direct {
        sets.push(set("extra", dir.to_owned(), true, true));
    }
    for subdir in subdirs {
        if collect_schemas(&subdir)?.is_empty() {
            continue;
        }
        let name = subdir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("non-UTF-8 extra schema directory")?
            .replace(['-', '.'], "_");
        sets.push(set(&format!("extra_{name}"), subdir, true, true));
    }
    Ok(sets)
}
