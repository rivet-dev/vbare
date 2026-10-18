//! Resolve root-relative schema imports once, preserving each type's defining module.

use crate::{
    parser::{self, AnyType},
    validate, Config, SchemaGenerator,
};
use proc_macro2::TokenStream;
use quote::quote;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

type Error = Box<dyn std::error::Error>;

#[derive(Clone)]
struct Schema {
    local: BTreeMap<String, AnyType>,
    registry: BTreeMap<String, AnyType>,
    origins: BTreeMap<String, PathBuf>,
    dependencies: BTreeSet<PathBuf>,
}

/// A compiler invocation's schema root and resolved import graph.
pub struct SchemaLoader {
    root: PathBuf,
    schemas: BTreeMap<PathBuf, Schema>,
    visiting: Vec<PathBuf>,
}

impl SchemaLoader {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_owned(),
            schemas: BTreeMap::new(),
            visiting: Vec::new(),
        }
    }

    /// Generate one module. `path` is relative to this loader's schema root.
    pub fn generate(&mut self, path: &Path, _config: Config) -> Result<TokenStream, Error> {
        let schema = self.resolve(path)?;
        let mut generator = SchemaGenerator {
            global_output: Vec::new(),
            user_type_registry: schema.registry,
        };
        for (name, definition) in &schema.local {
            generator.gen_user_type(name, definition);
        }
        let mut imports = Vec::new();
        for dependency in schema.dependencies {
            let name = module_name(&dependency)?;
            let ident: syn::Ident = syn::parse_str(&name)?;
            imports.push(quote! {
                #[allow(unused_imports)]
                use super::#ident::*;
            });
        }
        let definitions = generator.complete();
        Ok(quote! { #(#imports)* #definitions })
    }

    fn resolve(&mut self, path: &Path) -> Result<Schema, Error> {
        if path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
            || path.extension().and_then(|extension| extension.to_str()) != Some("bare")
        {
            return Err(format!(
                "schema path must be a root-relative .bare file: {}",
                path.display()
            )
            .into());
        }
        if let Some(schema) = self.schemas.get(path) {
            return Ok(schema.clone());
        }
        if let Some(start) = self.visiting.iter().position(|entry| entry == path) {
            let cycle = self.visiting[start..]
                .iter()
                .map(PathBuf::as_path)
                .chain(std::iter::once(path))
                .map(|entry| entry.display().to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(format!("schema import cycle: {cycle}").into());
        }
        self.visiting.push(path.to_owned());
        let result = self.load(path);
        self.visiting.pop();
        if let Ok(schema) = &result {
            self.schemas.insert(path.to_owned(), schema.clone());
        }
        result
    }

    fn load(&mut self, path: &Path) -> Result<Schema, Error> {
        let source = fs::read_to_string(self.root.join(path))
            .map_err(|error| format!("cannot read schema {}: {error}", path.display()))?;
        let imports =
            parser::imports(&source).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut registry = BTreeMap::new();
        let mut origins: BTreeMap<String, PathBuf> = BTreeMap::new();
        let mut dependencies = BTreeSet::new();
        for import in imports {
            let imported_path = PathBuf::from(&import);
            let imported = self
                .resolve(&imported_path)
                .map_err(|error| format!("{} imports {import}: {error}", path.display()))?;
            for (name, origin) in imported.origins {
                if let Some(previous) = origins.get(&name) {
                    if previous != &origin {
                        return Err(format!(
                            "{} imports conflicting type {name} from {} and {}",
                            path.display(),
                            previous.display(),
                            origin.display()
                        )
                        .into());
                    }
                }
                origins.insert(name, origin);
            }
            registry.extend(imported.registry);
            dependencies.insert(imported_path);
            dependencies.extend(imported.dependencies);
        }
        let local = parser::parse_string(&source, &registry).map_err(|error| {
            if let Some(origin) = error
                .split_once("duplicate or shadowed type ")
                .and_then(|(_, name)| origins.get(name))
            {
                format!(
                    "{}: {error}, imported from {}",
                    path.display(),
                    origin.display()
                )
            } else {
                format!("{}: {error}", path.display())
            }
        })?;
        for name in local.keys() {
            origins.insert(name.clone(), path.to_owned());
        }
        registry.extend(local.clone());
        validate::validate(&local, &registry)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(Schema {
            local,
            registry,
            origins,
            dependencies,
        })
    }
}

/// Strip `.bare` and replace each directory separator and dot with `_`.
/// For example, `shared/example.types.v1.bare` becomes `shared_example_types_v1`.
pub fn module_name(path: &Path) -> Result<String, Error> {
    let stem = path.with_extension("");
    let name = stem
        .components()
        .map(|part| {
            part.as_os_str()
                .to_str()
                .map(|part| part.replace('.', "_"))
                .ok_or_else(|| format!("schema path is not UTF-8: {}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("_");
    syn::parse_str::<syn::Ident>(&name)
        .map_err(|error| format!("invalid module name {name} for {}: {error}", path.display()))?;
    Ok(name)
}
