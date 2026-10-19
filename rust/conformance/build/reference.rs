//! The schema copy compiled by `bare_proc` 0.2.0 and the adapter converting its private types to
//! and from candidate types.
//!
//! The copy is printed from the candidate's view of the schema, in which the generator has
//! already named every anonymous struct, union and enum, so `bare_proc` never names anonymous
//! types itself. Every other rewrite preserves the wire format:
//!
//! - imports are inlined, each type once, in dependency order;
//! - the file uses spaces and ends with a newline (`bare_proc`'s `COMMENT` rule needs one);
//! - `f32` and `f64` become `u32` and `u64`, because `bare_proc` derives `Eq`;
//! - union members that are not named types or single-identifier primitives become synthetic
//!   named types, because `bare_proc` names variants after the member's Rust type;
//! - a top-level alias of another user type takes that type's definition, because `bare_proc`
//!   panics on it;
//! - a top-level `optional<T>` becomes a struct with one `value: optional<T>` field, because
//!   `bare_proc` generates `type X = T` for it and drops the `Option` (a struct with one field
//!   encodes exactly like the field);
//! - explicit enum values are dropped (`bare_proc` encodes the variant index regardless);
//! - field and enum value names use the candidate's Rust names, which are valid identifiers.

use crate::model::{Error, FileModel};
use std::fmt::Write;
use vbare_gen::{
    ast::{AnyType, PrimitiveType, StructField},
    naming::{rust_enum_variant_name, rust_field_name, rust_union_variant_name},
};

const MAX_ARRAY_LEN: usize = 32;

const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

/// The reference field name: the candidate's snake_case name, with keywords suffixed because
/// `bare_proc` creates identifiers with `Ident::new`.
fn reference_field(name: &str) -> Result<String, Error> {
    let candidate = rust_field_name(name)?;
    let plain = candidate.trim_start_matches("r#").to_owned();
    if KEYWORDS.contains(&plain.as_str()) {
        Ok(format!("{plain}_"))
    } else {
        Ok(plain)
    }
}

/// The variant `bare_proc` emits for a union member that stays inline: the member's Rust type.
fn inline_variant(member: &AnyType) -> Option<String> {
    Some(match member {
        AnyType::TypeReference(name) => name.clone(),
        AnyType::Primitive(primitive) => match primitive {
            PrimitiveType::UInt => "Uint".into(),
            PrimitiveType::Int => "Int".into(),
            PrimitiveType::U64 => "u64".into(),
            PrimitiveType::U32 => "u32".into(),
            PrimitiveType::U16 => "u16".into(),
            PrimitiveType::U8 => "u8".into(),
            PrimitiveType::I64 => "i64".into(),
            PrimitiveType::I32 => "i32".into(),
            PrimitiveType::I16 => "i16".into(),
            PrimitiveType::I8 => "i8".into(),
            PrimitiveType::Bool => "bool".into(),
            PrimitiveType::Str => "String".into(),
            // Floats are rewritten, data types are not single identifiers, and a literal void
            // would become `()(())`.
            PrimitiveType::F32
            | PrimitiveType::F64
            | PrimitiveType::Data(_)
            | PrimitiveType::Void => return None,
        },
        AnyType::List { .. }
        | AnyType::Map { .. }
        | AnyType::Optional(_)
        | AnyType::Struct(_)
        | AnyType::Union(_)
        | AnyType::Enum(_) => return None,
    })
}

fn reference_variant(union: &str, member: &AnyType, index: usize) -> String {
    inline_variant(member).unwrap_or_else(|| format!("{union}Member{index}"))
}

