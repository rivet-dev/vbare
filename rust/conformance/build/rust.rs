//! Descriptors, proptest strategies and harness cases for each file's local types.

use crate::model::{Error, FileModel};
use std::fmt::Write;
use vbare_gen::{
    ast::{AnyType, PrimitiveType},
    naming::{rust_enum_variant_name, rust_field_name, rust_union_variant_name},
};

const MAX_INLINE_DATA_LEN: usize = 32;

/// An expression of type `&'static Desc` for a type expression.
fn desc_ref(model: &FileModel, definition: &AnyType) -> Result<String, Error> {
    let desc = "crate::desc::Desc";
    Ok(match definition {
        AnyType::Primitive(primitive) => {
            let variant = match primitive {
                PrimitiveType::UInt => "Uint".to_owned(),
                PrimitiveType::U64 => "U64".to_owned(),
                PrimitiveType::U32 => "U32".to_owned(),
                PrimitiveType::U16 => "U16".to_owned(),
                PrimitiveType::U8 => "U8".to_owned(),
                PrimitiveType::Int => "Int".to_owned(),
                PrimitiveType::I64 => "I64".to_owned(),
                PrimitiveType::I32 => "I32".to_owned(),
                PrimitiveType::I16 => "I16".to_owned(),
                PrimitiveType::I8 => "I8".to_owned(),
                PrimitiveType::F64 => "F64".to_owned(),
                PrimitiveType::F32 => "F32".to_owned(),
                PrimitiveType::Str => "Str".to_owned(),
                PrimitiveType::Data(None) => "Data".to_owned(),
                PrimitiveType::Data(Some(length)) => format!("FixedData({length})"),
                PrimitiveType::Void => "Void".to_owned(),
                PrimitiveType::Bool => "Bool".to_owned(),
            };
            format!("&{desc}::{variant}")
        }
        AnyType::Optional(inner) => format!("&{desc}::Optional({})", desc_ref(model, inner)?),
        AnyType::List {
            inner,
            length: None,
        } => format!("&{desc}::List({})", desc_ref(model, inner)?),
        AnyType::List {
            inner,
            length: Some(length),
        } => format!("&{desc}::FixedList({}, {length})", desc_ref(model, inner)?),
        AnyType::Map { key, value } => format!(
            "&{desc}::Map({}, {})",
            desc_ref(model, key)?,
            desc_ref(model, value)?
        ),
        AnyType::TypeReference(name) => model.item_path("desc", name),
        AnyType::Enum(values) => {
            let values: Vec<String> = values
                .iter()
                .map(|(name, value)| format!("({name:?}, {value})"))
                .collect();
            format!("&{desc}::Enum(&[{}])", values.join(", "))
        }
        AnyType::Union(members) => {
            let members: Vec<String> = members
                .iter()
                .map(|(member, tag)| Ok(format!("({tag}, {})", desc_ref(model, member)?)))
                .collect::<Result<_, Error>>()?;
            format!("&{desc}::Union(&[{}])", members.join(", "))
        }
        AnyType::Struct(fields) => {
            let fields: Vec<String> = fields
                .iter()
                .map(|field| {
                    Ok(format!(
                        "({:?}, {})",
                        field.name,
                        desc_ref(model, &field.type_r)?
                    ))
                })
                .collect::<Result<_, Error>>()?;
            format!("&{desc}::Struct(&[{}])", fields.join(", "))
        }
    })
}

/// An expression of type `BoxedStrategy<Owned>` for a type expression.
fn strategy(model: &FileModel, definition: &AnyType) -> Result<String, Error> {
    let s = "crate::harness::strategies";
    Ok(match definition {
        AnyType::Primitive(primitive) => match primitive {
            PrimitiveType::UInt => format!("{s}::uint()"),
            PrimitiveType::Int => format!("{s}::int()"),
            PrimitiveType::F32 => format!("{s}::f32()"),
            PrimitiveType::F64 => format!("{s}::f64()"),
            PrimitiveType::Str => format!("{s}::string()"),
            PrimitiveType::Data(None) => format!("{s}::data()"),
            PrimitiveType::Data(Some(length)) if *length <= MAX_INLINE_DATA_LEN => {
                format!("{s}::fixed_array::<{length}>()")
            }
            PrimitiveType::Data(Some(length)) => format!("{s}::fixed_bytes::<{length}>()"),
            PrimitiveType::Void => "proptest::strategy::Just(()).boxed()".to_owned(),
            PrimitiveType::U64 => "proptest::prelude::any::<u64>().boxed()".to_owned(),
            PrimitiveType::U32 => "proptest::prelude::any::<u32>().boxed()".to_owned(),
            PrimitiveType::U16 => "proptest::prelude::any::<u16>().boxed()".to_owned(),
            PrimitiveType::U8 => "proptest::prelude::any::<u8>().boxed()".to_owned(),
            PrimitiveType::I64 => "proptest::prelude::any::<i64>().boxed()".to_owned(),
            PrimitiveType::I32 => "proptest::prelude::any::<i32>().boxed()".to_owned(),
            PrimitiveType::I16 => "proptest::prelude::any::<i16>().boxed()".to_owned(),
            PrimitiveType::I8 => "proptest::prelude::any::<i8>().boxed()".to_owned(),
            PrimitiveType::Bool => "proptest::prelude::any::<bool>().boxed()".to_owned(),
        },
        AnyType::Optional(inner) => {
            format!("proptest::option::of({}).boxed()", strategy(model, inner)?)
        }
        AnyType::List {
            inner,
            length: None,
        } => format!(
            "proptest::collection::vec({}, 0..4).boxed()",
            strategy(model, inner)?
        ),
        AnyType::List {
            inner,
            length: Some(length),
        } => format!(
            "{s}::fixed_list::<_, {length}>({})",
            strategy(model, inner)?
        ),
        // Duplicate keys collapse, so small key domains such as `bool` never stall generation.
        AnyType::Map { key, value } => format!(
            "proptest::collection::vec(({}, {}), 0..4).prop_map(|pairs| pairs.into_iter().collect::<std::collections::BTreeMap<_, _>>()).boxed()",
            strategy(model, key)?,
            strategy(model, value)?
        ),
        AnyType::TypeReference(name) => format!("{}()", model.item_path("strat", name)),
        AnyType::Struct(_) | AnyType::Union(_) | AnyType::Enum(_) => {
            return Err("anonymous aggregate after hoisting".into())
        }
    })
}

