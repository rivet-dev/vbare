use std::{env, fs, path::{Path, PathBuf}};

use anyhow::Result;

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Locate repo root from this crate: rust/examples/basic
    let mut repo_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    repo_root.pop(); // rust/examples/basic -> rust/examples
    repo_root.pop(); // rust/examples -> rust
    repo_root.pop(); // rust -> repo root
    // Now at repo root

    let fixtures_dir = repo_root.join("fixtures/tests/basic");
    println!("cargo:rerun-if-changed={}", fixtures_dir.display());

    // Normalize the schemas into a temp dir we control, as the fixtures use a slightly different
    // flavor than our grammar expects.
    let schema_dir = out_dir.join("normalized_schemas");
    fs::create_dir_all(&schema_dir)?;

    for v in ["v1", "v2", "v3"] {
        let src = fixtures_dir.join(format!("{v}.bare"));
        let dst = schema_dir.join(format!("{v}.bare"));

        let content_raw = fs::read_to_string(&src)?;

        // Normalize fixture syntax to match current grammar:
        // - Strip '//' comments
        // - Insert missing 'type' for top-level enums (e.g., `enum X {` -> `type X enum {`)
        // - string -> str
        // - []Todo -> list<Todo>
        // - map<K, V> -> map<K><V>
        let mut normalized = String::new();
        for line in content_raw.lines() {
            let line_wo_comment = match line.find("//") { Some(i) => &line[..i], None => line };
            let trimmed = line_wo_comment.trim_start();
            let converted = if trimmed.starts_with("enum ") {
                let rest = &trimmed["enum ".len()..];
                if let Some(brace_idx) = rest.find('{') {
                    let name = rest[..brace_idx].trim();
                    format!("type {name} enum {{")
                } else {
                    line_wo_comment.to_string()
                }
            } else {
                line_wo_comment.to_string()
            };
            if !converted.trim().is_empty() {
                normalized.push_str(&converted);
                normalized.push('\n');
            }
        }
        let mut normalized = normalized
            .replace("string", "str")
            .replace("[]Todo", "list<Todo>")
            .replace("map<str, str>", "map<str><str>")
            .replace("map<TodoId, Todo>", "map<TodoId><Todo>")
            .replace("map<TagId, Tag>", "map<TagId><Tag>")
            .replace("map<BoardId, Board>", "map<BoardId><Board>")
            .replace("map<str, list<TodoId>>", "map<str><list<TodoId>>");

        if v == "v3" {
            // Ensure ChangeKind enum is defined before Change struct
            let mut lines_all: Vec<&str> = normalized.lines().collect();

            fn extract_block<'a>(lines: &mut Vec<&'a str>, start_pred: &str) -> Option<Vec<&'a str>> {
                let start = lines.iter().position(|l| l.trim_start().starts_with(start_pred))?;
                let mut end = start;
                let mut brace_count = 0i32;
                let mut seen_open = false;
                for i in start..lines.len() {
                    let l = lines[i];
                    if l.contains('{') { brace_count += 1; seen_open = true; }
                    if l.contains('}') { brace_count -= 1; }
                    end = i;
                    if seen_open && brace_count == 0 { break; }
                }
                let block: Vec<&str> = lines[start..=end].to_vec();
                lines.drain(start..=end);
                Some(block)
            }

            let change_block = extract_block(&mut lines_all, "type Change struct");
            let kind_block = extract_block(&mut lines_all, "type ChangeKind enum");

            if change_block.is_some() && kind_block.is_some() {
                let insert_at = lines_all
                    .iter()
                    .position(|l| l.trim_start().starts_with("type Todo struct"))
                    .unwrap_or(lines_all.len());
                let mut rebuilt: Vec<&str> = Vec::new();
                rebuilt.extend_from_slice(&lines_all[..insert_at]);
                for l in kind_block.unwrap() { rebuilt.push(l); }
                for l in change_block.unwrap() { rebuilt.push(l); }
                rebuilt.extend_from_slice(&lines_all[insert_at..]);
                normalized = rebuilt.join("\n");
            }
        }

        fs::write(&dst, normalized)?;
    }

    // Generate Rust from schemas: write one file per schema + combined_imports.rs
    let out_path = &out_dir;
    let mut all_names = Vec::new();
    for entry in fs::read_dir(&schema_dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() { continue; }
        let bare_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit_once('.'))
            .map(|(n, _)| n)
            .expect("valid file name");

        // Use HashMap instead of rivet_util::HashableMap to avoid extra dependency here.
        let tokens = vbare_gen::bare_schema(&path, vbare_gen::Config { use_hashable_map: false });
        let ast = syn::parse2(tokens).expect("parse generated code");
        let content = prettyplease::unparse(&ast);
        fs::write(out_path.join(format!("{bare_name}_generated.rs")), content)?;
        all_names.push(bare_name.to_string());
    }

    let mut mod_content = String::from("// Auto-generated module file for schemas\n\n");
    for name in all_names {
        mod_content.push_str(&format!(
            "pub mod {name} {{\n    include!(concat!(env!(\"OUT_DIR\"), \"/{name}_generated.rs\"));\n}}\n"
        ));
    }
    fs::write(out_path.join("combined_imports.rs"), mod_content)?;

    Ok(())
}