fn print_type(model: &FileModel, definition: &AnyType) -> Result<String, Error> {
    Ok(match definition {
        AnyType::Primitive(primitive) => match primitive {
            PrimitiveType::UInt => "uint".into(),
            PrimitiveType::U64 | PrimitiveType::F64 => "u64".into(),
            PrimitiveType::U32 | PrimitiveType::F32 => "u32".into(),
            PrimitiveType::U16 => "u16".into(),
            PrimitiveType::U8 => "u8".into(),
            PrimitiveType::Int => "int".into(),
            PrimitiveType::I64 => "i64".into(),
            PrimitiveType::I32 => "i32".into(),
            PrimitiveType::I16 => "i16".into(),
            PrimitiveType::I8 => "i8".into(),
            PrimitiveType::Str => "str".into(),
            PrimitiveType::Data(None) => "data".into(),
            PrimitiveType::Data(Some(length)) => format!("data[{length}]"),
            PrimitiveType::Void => "void".into(),
            PrimitiveType::Bool => "bool".into(),
        },
        AnyType::List { inner, length } => {
            let inner = print_type(model, inner)?;
            match length {
                Some(length) => format!("list<{inner}>[{length}]"),
                None => format!("list<{inner}>"),
            }
        }
        AnyType::Map { key, value } => {
            if matches!(
                model.resolve(key),
                AnyType::Primitive(PrimitiveType::UInt | PrimitiveType::Int) | AnyType::Enum(_)
            ) {
                return Err(format!(
                    "{}: map keys of uint, int or enum type do not compile with bare_proc \
                     (serde_bare::Uint, Int and generated enums lack Hash); move the schema to a \
                     candidate-only set",
                    model.path.display()
                )
                .into());
            }
            format!(
                "map<{}><{}>",
                print_type(model, key)?,
                print_type(model, value)?
            )
        }
        AnyType::Optional(inner) => format!("optional<{}>", print_type(model, inner)?),
        AnyType::TypeReference(name) => name.clone(),
        AnyType::Struct(_) | AnyType::Union(_) | AnyType::Enum(_) => {
            return Err("anonymous aggregate after hoisting".into())
        }
    })
}

/// Prints one named definition, preceded by any synthetic union member types it needs.
fn print_definition(model: &FileModel, name: &str, out: &mut String) -> Result<(), Error> {
    let target = model.resolve_name(name);
    let definition = &model.defs[target].definition;
    match definition {
        AnyType::Struct(fields) => {
            writeln!(out, "type {name} struct {{")?;
            for field in fields {
                writeln!(
                    out,
                    "  {}: {}",
                    reference_field(&field.name)?,
                    print_type(model, &field.type_r)?
                )?;
            }
            writeln!(out, "}}")?;
        }
        AnyType::Union(members) => {
            let mut variants = Vec::new();
            for (index, (member, _)) in members.iter().enumerate() {
                match inline_variant(member) {
                    Some(_) => variants.push(print_type(model, member)?),
                    None => {
                        let synthetic = format!("{name}Member{index}");
                        writeln!(out, "type {synthetic} {}", print_type(model, member)?)?;
                        variants.push(synthetic);
                    }
                }
            }
            writeln!(out, "type {name} union {{ {} }}", variants.join(" | "))?;
        }
        AnyType::Enum(values) => {
            writeln!(out, "type {name} enum {{")?;
            for (value, _) in values {
                writeln!(out, "  {}", rust_enum_variant_name(value)?)?;
            }
            writeln!(out, "}}")?;
        }
        AnyType::Optional(_) => {
            writeln!(
                out,
                "type {name} struct {{\n  value: {}\n}}",
                print_type(model, definition)?
            )?;
        }
        AnyType::Primitive(_)
        | AnyType::TypeReference(_)
        | AnyType::List { .. }
        | AnyType::Map { .. } => {
            writeln!(out, "type {name} {}", print_type(model, definition)?)?;
        }
    }
    Ok(())
}

/// Converts a candidate value reference `value` to the reference type.
fn to_reference(definition: &AnyType, value: &str, depth: usize) -> Result<String, Error> {
    let x = format!("x{depth}");
    let k = format!("k{depth}");
    Ok(match definition {
        AnyType::Primitive(primitive) => match primitive {
            PrimitiveType::UInt => format!("serde_bare::Uint(*{value})"),
            PrimitiveType::Int => format!("serde_bare::Int(*{value})"),
            PrimitiveType::F32 | PrimitiveType::F64 => format!("{value}.to_bits()"),
            PrimitiveType::Str => format!("{value}.as_str().to_owned()"),
            PrimitiveType::Data(None) => format!("{value}.to_vec()"),
            PrimitiveType::Data(Some(length)) if *length <= MAX_ARRAY_LEN => format!("*{value}"),
            PrimitiveType::Data(Some(_)) => format!("{value}.as_bytes().to_vec()"),
            PrimitiveType::Void => "()".into(),
            PrimitiveType::U64
            | PrimitiveType::U32
            | PrimitiveType::U16
            | PrimitiveType::U8
            | PrimitiveType::I64
            | PrimitiveType::I32
            | PrimitiveType::I16
            | PrimitiveType::I8
            | PrimitiveType::Bool => format!("*{value}"),
        },
        AnyType::Optional(inner) => format!(
            "{value}.as_ref().map(|{x}| {})",
            to_reference(inner, &x, depth + 1)?
        ),
        AnyType::List {
            inner,
            length: Some(length),
        } if *length <= MAX_ARRAY_LEN => format!(
            "{value}.each_ref().map(|{x}| {})",
            to_reference(inner, &x, depth + 1)?
        ),
        AnyType::List { inner, .. } => format!(
            "{value}.iter().map(|{x}| {}).collect::<Vec<_>>()",
            to_reference(inner, &x, depth + 1)?
        ),
        AnyType::Map { key, value: inner } => format!(
            "{value}.iter().map(|({k}, {x})| ({}, {})).collect::<std::collections::HashMap<_, _>>()",
            to_reference(key, &k, depth + 1)?,
            to_reference(inner, &x, depth + 1)?
        ),
        AnyType::TypeReference(name) => format!("__to_{name}({value})"),
        AnyType::Struct(_) | AnyType::Union(_) | AnyType::Enum(_) => {
            return Err("anonymous aggregate after hoisting".into())
        }
    })
}

