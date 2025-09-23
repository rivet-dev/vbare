use std::{env, fs, path::PathBuf};

// Exercise process_schemas against the repo fixtures in fixtures/tests/basic
#[test]
fn processes_basic_fixtures() {
    // Build path to fixtures/tests/basic relative to this crate directory
    let mut fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures_dir.pop(); // up to rust/
    fixtures_dir.pop(); // up to repo root
    fixtures_dir.push("fixtures/tests/basic");

    assert!(
        fixtures_dir.is_dir(),
        "fixtures dir missing: {}",
        fixtures_dir.display()
    );

    // Create a dedicated OUT_DIR for the test and set env var
    let out_dir = tempfile::tempdir().expect("create tempdir for OUT_DIR");
    env::set_var("OUT_DIR", out_dir.path());

    // Create an isolated schema dir and copy/normalize all three versions from fixtures
    let schema_dir = tempfile::tempdir().expect("create tempdir for schema dir");

    for v in ["v1", "v2", "v3"] {
        let src = fixtures_dir.join(format!("{v}.bare"));
        let dst = schema_dir.path().join(format!("{v}.bare"));

        let content_raw = fs::read_to_string(&src).expect("read schema fixture");

        // Normalize fixture syntax to match current grammar:
        // - Strip '//' comments
        // - Insert missing 'type' for top-level enums (e.g., `enum X {` -> `type X enum {`)
        // - string -> str
        // - []Todo -> list<Todo>
        // - map<K, V> -> map<K><V>
        let mut normalized = String::new();
        for line in content_raw.lines() {
            let line_wo_comment = match line.find("//") {
                Some(i) => &line[..i],
                None => line,
            };
            let trimmed = line_wo_comment.trim_start();
            let converted = if trimmed.starts_with("enum ") {
                let rest = &trimmed["enum ".len()..];
                if let Some(brace_idx) = rest.find('{') {
                    let name = rest[..brace_idx].trim();
                    format!("type {name} enum {{")
                } else {
                    // Fallback: keep original if malformed
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

        // Fix forward reference in v3: ensure ChangeKind is defined before Change
        if v == "v3" {
            // Reorder so that ChangeKind enum appears before Change struct, and both before Todo
            let mut lines_all: Vec<&str> = normalized.lines().collect();

            fn extract_block<'a>(
                lines: &mut Vec<&'a str>,
                start_pred: &str,
            ) -> Option<Vec<&'a str>> {
                let start = lines
                    .iter()
                    .position(|l| l.trim_start().starts_with(start_pred))?;
                let mut end = start;
                let mut brace_count = 0i32;
                let mut seen_open = false;
                for i in start..lines.len() {
                    let l = lines[i];
                    if l.contains('{') {
                        brace_count += 1;
                        seen_open = true;
                    }
                    if l.contains('}') {
                        brace_count -= 1;
                    }
                    end = i;
                    if seen_open && brace_count == 0 {
                        break;
                    }
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
                for l in kind_block.unwrap() {
                    rebuilt.push(l);
                }
                for l in change_block.unwrap() {
                    rebuilt.push(l);
                }
                rebuilt.extend_from_slice(&lines_all[insert_at..]);
                normalized = rebuilt.join("\n");
            }
        }

        fs::write(&dst, normalized).expect("write normalized schema");
    }

    // Run the schema processor on the isolated dir
    vbare_compiler::process_schemas(schema_dir.path()).expect("process schemas");

    // Verify expected generated files exist and are non-empty
    let expected = [
        "v1_generated.rs",
        "v2_generated.rs",
        "v3_generated.rs",
        "combined_imports.rs",
    ];

    for file in expected {
        let p = out_dir.path().join(file);
        assert!(p.exists(), "missing generated file: {}", p.display());
        let meta = fs::metadata(&p).expect("metadata");
        assert!(meta.len() > 0, "empty generated file: {}", p.display());
    }
}
