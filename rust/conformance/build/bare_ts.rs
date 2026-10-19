//! The schema copy compiled by bare-ts.
//!
//! Imports are inlined, field names become camelCase (as `typescript/vbare-compiler` does), enum
//! value names become CONSTANT_CASE, and every enum value and union tag is written explicitly.
//! Names are informational, so none of this changes the wire format. Type names are kept and
//! must already be PascalCase.
//!
//! bare-ts 0.16 (and 0.19) omits the tag when encoding an unnamed member of a union that also
//! has named members. Such members are moved into synthetic named types, which preserves the
//! wire format.

use crate::model::{references_of, Error, FileModel};
use heck::ToShoutySnakeCase;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
use vbare_gen::ast::{AnyType, PrimitiveType};

/// The same conversion `typescript/vbare-compiler` applies to field names.
pub fn camel_case(name: &str) -> String {
    let mut out = String::new();
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            if let Some(next) = chars.peek().copied() {
                if next.is_ascii_alphanumeric() {
                    out.push(next.to_ascii_uppercase());
                    chars.next();
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

fn is_pascal_case(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

fn is_camel_case(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

struct Printer<'a> {
    model: &'a FileModel,
    /// Synthetic types the current definition needs, printed before it.
    pending: Vec<String>,
    names: BTreeSet<String>,
}

impl Printer<'_> {
    fn print(&mut self, definition: &AnyType, context: &str) -> Result<String, Error> {
        Ok(match definition {
            AnyType::Primitive(primitive) => match primitive {
                PrimitiveType::UInt => "uint".into(),
                PrimitiveType::U64 => "u64".into(),
                PrimitiveType::U32 => "u32".into(),
                PrimitiveType::U16 => "u16".into(),
                PrimitiveType::U8 => "u8".into(),
                PrimitiveType::Int => "int".into(),
                PrimitiveType::I64 => "i64".into(),
                PrimitiveType::I32 => "i32".into(),
                PrimitiveType::I16 => "i16".into(),
                PrimitiveType::I8 => "i8".into(),
                PrimitiveType::F64 => "f64".into(),
                PrimitiveType::F32 => "f32".into(),
                PrimitiveType::Str => "str".into(),
                PrimitiveType::Data(None) => "data".into(),
                PrimitiveType::Data(Some(length)) => format!("data[{length}]"),
                PrimitiveType::Void => "void".into(),
                PrimitiveType::Bool => "bool".into(),
            },
            AnyType::List { inner, length } => {
                let inner = self.print(inner, context)?;
                match length {
                    Some(length) => format!("list<{inner}>[{length}]"),
                    None => format!("list<{inner}>"),
                }
            }
            AnyType::Map { key, value } => format!(
                "map<{}><{}>",
                self.print(key, context)?,
                self.print(value, context)?
            ),
            AnyType::Optional(inner) => format!("optional<{}>", self.print(inner, context)?),
            AnyType::TypeReference(name) => name.clone(),
            AnyType::Struct(fields) => {
                let mut out = String::from("struct {\n");
                for field in fields {
                    let name = camel_case(&field.name);
                    if !is_camel_case(&name) {
                        return Err(format!(
                            "{}: field {} is not camelCase after conversion, which bare-ts requires",
                            self.model.path.display(),
                            field.name
                        )
                        .into());
                    }
                    let field_context = format!("{context}{}", upper_first(&name));
                    writeln!(
                        out,
                        "  {name}: {}",
                        self.print(&field.type_r, &field_context)?
                    )?;
                }
                out.push('}');
                out
            }
            AnyType::Enum(values) => {
                let mut out = String::from("enum {\n");
                for (name, value) in values {
                    writeln!(out, "  {} = {value}", name.to_shouty_snake_case())?;
                }
                out.push('}');
                out
            }
            AnyType::Union(members) => {
                let named = members
                    .iter()
                    .filter(|(member, _)| matches!(member, AnyType::TypeReference(_)))
                    .count();
                let mixed = named > 0 && named < members.len();
                let mut variants = Vec::new();
                for (index, (member, tag)) in members.iter().enumerate() {
                    let member_context = format!("{context}Member{index}");
                    let printed = if mixed && !matches!(member, AnyType::TypeReference(_)) {
                        let body = self.print(member, &member_context)?;
                        self.define(&member_context, &body)?;
                        member_context
                    } else {
                        self.print(member, &member_context)?
                    };
                    variants.push(format!("{printed} = {tag}"));
                }
                format!("union {{ {} }}", variants.join(" | "))
            }
        })
    }

    fn define(&mut self, name: &str, body: &str) -> Result<(), Error> {
        if !is_pascal_case(name) {
            return Err(format!(
                "{}: type name {name} is not PascalCase, which bare-ts requires",
                self.model.path.display()
            )
            .into());
        }
        if !self.names.insert(name.to_owned()) {
            return Err(format!(
                "{}: duplicate bare-ts type {name}",
                self.model.path.display()
            )
            .into());
        }
        self.pending.push(format!("type {name} {body}\n"));
        Ok(())
    }
}

fn upper_first(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// Prints the bare-ts copy of one file and its import closure.
pub fn generate(model: &FileModel) -> Result<String, Error> {
    let registry = &model.resolved.registry;
    let mut order = Vec::new();
    let mut seen = BTreeSet::new();
    for name in registry.keys() {
        visit(registry, name, &mut seen, &mut order);
    }

    let mut printer = Printer {
        model,
        pending: Vec::new(),
        names: registry.keys().cloned().collect(),
    };
    let mut out = format!(
        "# bare-ts copy of {}. Generated by the conformance build script.\n",
        model.path.display()
    );
    for name in &order {
        if !is_pascal_case(name) {
            return Err(format!(
                "{}: type name {name} is not PascalCase, which bare-ts requires",
                model.path.display()
            )
            .into());
        }
        let body = printer.print(&registry[name], name)?;
        for pending in printer.pending.drain(..) {
            out.push_str(&pending);
        }
        writeln!(out, "type {name} {body}")?;
    }
    Ok(out)
}

fn visit(
    registry: &BTreeMap<String, AnyType>,
    name: &str,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<String>,
) {
    if !seen.insert(name.to_owned()) {
        return;
    }
    if let Some(definition) = registry.get(name) {
        let mut references = Vec::new();
        references_of(definition, &mut references);
        for reference in references {
            visit(registry, &reference, seen, out);
        }
        out.push(name.to_owned());
    }
}