/// Converts an owned reference value `value` to the candidate type.
fn from_reference(definition: &AnyType, value: &str, depth: usize) -> Result<String, Error> {
    let x = format!("x{depth}");
    let k = format!("k{depth}");
    Ok(match definition {
        AnyType::Primitive(primitive) => match primitive {
            PrimitiveType::UInt | PrimitiveType::Int => format!("{value}.0"),
            PrimitiveType::F32 => format!("f32::from_bits({value})"),
            PrimitiveType::F64 => format!("f64::from_bits({value})"),
            PrimitiveType::Str => format!("vbare::Str::from({value})"),
            PrimitiveType::Data(None) => format!("vbare::Bytes::from({value})"),
            PrimitiveType::Data(Some(length)) if *length <= MAX_ARRAY_LEN => value.to_owned(),
            PrimitiveType::Data(Some(_)) => format!(
                "vbare::FixedBytes::new(vbare::Bytes::from({value})).expect(\"reference data[N] has N bytes\")"
            ),
            PrimitiveType::Void
            | PrimitiveType::U64
            | PrimitiveType::U32
            | PrimitiveType::U16
            | PrimitiveType::U8
            | PrimitiveType::I64
            | PrimitiveType::I32
            | PrimitiveType::I16
            | PrimitiveType::I8
            | PrimitiveType::Bool => value.to_owned(),
        },
        AnyType::Optional(inner) => format!(
            "{value}.map(|{x}| {})",
            from_reference(inner, &x, depth + 1)?
        ),
        AnyType::List {
            inner,
            length: Some(length),
        } if *length <= MAX_ARRAY_LEN => format!(
            "{value}.map(|{x}| {})",
            from_reference(inner, &x, depth + 1)?
        ),
        AnyType::List {
            inner,
            length: Some(length),
        } => format!(
            "<[_; {length}]>::try_from({value}.into_iter().map(|{x}| {}).collect::<Vec<_>>()).ok().expect(\"reference list[N] has N elements\")",
            from_reference(inner, &x, depth + 1)?
        ),
        AnyType::List {
            inner,
            length: None,
        } => format!(
            "{value}.into_iter().map(|{x}| {}).collect::<Vec<_>>()",
            from_reference(inner, &x, depth + 1)?
        ),
        AnyType::Map { key, value: inner } => format!(
            "{value}.into_iter().map(|({k}, {x})| ({}, {})).collect::<std::collections::BTreeMap<_, _>>()",
            from_reference(key, &k, depth + 1)?,
            from_reference(inner, &x, depth + 1)?
        ),
        AnyType::TypeReference(name) => format!("__from_{name}({value})"),
        AnyType::Struct(_) | AnyType::Union(_) | AnyType::Enum(_) => {
            return Err("anonymous aggregate after hoisting".into())
        }
    })
}

fn struct_conversions(
    name: &str,
    candidate: &str,
    fields: &[StructField],
) -> Result<(String, String), Error> {
    let mut to = Vec::new();
    let mut from = Vec::new();
    for field in fields {
        let candidate_field = rust_field_name(&field.name)?;
        let reference = reference_field(&field.name)?;
        to.push(format!(
            "{reference}: {}",
            to_reference(&field.type_r, &format!("(&value.{candidate_field})"), 0)?
        ));
        from.push(format!(
            "{candidate_field}: {}",
            from_reference(&field.type_r, &format!("value.{reference}"), 0)?
        ));
    }
    Ok((
        format!("{name} {{ {} }}", to.join(", ")),
        format!("{candidate} {{ {} }}", from.join(", ")),
    ))
}

