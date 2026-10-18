//! Schema invariants from draft-devault-bare-11 section 2.4, checked after parsing and import
//! resolution.
//!
//! The decoder relies on these invariants. In particular, because void only appears inside a
//! union (which always writes a tag byte) and structs have at least one field, every encoded list
//! element and map pair occupies at least one byte, so a list or map count can never exceed the
//! remaining input.

use crate::parser::{AnyType, PrimitiveType};
use std::collections::{BTreeMap, BTreeSet};

/// Checks every type in `local` against the spec invariants. `registry` must contain every type
/// `local` can reference, including `local` itself.
pub fn validate(
    local: &BTreeMap<String, AnyType>,
    registry: &BTreeMap<String, AnyType>,
) -> Result<(), String> {
    let validator = Validator { registry };
    for (name, definition) in local {
        validator.check_recursion(name, &mut Vec::new())?;
        validator
            .check(definition, Position::TopLevel)
            .map_err(|problem| format!("type {name}: {problem}"))?;
    }
    Ok(())
}

/// Where a type appears, which decides whether void is allowed there.
#[derive(Clone, Copy)]
enum Position {
    TopLevel,
    UnionMember,
    Optional,
    ListElement,
    MapKey,
    MapValue,
    StructField,
}

impl Position {
    fn describe(self) -> &'static str {
        match self {
            Position::TopLevel => "a type definition",
            Position::UnionMember => "a union member",
            Position::Optional => "an optional type",
            Position::ListElement => "a list element",
            Position::MapKey => "a map key",
            Position::MapValue => "a map value",
            Position::StructField => "a struct field",
        }
    }
}

struct Validator<'a> {
    registry: &'a BTreeMap<String, AnyType>,
}

