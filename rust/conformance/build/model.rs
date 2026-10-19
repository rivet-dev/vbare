//! One schema file with its import closure, after the candidate generator's naming.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use vbare_gen::{
    ast::{AnyType, PrimitiveType},
    naming::{hoist, Definition},
    ResolvedSchema, SchemaLoader,
};

pub type Error = Box<dyn std::error::Error>;

/// A group of schema files sharing one import root.
pub struct SchemaSet {
    pub name: String,
    pub root: PathBuf,
    /// Whether `bare_proc` and `serde_bare` can express every schema in the set.
    pub reference: bool,
    /// Whether bare-ts copies are generated.
    pub bare_ts: bool,
}

pub struct FileModel {
    pub set: String,
    pub module: String,
    pub path: PathBuf,
    pub resolved: ResolvedSchema,
    /// Every type in the import closure, with anonymous aggregates named as the candidate
    /// generator names them.
    pub defs: BTreeMap<String, Definition>,
    /// The generated module that defines each type in `defs`.
    pub module_of: BTreeMap<String, String>,
    /// Types this file defines, user types and named anonymous types, in dependency order.
    pub local: Vec<String>,
}

impl FileModel {
    pub fn load(loader: &mut SchemaLoader, set: &str, path: &Path) -> Result<Self, Error> {
        let module = vbare_gen::module_name(path)?;
        let resolved = loader.resolve_schema(path)?;
        let mut defs = BTreeMap::new();
        let mut module_of = BTreeMap::new();
        for (name, definition) in &resolved.registry {
            let origin = resolved
                .origins
                .get(name)
                .ok_or_else(|| format!("no origin for {name}"))?;
            let origin_module = vbare_gen::module_name(origin)?;
            for hoisted in hoist(name, definition) {
                module_of.insert(hoisted.name.clone(), origin_module.clone());
                defs.insert(hoisted.name.clone(), hoisted);
            }
        }
        let mut model = FileModel {
            set: set.to_owned(),
            module,
            path: path.to_owned(),
            resolved,
            defs,
            module_of,
            local: Vec::new(),
        };
        let all: Vec<String> = model.defs.keys().cloned().collect();
        model.local = model
            .topological(&all)
            .into_iter()
            .filter(|name| model.module_of[name] == model.module)
            .collect();
        Ok(model)
    }

    /// Whether `name` was written in the schema, as opposed to named by the generator.
    pub fn is_user_type(&self, name: &str) -> bool {
        self.defs
            .get(name)
            .is_some_and(|definition| !definition.hoisted)
    }

    /// Follows a chain of named aliases to the first definition that is not a bare reference.
    pub fn resolve_name<'a>(&'a self, mut name: &'a str) -> &'a str {
        while let Some(AnyType::TypeReference(next)) =
            self.defs.get(name).map(|definition| &definition.definition)
        {
            name = next;
        }
        name
    }

    /// Follows references to the underlying definition.
    pub fn resolve<'a>(&'a self, definition: &'a AnyType) -> &'a AnyType {
        match definition {
            AnyType::TypeReference(name) => {
                let target = self.resolve_name(name);
                &self.defs[target].definition
            }
            AnyType::Primitive(_)
            | AnyType::List { .. }
            | AnyType::Struct(_)
            | AnyType::Enum(_)
            | AnyType::Map { .. }
            | AnyType::Union(_)
            | AnyType::Optional(_) => definition,
        }
    }

    pub fn is_void(&self, definition: &AnyType) -> bool {
        matches!(
            self.resolve(definition),
            AnyType::Primitive(PrimitiveType::Void)
        )
    }

    /// The candidate type path for a named type.
    pub fn candidate_path(&self, name: &str) -> String {
        format!(
            "crate::generated::cand::{}::{}::{}",
            self.set, self.module_of[name], name
        )
    }

    pub fn item_path(&self, kind: &str, name: &str) -> String {
        format!(
            "crate::generated::{kind}::{}::{}::{}",
            self.set, self.module_of[name], name
        )
    }

    /// Orders `names` so every type comes after the types it references.
    pub fn topological(&self, names: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for name in names {
            self.visit(name, &mut seen, &mut out);
        }
        out
    }

    fn visit(&self, name: &str, seen: &mut BTreeSet<String>, out: &mut Vec<String>) {
        if !seen.insert(name.to_owned()) {
            return;
        }
        if let Some(definition) = self.defs.get(name) {
            let mut references = Vec::new();
            references_of(&definition.definition, &mut references);
            for reference in references {
                self.visit(&reference, seen, out);
            }
        }
        out.push(name.to_owned());
    }
}

pub fn references_of(definition: &AnyType, out: &mut Vec<String>) {
    match definition {
        AnyType::Primitive(_) | AnyType::Enum(_) => {}
        AnyType::TypeReference(name) => out.push(name.clone()),
        AnyType::List { inner, .. } | AnyType::Optional(inner) => references_of(inner, out),
        AnyType::Map { key, value } => {
            references_of(key, out);
            references_of(value, out);
        }
        AnyType::Struct(fields) => {
            for field in fields {
                references_of(&field.type_r, out);
            }
        }
        AnyType::Union(members) => {
            for (member, _) in members {
                references_of(member, out);
            }
        }
    }
}

/// Collects `.bare` files below `root`, relative to it, sorted.
pub fn collect_schemas(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut out = Vec::new();
    collect(root, Path::new(""), &mut out)?;
    out.sort();
    Ok(out)
}

fn collect(root: &Path, relative: &Path, out: &mut Vec<PathBuf>) -> Result<(), Error> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            collect(root, &path, out)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "bare")
        {
            out.push(path);
        }
    }
    Ok(())
}
