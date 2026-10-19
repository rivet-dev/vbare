//! Gives every anonymous struct, union and enum a name, since Rust has no anonymous aggregates.
//!
//! Names come from where the type appears: a struct field `orders` of `Customer` becomes
//! `CustomerOrders`, and union member `i` of `Person` becomes `PersonMember{i}`. Containers pass
//! their context through, except directly under a type definition, where `Item`, `Key` and
//! `Value` suffixes keep the hoisted name distinct from the definition itself.

use crate::parser::{AnyType, StructField};
use heck::ToUpperCamelCase;
use std::collections::BTreeSet;

/// A named type after hoisting. Its definition refers to other aggregates only by name.
#[derive(Clone, Debug)]
pub struct Definition {
    pub name: String,
    pub definition: AnyType,
    /// For a union, the indices of members that were hoisted out of it.
    pub hoisted_members: BTreeSet<usize>,
    /// Whether this definition was anonymous in the schema.
    pub hoisted: bool,
}

/// Hoists the anonymous aggregates of one user type. Returns the hoisted definitions followed by
/// the user type itself.
pub fn hoist(name: &str, definition: &AnyType) -> Vec<Definition> {
    let mut out = Vec::new();
    let (definition, hoisted_members) = hoist_top(name, definition, &mut out);
    out.push(Definition {
        name: name.to_owned(),
        definition,
        hoisted_members,
        hoisted: false,
    });
    out
}

fn hoist_top(
    name: &str,
    definition: &AnyType,
    out: &mut Vec<Definition>,
) -> (AnyType, BTreeSet<usize>) {
    let mut hoisted_members = BTreeSet::new();
    let definition = match definition {
        AnyType::Struct(fields) => AnyType::Struct(hoist_fields(name, fields, out)),
        AnyType::Union(members) => {
            let members = members
                .iter()
                .enumerate()
                .map(|(index, (member, tag))| {
                    if is_aggregate(member) {
                        hoisted_members.insert(index);
                    }
                    let context = format!("{name}Member{index}");
                    (hoist_nested(member, &context, out), *tag)
                })
                .collect();
            AnyType::Union(members)
        }
        AnyType::Enum(_) | AnyType::Primitive(_) | AnyType::TypeReference(_) => definition.clone(),
        AnyType::List { inner, length } => AnyType::List {
            inner: Box::new(hoist_nested(inner, &format!("{name}Item"), out)),
            length: *length,
        },
        AnyType::Optional(inner) => {
            AnyType::Optional(Box::new(hoist_nested(inner, &format!("{name}Value"), out)))
        }
        AnyType::Map { key, value } => AnyType::Map {
            key: Box::new(hoist_nested(key, &format!("{name}Key"), out)),
            value: Box::new(hoist_nested(value, &format!("{name}Value"), out)),
        },
    };
    (definition, hoisted_members)
}

fn hoist_fields(
    parent: &str,
    fields: &[StructField],
    out: &mut Vec<Definition>,
) -> Vec<StructField> {
    fields
        .iter()
        .map(|field| StructField {
            name: field.name.clone(),
            type_r: hoist_nested(
                &field.type_r,
                &format!("{parent}{}", field.name.to_upper_camel_case()),
                out,
            ),
        })
        .collect()
}

/// Replaces an aggregate nested under `context` with a reference to a new definition.
fn hoist_nested(definition: &AnyType, context: &str, out: &mut Vec<Definition>) -> AnyType {
    match definition {
        AnyType::Struct(_) | AnyType::Union(_) | AnyType::Enum(_) => {
            let (hoisted, hoisted_members) = hoist_top(context, definition, out);
            out.push(Definition {
                name: context.to_owned(),
                definition: hoisted,
                hoisted_members,
                hoisted: true,
            });
            AnyType::TypeReference(context.to_owned())
        }
        AnyType::List { inner, length } => AnyType::List {
            inner: Box::new(hoist_nested(inner, context, out)),
            length: *length,
        },
        AnyType::Optional(inner) => AnyType::Optional(Box::new(hoist_nested(inner, context, out))),
        AnyType::Map { key, value } => {
            // Only enums can be inline keys; give them their own name so a value aggregate can
            // use the context name.
            AnyType::Map {
                key: Box::new(hoist_nested(key, &format!("{context}Key"), out)),
                value: Box::new(hoist_nested(value, context, out)),
            }
        }
        AnyType::Primitive(_) | AnyType::TypeReference(_) => definition.clone(),
    }
}

fn is_aggregate(definition: &AnyType) -> bool {
    match definition {
        AnyType::Struct(_) | AnyType::Union(_) | AnyType::Enum(_) => true,
        AnyType::Primitive(_)
        | AnyType::TypeReference(_)
        | AnyType::List { .. }
        | AnyType::Optional(_)
        | AnyType::Map { .. } => false,
    }
}