impl Validator<'_> {
    /// Follows type references to the underlying definition. Returns `None` for an unknown name,
    /// which the parser already rejects.
    fn resolve<'t>(&'t self, mut definition: &'t AnyType) -> Option<&'t AnyType> {
        let mut steps = 0;
        while let AnyType::TypeReference(name) = definition {
            definition = self.registry.get(name)?;
            // A reference chain longer than the registry must loop. `check_recursion` reports it.
            steps += 1;
            if steps > self.registry.len() {
                return None;
            }
        }
        Some(definition)
    }

    fn is_void(&self, definition: &AnyType) -> bool {
        matches!(
            self.resolve(definition),
            Some(AnyType::Primitive(PrimitiveType::Void))
        )
    }

    fn check(&self, definition: &AnyType, position: Position) -> Result<(), String> {
        if self.is_void(definition)
            && !matches!(position, Position::TopLevel | Position::UnionMember)
        {
            return Err(format!(
                "void may only be a union member, not {}",
                position.describe()
            ));
        }
        if let Position::MapKey = position {
            self.check_map_key(definition)?;
        }

        match definition {
            AnyType::Primitive(PrimitiveType::Data(Some(0))) => {
                Err("fixed-length data must have a length of at least 1".into())
            }
            AnyType::Primitive(_) => Ok(()),
            AnyType::TypeReference(name) => {
                if self.registry.contains_key(name) {
                    Ok(())
                } else {
                    Err(format!("unknown type {name}"))
                }
            }
            AnyType::Optional(inner) => self.check(inner, Position::Optional),
            AnyType::List { inner, length } => {
                if *length == Some(0) {
                    return Err("fixed-length list must have a length of at least 1".into());
                }
                self.check(inner, Position::ListElement)
            }
            AnyType::Map { key, value } => {
                self.check(key, Position::MapKey)?;
                self.check(value, Position::MapValue)
            }
            AnyType::Enum(values) => {
                if values.is_empty() {
                    return Err("enum must have at least one value".into());
                }
                let mut names = BTreeSet::new();
                let mut numbers = BTreeSet::new();
                for (name, value) in values {
                    if !names.insert(name) {
                        return Err(format!("duplicate enum value name {name}"));
                    }
                    if !numbers.insert(value) {
                        return Err(format!("duplicate enum value {value} for {name}"));
                    }
                }
                Ok(())
            }
            AnyType::Union(members) => {
                if members.is_empty() {
                    return Err("union must have at least one member".into());
                }
                let mut types = BTreeSet::new();
                let mut tags = BTreeSet::new();
                for (member, tag) in members {
                    // Distinct user types are distinct even with identical definitions, so
                    // members are compared as written.
                    if !types.insert(format!("{member:?}")) {
                        return Err(format!("duplicate union member {member:?}"));
                    }
                    if !tags.insert(tag) {
                        return Err(format!("duplicate union tag {tag}"));
                    }
                    self.check(member, Position::UnionMember)
                        .map_err(|problem| format!("union member {tag}: {problem}"))?;
                }
                Ok(())
            }
            AnyType::Struct(fields) => {
                if fields.is_empty() {
                    return Err("struct must have at least one field".into());
                }
                let mut names = BTreeSet::new();
                for field in fields {
                    if !names.insert(&field.name) {
                        return Err(format!("duplicate struct field {}", field.name));
                    }
                    self.check(&field.type_r, Position::StructField)
                        .map_err(|problem| format!("field {}: {problem}", field.name))?;
                }
                Ok(())
            }
        }
    }

    /// Map keys must be primitive types other than `f32`, `f64`, `data`, `data[N]` and `void`.
    /// Enums count as primitive types (spec section 2.1).
    fn check_map_key(&self, key: &AnyType) -> Result<(), String> {
        let allowed = match self.resolve(key) {
            Some(AnyType::Primitive(primitive)) => match primitive {
                PrimitiveType::UInt
                | PrimitiveType::U64
                | PrimitiveType::U32
                | PrimitiveType::U16
                | PrimitiveType::U8
                | PrimitiveType::Int
                | PrimitiveType::I64
                | PrimitiveType::I32
                | PrimitiveType::I16
                | PrimitiveType::I8
                | PrimitiveType::Str
                | PrimitiveType::Bool => true,
                PrimitiveType::F64
                | PrimitiveType::F32
                | PrimitiveType::Data(_)
                | PrimitiveType::Void => false,
            },
            Some(AnyType::Enum(_)) => true,
            Some(
                AnyType::List { .. }
                | AnyType::Struct(_)
                | AnyType::Map { .. }
                | AnyType::Union(_)
                | AnyType::Optional(_)
                | AnyType::TypeReference(_),
            )
            | None => false,
        };
        if allowed {
            Ok(())
        } else {
            Err(format!(
                "map key {key:?} must be a primitive type other than f32, f64, data or void"
            ))
        }
    }

    /// Define-before-use already rules out recursion, but generated code would not terminate on a
    /// recursive type, so the invariant is checked directly.
    fn check_recursion(&self, name: &str, stack: &mut Vec<String>) -> Result<(), String> {
        if stack.iter().any(|entry| entry == name) {
            stack.push(name.to_owned());
            return Err(format!("recursive type: {}", stack.join(" -> ")));
        }
        let Some(definition) = self.registry.get(name) else {
            return Ok(());
        };
        stack.push(name.to_owned());
        let mut references = Vec::new();
        collect_references(definition, &mut references);
        for reference in references {
            self.check_recursion(reference, stack)?;
        }
        stack.pop();
        Ok(())
    }
}

fn collect_references<'a>(definition: &'a AnyType, out: &mut Vec<&'a str>) {
    match definition {
        AnyType::Primitive(_) | AnyType::Enum(_) => {}
        AnyType::TypeReference(name) => out.push(name),
        AnyType::Optional(inner) | AnyType::List { inner, .. } => collect_references(inner, out),
        AnyType::Map { key, value } => {
            collect_references(key, out);
            collect_references(value, out);
        }
        AnyType::Union(members) => {
            for (member, _) in members {
                collect_references(member, out);
            }
        }
        AnyType::Struct(fields) => {
            for field in fields {
                collect_references(&field.type_r, out);
            }
        }
    }
}