/// Combines strategies into nested tuples of at most six, which proptest implements
/// `Strategy` for. Returns the strategy expression and the matching pattern.
fn tuple_strategy(items: Vec<(String, String)>) -> (String, String) {
    let mut items = items;
    while items.len() > 6 {
        items = items
            .chunks(6)
            .map(|chunk| {
                let strategies: Vec<&str> = chunk.iter().map(|(s, _)| s.as_str()).collect();
                let patterns: Vec<&str> = chunk.iter().map(|(_, p)| p.as_str()).collect();
                (
                    format!("({},)", strategies.join(", ")),
                    format!("({},)", patterns.join(", ")),
                )
            })
            .collect();
    }
    let strategies: Vec<&str> = items.iter().map(|(s, _)| s.as_str()).collect();
    let patterns: Vec<&str> = items.iter().map(|(_, p)| p.as_str()).collect();
    (
        format!("({},)", strategies.join(", ")),
        format!("({},)", patterns.join(", ")),
    )
}

fn named_strategy(model: &FileModel, name: &str) -> Result<String, Error> {
    let definition = &model.defs[name];
    let path = model.candidate_path(name);
    Ok(match &definition.definition {
        AnyType::Struct(fields) => {
            let mut items = Vec::new();
            let mut assignments = Vec::new();
            for (index, field) in fields.iter().enumerate() {
                let binding = format!("f{index}");
                items.push((strategy(model, &field.type_r)?, binding.clone()));
                assignments.push(format!("{}: {binding}", rust_field_name(&field.name)?));
            }
            let (strategies, pattern) = tuple_strategy(items);
            format!(
                "{strategies}.prop_map(|{pattern}| {path} {{ {} }}).boxed()",
                assignments.join(", ")
            )
        }
        AnyType::Union(members) => {
            let mut arms = Vec::new();
            for (index, (member, _)) in members.iter().enumerate() {
                let variant = rust_union_variant_name(
                    member,
                    index,
                    definition.hoisted_members.contains(&index),
                );
                if model.is_void(member) {
                    arms.push(format!(
                        "proptest::strategy::Just({path}::{variant}).boxed()"
                    ));
                } else {
                    arms.push(format!(
                        "{}.prop_map({path}::{variant}).boxed()",
                        strategy(model, member)?
                    ));
                }
            }
            format!(
                "proptest::strategy::Union::new(vec![{}]).boxed()",
                arms.join(", ")
            )
        }
        AnyType::Enum(values) => {
            let variants: Vec<String> = values
                .iter()
                .map(|(value, _)| Ok(format!("{path}::{}", rust_enum_variant_name(value)?)))
                .collect::<Result<_, Error>>()?;
            format!(
                "proptest::sample::select(vec![{}]).boxed()",
                variants.join(", ")
            )
        }
        AnyType::Primitive(_)
        | AnyType::TypeReference(_)
        | AnyType::List { .. }
        | AnyType::Optional(_)
        | AnyType::Map { .. } => strategy(model, &definition.definition)?,
    })
}

/// Per-file output for the `desc`, `strat` and `cases` modules, plus entries.
pub struct RustItems {
    pub desc: String,
    pub strat: String,
    pub cases: String,
    pub entries: Vec<String>,
}

pub fn generate(model: &FileModel, reference: bool) -> Result<RustItems, Error> {
    let mut items = RustItems {
        desc: String::new(),
        strat: String::new(),
        cases: String::new(),
        entries: Vec::new(),
    };
    for name in &model.local {
        let definition = &model.defs[name];
        let path = model.candidate_path(name);
        writeln!(
            items.desc,
            "pub const {name}: &crate::desc::Desc = {};",
            desc_ref(model, &definition.definition)?
        )?;
        writeln!(
            items.strat,
            "pub fn {name}() -> proptest::strategy::BoxedStrategy<{path}> {{ {} }}",
            named_strategy(model, name)?
        )?;
        writeln!(
            items.cases,
            "pub struct {name};
impl crate::harness::Case for {name} {{
    type Wire = {path}Wire;
    type Owned = {path};
    const ID: crate::harness::CaseId = crate::harness::CaseId {{ set: {set:?}, module: {module:?}, name: {name:?} }};
    const DESC: &'static crate::desc::Desc = {desc};
    fn strategy() -> proptest::strategy::BoxedStrategy<{path}> {{ {strat}() }}
}}",
            set = model.set,
            module = model.module,
            desc = model.item_path("desc", name),
            strat = model.item_path("strat", name),
        )?;
        let constructor = if reference { "with_reference" } else { "new" };
        items.entries.push(format!(
            "crate::harness::Entry::{constructor}::<{}>({})",
            model.item_path("cases", name),
            model.is_user_type(name)
        ));
    }
    Ok(items)
}