/// Generates the reference schema copy and the adapter module source for one file.
pub fn generate(model: &FileModel, copy_path: &str) -> Result<(String, String), Error> {
    let names: Vec<String> = model.defs.keys().cloned().collect();
    let ordered = model.topological(&names);

    let mut schema = format!(
        "# Reference copy of {} for bare_proc 0.2.0. Generated by the conformance build script.\n",
        model.path.display()
    );
    for name in &ordered {
        print_definition(model, name, &mut schema)?;
    }

    let mut adapter = format!("bare_proc::bare_schema!({copy_path:?});\n");
    for name in &ordered {
        let candidate = model.candidate_path(name);
        let target = model.resolve_name(name);
        let definition = &model.defs[target];
        let (to, from) = match &definition.definition {
            AnyType::Struct(fields) => struct_conversions(name, &candidate, fields)?,
            AnyType::Union(members) => {
                let mut to_arms = Vec::new();
                let mut from_arms = Vec::new();
                for (index, (member, _)) in members.iter().enumerate() {
                    let candidate_variant = rust_union_variant_name(
                        member,
                        index,
                        definition.hoisted_members.contains(&index),
                    );
                    let reference_variant = reference_variant(name, member, index);
                    if model.is_void(member) {
                        to_arms.push(format!(
                            "{candidate}::{candidate_variant} => {name}::{reference_variant}"
                        ));
                        from_arms.push(format!(
                            "{name}::{reference_variant} => {candidate}::{candidate_variant}"
                        ));
                    } else {
                        to_arms.push(format!(
                            "{candidate}::{candidate_variant}(member) => {name}::{reference_variant}({})",
                            to_reference(member, "member", 0)?
                        ));
                        from_arms.push(format!(
                            "{name}::{reference_variant}(member) => {candidate}::{candidate_variant}({})",
                            from_reference(member, "member", 0)?
                        ));
                    }
                }
                (
                    format!("match value {{ {} }}", to_arms.join(", ")),
                    format!("match value {{ {} }}", from_arms.join(", ")),
                )
            }
            AnyType::Enum(values) => {
                let mut to_arms = Vec::new();
                let mut from_arms = Vec::new();
                for (value, _) in values {
                    let variant = rust_enum_variant_name(value)?;
                    to_arms.push(format!("{candidate}::{variant} => {name}::{variant}"));
                    from_arms.push(format!("{name}::{variant} => {candidate}::{variant}"));
                }
                (
                    format!("match value {{ {} }}", to_arms.join(", ")),
                    format!("match value {{ {} }}", from_arms.join(", ")),
                )
            }
            AnyType::Optional(_) => (
                format!(
                    "{name} {{ value: {} }}",
                    to_reference(&definition.definition, "value", 0)?
                ),
                from_reference(&definition.definition, "value.value", 0)?,
            ),
            AnyType::Primitive(_)
            | AnyType::TypeReference(_)
            | AnyType::List { .. }
            | AnyType::Map { .. } => (
                to_reference(&definition.definition, "value", 0)?,
                from_reference(&definition.definition, "value", 0)?,
            ),
        };
        let to_parameter = if to.contains("value") {
            "value"
        } else {
            "_value"
        };
        let from_parameter = if from.contains("value") {
            "value"
        } else {
            "_value"
        };
        writeln!(
            adapter,
            "fn __to_{name}({to_parameter}: &{candidate}) -> {name} {{ {to} }}\n\
             fn __from_{name}({from_parameter}: {name}) -> {candidate} {{ {from} }}"
        )?;
    }

    for name in &model.local {
        let candidate = model.candidate_path(name);
        let case = model.item_path("cases", name);
        writeln!(
            adapter,
            "pub struct __Opaque_{name}({name});
impl crate::harness::ReferenceCase for {case} {{
    type RefValue = __Opaque_{name};
    fn to_ref(value: &{candidate}) -> __Opaque_{name} {{ __Opaque_{name}(__to_{name}(value)) }}
    fn from_ref(value: __Opaque_{name}) -> {candidate} {{ __from_{name}(value.0) }}
    fn ref_to_vec(value: &__Opaque_{name}) -> Vec<u8> {{ serde_bare::to_vec(&value.0).expect(\"serde_bare encodes every value\") }}
    fn ref_from_slice(bytes: &[u8]) -> Result<__Opaque_{name}, serde_bare::error::Error> {{ serde_bare::from_slice::<{name}>(bytes).map(__Opaque_{name}) }}
}}"
        )?;
    }

    Ok((schema, adapter))
}
