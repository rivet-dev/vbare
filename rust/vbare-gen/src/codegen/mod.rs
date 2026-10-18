//! Rust code generation for the zero-copy codec in the `vbare` runtime.
//!
//! Every user type `Foo` produces:
//!
//! - `Foo`, the owned type used to build and encode values;
//! - `FooRef<'a>`, a borrowed view whose accessors read fields in place;
//! - `FooWire`, the marker implementing `vbare::Wire` with the verifier and codec;
//! - `FooView`, an alias for `vbare::View<FooWire>`, which owns a verified buffer.
//!
//! Type aliases produce aliases for all four. Anonymous aggregates are named first (see
//! [`hoist`]).
//!
//! Generated `verify_owned` methods are `#[inline]`. They are not generic, so without the hint a
//! caller in another crate, such as `vbare::decode`, cannot inline them, and single-pass decoding
//! then pays a call per nested type.

pub mod hoist;

use crate::{
    parser::{AnyType, PrimitiveType, StructField},
    Config,
};
use heck::{ToSnakeCase, ToUpperCamelCase};
use hoist::Definition;
use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};

/// `data[N]` up to this length is owned as `[u8; N]`; longer lengths share the input buffer.
const MAX_INLINE_DATA_LEN: usize = 32;

/// Suffixes of the items generated for each user type.
const DERIVED_SUFFIXES: [&str; 3] = ["Ref", "Wire", "View"];

/// The longest fixed-length list serde implements `Serialize` and `Deserialize` for.
const MAX_SERDE_ARRAY_LEN: usize = 32;

/// Inherent methods on generated `Ref` structs, which field accessors must not shadow.
const RESERVED_ACCESSORS: [&str; 2] = ["to_owned", "as_bytes"];

/// Generates the items for the types defined in one schema file. `registry` holds every type the
/// file can reference, including `local` and imported types.
pub fn generate(
    local: &BTreeMap<String, AnyType>,
    registry: &BTreeMap<String, AnyType>,
    config: Config,
) -> Result<TokenStream, String> {
    let mut definitions = Vec::new();
    for (name, definition) in local {
        definitions.extend(hoist::hoist(name, definition));
    }

    let mut types = registry.clone();
    for definition in &definitions {
        if definition.hoisted && types.contains_key(&definition.name) {
            return Err(format!(
                "generated name {} for an anonymous type collides with a user type",
                definition.name
            ));
        }
        types.insert(definition.name.clone(), definition.definition.clone());
    }
    check_derived_names(&types)?;
    if config.serde {
        for definition in &definitions {
            check_serde(&definition.name, &definition.definition)?;
        }
    }

    let generator = Generator {
        types: &types,
        serde: config.serde,
    };
    let mut items = Vec::new();
    for definition in &definitions {
        items.push(generator.definition(definition)?);
    }
    Ok(quote! { #(#items)* })
}

/// Rejects constructs whose serde derives would not write the bytes the codec writes. See
/// [`Config::serde`]. Hoisted definitions reference their anonymous parts by name, so each part is
/// checked as its own definition.
fn check_serde(name: &str, definition: &AnyType) -> Result<(), String> {
    match definition {
        AnyType::Primitive(PrimitiveType::UInt | PrimitiveType::Int) => Err(format!(
            "{name}: uint and int are not supported with serde derives, because serde_bare would \
             write them as fixed-width integers; use a fixed-width type"
        )),
        AnyType::Primitive(_) | AnyType::TypeReference(_) => Ok(()),
        AnyType::List { inner, length } => {
            if let Some(length) = length {
                if *length > MAX_SERDE_ARRAY_LEN {
                    return Err(format!(
                        "{name}: fixed-length lists longer than {MAX_SERDE_ARRAY_LEN} are not \
                         supported with serde derives"
                    ));
                }
            }
            check_serde(name, inner)
        }
        AnyType::Optional(inner) => check_serde(name, inner),
        AnyType::Map { key, value } => {
            check_serde(name, key)?;
            check_serde(name, value)
        }
        AnyType::Struct(fields) => {
            for field in fields {
                check_serde(&format!("{name}.{}", field.name), &field.type_r)?;
            }
            Ok(())
        }
        AnyType::Enum(values) => {
            for (index, (value_name, value)) in values.iter().enumerate() {
                if *value != index as u64 {
                    return Err(format!(
                        "{name}: enum value {value_name} = {value} differs from its index \
                         {index}, which serde derives would write instead"
                    ));
                }
            }
            Ok(())
        }
        AnyType::Union(members) => {
            for (index, (member, tag)) in members.iter().enumerate() {
                if *tag != index as u64 {
                    return Err(format!(
                        "{name}: union member {index} has tag {tag}, but serde derives would \
                         write its index"
                    ));
                }
                check_serde(&format!("{name} member {index}"), member)?;
            }
            Ok(())
        }
    }
}

/// Rejects a user type whose name equals another type's generated `Ref`, `Wire` or `View` name.
fn check_derived_names(types: &BTreeMap<String, AnyType>) -> Result<(), String> {
    for name in types.keys() {
        for suffix in DERIVED_SUFFIXES {
            let derived = format!("{name}{suffix}");
            if types.contains_key(&derived) {
                return Err(format!(
                    "type {derived} collides with the {suffix} item generated for type {name}"
                ));
            }
        }
    }
    Ok(())
}

/// The Rust name of the generated field or accessor for a schema field: snake_case, as a raw
/// identifier (`r#type`) for keywords.
pub fn rust_field_name(name: &str) -> Result<String, String> {
    field_ident(name).map(|(ident, _)| ident.to_string())
}

/// The Rust variant name generated for an enum value.
pub fn rust_enum_variant_name(name: &str) -> Result<String, String> {
    variant_ident(name).map(|ident| ident.to_string())
}

/// The Rust variant name generated for member `index` of a union. `hoisted` is whether the member
/// was an anonymous struct, union or enum (see [`hoist`](self::hoist)).
pub fn rust_union_variant_name(member: &AnyType, index: usize, hoisted: bool) -> String {
    if hoisted {
        format!("Member{index}")
    } else {
        member_variant_name(member, index)
    }
}

fn type_ident(name: &str) -> Result<Ident, String> {
    syn::parse_str::<Ident>(name).map_err(|_| format!("{name} is not a valid Rust type name"))
}

/// Converts a schema field name to a snake_case Rust identifier, using a raw identifier for
/// keywords.
fn field_ident(name: &str) -> Result<(Ident, String), String> {
    let snake = name.to_snake_case();
    if let Ok(ident) = syn::parse_str::<Ident>(&snake) {
        return Ok((ident, snake));
    }
    match syn::parse_str::<Ident>(&format!("r#{snake}")) {
        Ok(ident) => Ok((ident, snake)),
        Err(_) => Err(format!("field {name} cannot be a Rust identifier")),
    }
}

fn variant_ident(name: &str) -> Result<Ident, String> {
    let camel = name.to_upper_camel_case();
    syn::parse_str::<Ident>(&camel)
        .map_err(|_| format!("{name} cannot be a Rust enum variant name"))
}

fn index_literal(value: usize) -> Literal {
    Literal::usize_unsuffixed(value)
}

struct Generator<'a> {
    /// Every type visible to the file, with local anonymous aggregates hoisted.
    types: &'a BTreeMap<String, AnyType>,
    /// Whether owned types derive serde traits.
    serde: bool,
}

/// One union member as generated.
struct Member {
    variant: Ident,
    tag: u64,
    definition: AnyType,
    void: bool,
}

impl Generator<'_> {
    fn definition(&self, definition: &Definition) -> Result<TokenStream, String> {
        let name = definition.name.as_str();
        match &definition.definition {
            AnyType::Struct(fields) => self.structure(name, fields),
            AnyType::Union(members) => self.union(name, members, &definition.hoisted_members),
            AnyType::Enum(values) => self.enumeration(name, values),
            AnyType::Primitive(_)
            | AnyType::TypeReference(_)
            | AnyType::List { .. }
            | AnyType::Optional(_)
            | AnyType::Map { .. } => self.alias(name, &definition.definition),
        }
    }

    /// Follows type references to the underlying definition.
    fn resolve<'t>(&'t self, mut definition: &'t AnyType) -> &'t AnyType {
        // The validator rejects recursion, so a chain longer than the registry cannot occur.
        for _ in 0..=self.types.len() {
            match definition {
                AnyType::TypeReference(name) => match self.types.get(name) {
                    Some(next) => definition = next,
                    None => return definition,
                },
                AnyType::Primitive(_)
                | AnyType::List { .. }
                | AnyType::Struct(_)
                | AnyType::Enum(_)
                | AnyType::Map { .. }
                | AnyType::Union(_)
                | AnyType::Optional(_) => return definition,
            }
        }
        definition
    }

    fn is_void(&self, definition: &AnyType) -> bool {
        matches!(
            self.resolve(definition),
            AnyType::Primitive(PrimitiveType::Void)
        )
    }

    /// Whether a value of this type can hold a float, which rules out `Eq` and `Hash`.
    fn has_float(&self, definition: &AnyType) -> bool {
        match self.resolve(definition) {
            AnyType::Primitive(PrimitiveType::F32 | PrimitiveType::F64) => true,
            AnyType::Primitive(_) | AnyType::Enum(_) | AnyType::TypeReference(_) => false,
            AnyType::List { inner, .. } | AnyType::Optional(inner) => self.has_float(inner),
            AnyType::Map { key, value } => self.has_float(key) || self.has_float(value),
            AnyType::Struct(fields) => fields.iter().any(|field| self.has_float(&field.type_r)),
            AnyType::Union(members) => members.iter().any(|(member, _)| self.has_float(member)),
        }
    }

    /// Whether the borrowed form of this type holds a reference into the input. Types that do not
    /// borrow use their owned form as their borrowed form.
    fn borrows(&self, definition: &AnyType) -> bool {
        match self.resolve(definition) {
            AnyType::Primitive(primitive) => {
                matches!(primitive, PrimitiveType::Str | PrimitiveType::Data(_))
            }
            AnyType::List { .. } | AnyType::Map { .. } | AnyType::Struct(_) => true,
            AnyType::Optional(inner) => self.borrows(inner),
            AnyType::Union(members) => members.iter().any(|(member, _)| self.borrows(member)),
            AnyType::Enum(_) | AnyType::TypeReference(_) => false,
        }
    }

    /// The serde derives for owned types, preceded by a comma, if enabled.
    fn serde_derives(&self) -> TokenStream {
        if self.serde {
            quote! { , ::serde::Serialize, ::serde::Deserialize }
        } else {
            quote! {}
        }
    }

    fn eq_hash_derives(&self, definitions: &[&AnyType]) -> TokenStream {
        if definitions
            .iter()
            .any(|definition| self.has_float(definition))
        {
            quote! {}
        } else {
            quote! { , Eq, Hash }
        }
    }

    /// The `vbare::wire` marker naming this BARE type.
    fn wire(&self, definition: &AnyType) -> Result<TokenStream, String> {
        Ok(match definition {
            AnyType::Primitive(primitive) => match primitive {
                PrimitiveType::UInt => quote! { ::vbare::wire::Uint },
                PrimitiveType::U64 => quote! { ::vbare::wire::U64 },
                PrimitiveType::U32 => quote! { ::vbare::wire::U32 },
                PrimitiveType::U16 => quote! { ::vbare::wire::U16 },
                PrimitiveType::U8 => quote! { ::vbare::wire::U8 },
                PrimitiveType::Int => quote! { ::vbare::wire::Int },
                PrimitiveType::I64 => quote! { ::vbare::wire::I64 },
                PrimitiveType::I32 => quote! { ::vbare::wire::I32 },
                PrimitiveType::I16 => quote! { ::vbare::wire::I16 },
                PrimitiveType::I8 => quote! { ::vbare::wire::I8 },
                PrimitiveType::F64 => quote! { ::vbare::wire::F64 },
                PrimitiveType::F32 => quote! { ::vbare::wire::F32 },
                PrimitiveType::Str => quote! { ::vbare::wire::Str },
                PrimitiveType::Data(None) => quote! { ::vbare::wire::Data },
                PrimitiveType::Data(Some(length_value)) => {
                    let length = index_literal(*length_value);
                    if *length_value <= MAX_INLINE_DATA_LEN {
                        quote! { ::vbare::wire::FixedData<#length> }
                    } else {
                        quote! { ::vbare::wire::FixedDataBytes<#length> }
                    }
                }
                PrimitiveType::Void => quote! { ::vbare::wire::Void },
                PrimitiveType::Bool => quote! { ::vbare::wire::Bool },
            },
            AnyType::List { inner, length } => {
                let inner = self.wire(inner)?;
                match length {
                    Some(length) => {
                        let length = index_literal(*length);
                        quote! { ::vbare::wire::FixedList<#inner, #length> }
                    }
                    None => quote! { ::vbare::wire::List<#inner> },
                }
            }
            AnyType::Map { key, value } => {
                let key = self.wire(key)?;
                let value = self.wire(value)?;
                quote! { ::vbare::wire::Map<#key, #value> }
            }
            AnyType::Optional(inner) => {
                let inner = self.wire(inner)?;
                quote! { ::vbare::wire::Optional<#inner> }
            }
            AnyType::TypeReference(name) => {
                let ident = type_ident(&format!("{name}Wire"))?;
                quote! { #ident }
            }
            AnyType::Struct(_) | AnyType::Union(_) | AnyType::Enum(_) => {
                return Err("internal error: anonymous aggregate was not hoisted".into())
            }
        })
    }

    /// The owned Rust type. It must equal the marker's `Wire::Owned`.
    fn owned(&self, definition: &AnyType) -> Result<TokenStream, String> {
        Ok(match definition {
            AnyType::Primitive(primitive) => match primitive {
                PrimitiveType::UInt | PrimitiveType::U64 => quote! { u64 },
                PrimitiveType::U32 => quote! { u32 },
                PrimitiveType::U16 => quote! { u16 },
                PrimitiveType::U8 => quote! { u8 },
                PrimitiveType::Int | PrimitiveType::I64 => quote! { i64 },
                PrimitiveType::I32 => quote! { i32 },
                PrimitiveType::I16 => quote! { i16 },
                PrimitiveType::I8 => quote! { i8 },
                PrimitiveType::F64 => quote! { f64 },
                PrimitiveType::F32 => quote! { f32 },
                PrimitiveType::Str => quote! { ::vbare::Str },
                PrimitiveType::Data(None) => quote! { ::vbare::Bytes },
                PrimitiveType::Data(Some(length)) => {
                    let length_tokens = index_literal(*length);
                    if *length <= MAX_INLINE_DATA_LEN {
                        quote! { [u8; #length_tokens] }
                    } else {
                        quote! { ::vbare::FixedBytes<#length_tokens> }
                    }
                }
                PrimitiveType::Void => quote! { () },
                PrimitiveType::Bool => quote! { bool },
            },
            AnyType::List { inner, length } => {
                let inner = self.owned(inner)?;
                match length {
                    Some(length) => {
                        let length = index_literal(*length);
                        quote! { [#inner; #length] }
                    }
                    None => quote! { ::std::vec::Vec<#inner> },
                }
            }
            AnyType::Map { key, value } => {
                let key = self.owned(key)?;
                let value = self.owned(value)?;
                quote! { ::std::collections::BTreeMap<#key, #value> }
            }
            AnyType::Optional(inner) => {
                let inner = self.owned(inner)?;
                quote! { ::std::option::Option<#inner> }
            }
            AnyType::TypeReference(name) => {
                let ident = type_ident(name)?;
                quote! { #ident }
            }
            AnyType::Struct(_) | AnyType::Union(_) | AnyType::Enum(_) => {
                return Err("internal error: anonymous aggregate was not hoisted".into())
            }
        })
    }

    /// The borrowed Rust type with lifetime `'a`. It must equal the marker's `Wire::Ref<'a>`.
    fn borrowed(&self, definition: &AnyType) -> Result<TokenStream, String> {
        Ok(match definition {
            AnyType::Primitive(primitive) => match primitive {
                PrimitiveType::Str => quote! { &'a str },
                PrimitiveType::Data(None) => quote! { &'a [u8] },
                PrimitiveType::Data(Some(length)) => {
                    let length = index_literal(*length);
                    quote! { &'a [u8; #length] }
                }
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
                | PrimitiveType::F64
                | PrimitiveType::F32
                | PrimitiveType::Void
                | PrimitiveType::Bool => self.owned(definition)?,
            },
            AnyType::List { inner, .. } => {
                let inner = self.wire(inner)?;
                quote! { ::vbare::ListRef<'a, #inner> }
            }
            AnyType::Map { key, value } => {
                let key = self.wire(key)?;
                let value = self.wire(value)?;
                quote! { ::vbare::MapRef<'a, #key, #value> }
            }
            AnyType::Optional(inner) => {
                let inner = self.borrowed(inner)?;
                quote! { ::std::option::Option<#inner> }
            }
            AnyType::TypeReference(name) => {
                let ident = type_ident(&format!("{name}Ref"))?;
                quote! { #ident<'a> }
            }
            AnyType::Struct(_) | AnyType::Union(_) | AnyType::Enum(_) => {
                return Err("internal error: anonymous aggregate was not hoisted".into())
            }
        })
    }

    fn alias(&self, name: &str, definition: &AnyType) -> Result<TokenStream, String> {
        let ident = type_ident(name)?;
        let ref_ident = format_ident!("{name}Ref");
        let wire_ident = format_ident!("{name}Wire");
        let view_ident = format_ident!("{name}View");
        let owned = self.owned(definition)?;
        let borrowed = self.borrowed(definition)?;
        let wire = self.wire(definition)?;
        let doc = format!(" BARE type `{name}`.");
        Ok(quote! {
            #[doc = #doc]
            pub type #ident = #owned;
            pub type #ref_ident<'a> = #borrowed;
            pub type #wire_ident = #wire;
            pub type #view_ident = ::vbare::View<#wire_ident>;
        })
    }

    fn enumeration(&self, name: &str, values: &[(String, u64)]) -> Result<TokenStream, String> {
        let ident = type_ident(name)?;
        let ref_ident = format_ident!("{name}Ref");
        let wire_ident = format_ident!("{name}Wire");
        let view_ident = format_ident!("{name}View");

        let mut variants = Vec::new();
        let mut seen = BTreeSet::new();
        for (value_name, _) in values {
            let variant = variant_ident(value_name)?;
            if !seen.insert(variant.to_string()) {
                return Err(format!(
                    "enum {name}: values map to the same Rust variant {variant}"
                ));
            }
            variants.push(variant);
        }
        let numbers: Vec<Literal> = values
            .iter()
            .map(|(_, value)| Literal::u64_unsuffixed(*value))
            .collect();
        let first = variants
            .first()
            .ok_or_else(|| format!("enum {name} has no values"))?;
        let doc = format!(" BARE enum `{name}`.");
        let serde_derives = self.serde_derives();

        Ok(quote! {
            #[doc = #doc]
            #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash #serde_derives)]
            pub enum #ident {
                #(#variants),*
            }

            impl #ident {
                /// The value this variant encodes as.
                pub const fn value(self) -> u64 {
                    match self {
                        #(#ident::#variants => #numbers),*
                    }
                }

                pub const fn from_value(value: u64) -> ::std::option::Option<Self> {
                    match value {
                        #(#numbers => ::std::option::Option::Some(#ident::#variants),)*
                        _ => ::std::option::Option::None,
                    }
                }
            }

            pub type #ref_ident<'a> = #ident;

            pub struct #wire_ident;

            // SAFETY: `read` consumes the single varint that `verify` accepted.
            unsafe impl ::vbare::Wire for #wire_ident {
                type Ref<'a> = #ident;
                type Owned = #ident;
                type Root = ();

                const FIXED_SIZE: ::std::option::Option<usize> = ::std::option::Option::None;
                const MIN_SIZE: usize = 1;

                fn verify(v: &mut ::vbare::Verifier<'_>) -> ::std::result::Result<#ident, ::vbare::DecodeError> {
                    let offset = v.position();
                    let value = v.uint()?;
                    #ident::from_value(value).ok_or_else(|| {
                        ::vbare::DecodeError::new(::vbare::DecodeErrorKind::InvalidTag, offset)
                    })
                }

                fn read(r: &mut ::vbare::Reader<'_>) -> #ident {
                    #ident::from_value(r.uint()).unwrap_or(#ident::#first)
                }

                fn root(_: &#ident) {}

                fn from_root(mut r: ::vbare::Reader<'_>, _: ()) -> #ident {
                    <Self as ::vbare::Wire>::read(&mut r)
                }

                fn read_owned(r: &mut ::vbare::Reader<'_>) -> #ident {
                    <Self as ::vbare::Wire>::read(r)
                }

                #[inline]
                fn verify_owned(v: &mut ::vbare::Verifier<'_>) -> ::std::result::Result<#ident, ::vbare::DecodeError> {
                    <Self as ::vbare::Wire>::verify(v)
                }

                fn encoded_len(value: &#ident) -> usize {
                    ::vbare::varint::uint_len(value.value())
                }

                fn encode<S: ::vbare::Sink>(value: &#ident, out: &mut S) {
                    ::vbare::varint::put_uint(out, value.value());
                }

                fn ref_encoded_len(value: #ident) -> usize {
                    ::vbare::varint::uint_len(value.value())
                }

                fn encode_ref<S: ::vbare::Sink>(value: #ident, out: &mut S) {
                    ::vbare::varint::put_uint(out, value.value());
                }
            }

            impl ::vbare::Key for #wire_ident {
                fn cmp_ref(a: #ident, b: #ident) -> ::std::cmp::Ordering {
                    ::std::cmp::Ord::cmp(&a, &b)
                }
            }

            impl ::vbare::Encode for #ident {
                fn encoded_len(&self) -> usize {
                    <#wire_ident as ::vbare::Wire>::encoded_len(self)
                }

                fn encode<S: ::vbare::Sink>(&self, out: &mut S) {
                    <#wire_ident as ::vbare::Wire>::encode(self, out)
                }
            }

            impl ::vbare::Message for #ident {
                type Wire = #wire_ident;
            }

            pub type #view_ident = ::vbare::View<#wire_ident>;
        })
    }

    fn structure(&self, name: &str, fields: &[StructField]) -> Result<TokenStream, String> {
        let ident = type_ident(name)?;
        let ref_ident = format_ident!("{name}Ref");
        let wire_ident = format_ident!("{name}Wire");
        let view_ident = format_ident!("{name}View");

        let mut field_idents = Vec::new();
        let mut field_names = Vec::new();
        let mut seen = BTreeSet::new();
        for field in fields {
            let (field_ident, snake) = field_ident(&field.name)?;
            if RESERVED_ACCESSORS.contains(&snake.as_str()) {
                return Err(format!(
                    "struct {name}: field {} conflicts with the generated {snake} method",
                    field.name
                ));
            }
            if !seen.insert(snake.clone()) {
                return Err(format!(
                    "struct {name}: fields map to the same Rust name {snake}"
                ));
            }
            field_idents.push(field_ident);
            field_names.push(field.name.clone());
        }
        let owned: Vec<TokenStream> = fields
            .iter()
            .map(|field| self.owned(&field.type_r))
            .collect::<Result<_, _>>()?;
        let borrowed: Vec<TokenStream> = fields
            .iter()
            .map(|field| self.borrowed(&field.type_r))
            .collect::<Result<_, _>>()?;
        let wires: Vec<TokenStream> = fields
            .iter()
            .map(|field| self.wire(&field.type_r))
            .collect::<Result<_, _>>()?;
        let field_types: Vec<&AnyType> = fields.iter().map(|field| &field.type_r).collect();
        let extra_derives = self.eq_hash_derives(&field_types);
        let serde_derives = self.serde_derives();

        // The struct's borrowed form records where each field after the first starts.
        let last = fields
            .len()
            .checked_sub(1)
            .ok_or_else(|| format!("struct {name} has no fields"))?;
        let offsets_len = index_literal(last);
        let accessors = field_idents.iter().enumerate().map(|(index, field_ident)| {
            let wire = &wires[index];
            let borrowed = &borrowed[index];
            let start = if index == 0 {
                quote! { 0 }
            } else {
                let previous = index_literal(index - 1);
                quote! { self.offsets[#previous] as usize }
            };
            let end = if index == last {
                quote! { self.r.len() }
            } else {
                let current = index_literal(index);
                quote! { self.offsets[#current] as usize }
            };
            quote! {
                pub fn #field_ident(&self) -> #borrowed {
                    <#wire as ::vbare::Wire>::read_span(self.r.slice(#start, #end))
                }
            }
        });

        let record = |index: usize, position: TokenStream| {
            if index == last {
                quote! {}
            } else {
                let index = index_literal(index);
                quote! { offsets[#index] = #position; }
            }
        };
        let verify_steps = fields.iter().enumerate().map(|(index, _)| {
            let wire = &wires[index];
            let field_name = &field_names[index];
            let record = record(index, quote! { v.offset_from(start) });
            quote! {
                <#wire as ::vbare::Wire>::verify(v).map_err(|error| error.in_field(#field_name))?;
                #record
            }
        });
        let read_steps = fields.iter().enumerate().map(|(index, _)| {
            let wire = &wires[index];
            let record = record(index, quote! { (r.offset() - start) as u32 });
            quote! {
                <#wire as ::vbare::Wire>::skip(r);
                #record
            }
        });
        let offsets_binding = if last == 0 {
            quote! { let offsets = [0u32; 0]; }
        } else {
            quote! { let mut offsets = [0u32; #offsets_len]; }
        };
        let doc = format!(" BARE struct `{name}`.");
        let ref_doc = format!(" Borrowed view of [`{name}`] that reads fields in place.");

        Ok(quote! {
            #[doc = #doc]
            #[derive(Clone, Debug, PartialEq #extra_derives #serde_derives)]
            pub struct #ident {
                #(pub #field_idents: #owned,)*
            }

            #[doc = #ref_doc]
            #[derive(Clone, Copy)]
            pub struct #ref_ident<'a> {
                r: ::vbare::Reader<'a>,
                offsets: [u32; #offsets_len],
            }

            impl<'a> #ref_ident<'a> {
                #(#accessors)*

                /// Converts to the owned type, sharing `str` and `data` bytes with the input.
                pub fn to_owned(self) -> #ident {
                    let mut r = self.r;
                    <#wire_ident as ::vbare::Wire>::read_owned(&mut r)
                }

                /// The encoded struct.
                pub fn as_bytes(&self) -> &'a [u8] {
                    self.r.as_slice()
                }
            }

            impl ::std::fmt::Debug for #ref_ident<'_> {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    f.debug_struct(::std::stringify!(#ref_ident))
                        #(.field(#field_names, &self.#field_idents()))*
                        .finish()
                }
            }

            /// Encodings are canonical, so equal values have equal bytes, except for maps whose
            /// pairs appear in a different order.
            impl ::std::cmp::PartialEq for #ref_ident<'_> {
                fn eq(&self, other: &Self) -> bool {
                    self.as_bytes() == other.as_bytes()
                }
            }

            pub struct #wire_ident;

            // SAFETY: `read`, `skip` and `read_owned` visit the fields in the order `verify`
            // checks them, using each field's own `Wire` implementation. `verify_owned` checks
            // them in the same order with the same field paths.
            unsafe impl ::vbare::Wire for #wire_ident {
                type Ref<'a> = #ref_ident<'a>;
                type Owned = #ident;
                type Root = [u32; #offsets_len];

                const FIXED_SIZE: ::std::option::Option<usize> = ::vbare::wire::fixed_size_sum(&[
                    #(<#wires as ::vbare::Wire>::FIXED_SIZE),*
                ]);
                const MIN_SIZE: usize = ::vbare::wire::min_size_sum(&[
                    #(<#wires as ::vbare::Wire>::MIN_SIZE),*
                ]);

                fn verify<'a>(v: &mut ::vbare::Verifier<'a>) -> ::std::result::Result<#ref_ident<'a>, ::vbare::DecodeError> {
                    let start = v.position();
                    #offsets_binding
                    #(#verify_steps)*
                    ::std::result::Result::Ok(#ref_ident { r: v.reader_since(start), offsets })
                }

                fn read<'a>(r: &mut ::vbare::Reader<'a>) -> #ref_ident<'a> {
                    let start = r.offset();
                    #offsets_binding
                    #(#read_steps)*
                    #ref_ident { r: r.since(start), offsets }
                }

                fn skip(r: &mut ::vbare::Reader<'_>) {
                    #(<#wires as ::vbare::Wire>::skip(r);)*
                }

                fn root(value: &#ref_ident<'_>) -> [u32; #offsets_len] {
                    value.offsets
                }

                fn from_root(r: ::vbare::Reader<'_>, offsets: [u32; #offsets_len]) -> #ref_ident<'_> {
                    #ref_ident { r, offsets }
                }

                fn read_owned(r: &mut ::vbare::Reader<'_>) -> #ident {
                    #ident {
                        #(#field_idents: <#wires as ::vbare::Wire>::read_owned(r),)*
                    }
                }

                #[inline]
                fn verify_owned(v: &mut ::vbare::Verifier<'_>) -> ::std::result::Result<#ident, ::vbare::DecodeError> {
                    ::std::result::Result::Ok(#ident {
                        #(#field_idents: <#wires as ::vbare::Wire>::verify_owned(v)
                            .map_err(|error| error.in_field(#field_names))?,)*
                    })
                }

                fn encoded_len(value: &#ident) -> usize {
                    #(<#wires as ::vbare::Wire>::encoded_len(&value.#field_idents))+*
                }

                fn encode<S: ::vbare::Sink>(value: &#ident, out: &mut S) {
                    #(<#wires as ::vbare::Wire>::encode(&value.#field_idents, out);)*
                }

                fn ref_encoded_len(value: #ref_ident<'_>) -> usize {
                    value.r.len()
                }

                fn encode_ref<S: ::vbare::Sink>(value: #ref_ident<'_>, out: &mut S) {
                    out.put_slice(value.r.as_slice());
                }
            }

            impl ::vbare::Encode for #ident {
                fn encoded_len(&self) -> usize {
                    <#wire_ident as ::vbare::Wire>::encoded_len(self)
                }

                fn encode<S: ::vbare::Sink>(&self, out: &mut S) {
                    <#wire_ident as ::vbare::Wire>::encode(self, out)
                }
            }

            impl ::vbare::Encode for #ref_ident<'_> {
                fn encoded_len(&self) -> usize {
                    <#wire_ident as ::vbare::Wire>::ref_encoded_len(*self)
                }

                fn encode<S: ::vbare::Sink>(&self, out: &mut S) {
                    <#wire_ident as ::vbare::Wire>::encode_ref(*self, out)
                }
            }

            impl ::vbare::Message for #ident {
                type Wire = #wire_ident;
            }

            pub type #view_ident = ::vbare::View<#wire_ident>;
        })
    }

    fn union(
        &self,
        name: &str,
        members: &[(AnyType, u64)],
        hoisted_members: &BTreeSet<usize>,
    ) -> Result<TokenStream, String> {
        let ident = type_ident(name)?;
        let ref_ident = format_ident!("{name}Ref");
        let wire_ident = format_ident!("{name}Wire");
        let view_ident = format_ident!("{name}View");

        let mut generated = Vec::new();
        let mut seen = BTreeSet::new();
        for (index, (definition, tag)) in members.iter().enumerate() {
            let variant_name =
                rust_union_variant_name(definition, index, hoisted_members.contains(&index));
            let variant = type_ident(&variant_name)?;
            if !seen.insert(variant_name.clone()) {
                return Err(format!(
                    "union {name}: members map to the same Rust variant {variant_name}"
                ));
            }
            generated.push(Member {
                variant,
                tag: *tag,
                definition: definition.clone(),
                void: self.is_void(definition),
            });
        }

        let mut owned_variants = Vec::new();
        let mut ref_variants = Vec::new();
        let mut verify_arms = Vec::new();
        let mut read_arms = Vec::new();
        let mut skip_arms = Vec::new();
        let mut span_arms = Vec::new();
        let mut owned_arms = Vec::new();
        let mut verify_owned_arms = Vec::new();
        let mut len_arms = Vec::new();
        let mut encode_arms = Vec::new();
        let mut ref_len_arms = Vec::new();
        let mut ref_encode_arms = Vec::new();
        for member in &generated {
            let variant = &member.variant;
            let tag = Literal::u64_unsuffixed(member.tag);
            let tag_len = quote! { ::vbare::varint::uint_len(#tag) };
            let put_tag = quote! { ::vbare::varint::put_uint(out, #tag); };
            let variant_name = variant.to_string();
            if member.void {
                owned_variants.push(quote! { #variant });
                ref_variants.push(quote! { #variant });
                verify_arms
                    .push(quote! { #tag => ::std::result::Result::Ok(#ref_ident::#variant) });
                read_arms.push(quote! { #tag => #ref_ident::#variant });
                skip_arms.push(quote! { #tag => {} });
                span_arms.push(quote! { #tag => #ref_ident::#variant });
                owned_arms.push(quote! { #tag => #ident::#variant });
                verify_owned_arms
                    .push(quote! { #tag => ::std::result::Result::Ok(#ident::#variant) });
                len_arms.push(quote! { #ident::#variant => #tag_len });
                encode_arms.push(quote! { #ident::#variant => { #put_tag } });
                ref_len_arms.push(quote! { #ref_ident::#variant => #tag_len });
                ref_encode_arms.push(quote! { #ref_ident::#variant => { #put_tag } });
                continue;
            }
            let wire = self.wire(&member.definition)?;
            let owned = self.owned(&member.definition)?;
            let borrowed = self.borrowed(&member.definition)?;
            owned_variants.push(quote! { #variant(#owned) });
            ref_variants.push(quote! { #variant(#borrowed) });
            verify_arms.push(quote! {
                #tag => <#wire as ::vbare::Wire>::verify(v)
                    .map(#ref_ident::#variant)
                    .map_err(|error| error.in_field(#variant_name))
            });
            read_arms
                .push(quote! { #tag => #ref_ident::#variant(<#wire as ::vbare::Wire>::read(r)) });
            skip_arms.push(quote! { #tag => <#wire as ::vbare::Wire>::skip(r) });
            span_arms.push(quote! {
                #tag => #ref_ident::#variant(<#wire as ::vbare::Wire>::read_span(r.rest()))
            });
            owned_arms
                .push(quote! { #tag => #ident::#variant(<#wire as ::vbare::Wire>::read_owned(r)) });
            verify_owned_arms.push(quote! {
                #tag => <#wire as ::vbare::Wire>::verify_owned(v)
                    .map(#ident::#variant)
                    .map_err(|error| error.in_field(#variant_name))
            });
            len_arms.push(quote! {
                #ident::#variant(value) => #tag_len + <#wire as ::vbare::Wire>::encoded_len(value)
            });
            encode_arms.push(quote! {
                #ident::#variant(value) => {
                    #put_tag
                    <#wire as ::vbare::Wire>::encode(value, out);
                }
            });
            ref_len_arms.push(quote! {
                #ref_ident::#variant(value) => #tag_len + <#wire as ::vbare::Wire>::ref_encoded_len(value)
            });
            ref_encode_arms.push(quote! {
                #ref_ident::#variant(value) => {
                    #put_tag
                    <#wire as ::vbare::Wire>::encode_ref(value, out);
                }
            });
        }

        // Unknown tags never reach the readers on verified input; they fall back to the first
        // member so reads stay total.
        let first = generated
            .first()
            .ok_or_else(|| format!("union {name} has no members"))?;
        let first_variant = &first.variant;
        let (read_fallback, span_fallback, owned_fallback) = if first.void {
            (
                quote! { #ref_ident::#first_variant },
                quote! { #ref_ident::#first_variant },
                quote! { #ident::#first_variant },
            )
        } else {
            let wire = self.wire(&first.definition)?;
            (
                quote! { #ref_ident::#first_variant(<#wire as ::vbare::Wire>::read(r)) },
                quote! { #ref_ident::#first_variant(<#wire as ::vbare::Wire>::read_span(r.rest())) },
                quote! { #ident::#first_variant(<#wire as ::vbare::Wire>::read_owned(r)) },
            )
        };

        // `verify` and `verify_owned` reject an unknown tag at the tag's offset.
        let invalid_tag = quote! {
            _ => ::std::result::Result::Err(::vbare::DecodeError::new(
                ::vbare::DecodeErrorKind::InvalidTag,
                offset,
            ))
        };

        let member_types: Vec<&AnyType> = members.iter().map(|(member, _)| member).collect();
        let extra_derives = self.eq_hash_derives(&member_types);
        let serde_derives = self.serde_derives();
        let doc = format!(" BARE union `{name}`.");
        let ref_doc = format!(" Borrowed view of [`{name}`].");

        // A union whose members never borrow from the input is its own borrowed form.
        let borrowing = member_types.iter().any(|member| self.borrows(member));
        let (copy_derive, ref_definition, ref_encode) = if borrowing {
            (
                quote! {},
                quote! {
                    #[doc = #ref_doc]
                    #[derive(Clone, Copy, Debug, PartialEq)]
                    pub enum #ref_ident<'a> {
                        #(#ref_variants),*
                    }
                },
                quote! {
                    impl ::vbare::Encode for #ref_ident<'_> {
                        fn encoded_len(&self) -> usize {
                            <#wire_ident as ::vbare::Wire>::ref_encoded_len(*self)
                        }

                        fn encode<S: ::vbare::Sink>(&self, out: &mut S) {
                            <#wire_ident as ::vbare::Wire>::encode_ref(*self, out)
                        }
                    }
                },
            )
        } else {
            (
                quote! { Copy, },
                quote! {
                    #[doc = #ref_doc]
                    pub type #ref_ident<'a> = #ident;
                },
                quote! {},
            )
        };

        Ok(quote! {
            #[doc = #doc]
            #[derive(Clone, #copy_derive Debug, PartialEq #extra_derives #serde_derives)]
            pub enum #ident {
                #(#owned_variants),*
            }

            #ref_definition

            pub struct #wire_ident;

            // SAFETY: every reader matches the tag the same way `verify` does and reads the
            // member with its own `Wire` implementation.
            unsafe impl ::vbare::Wire for #wire_ident {
                type Ref<'a> = #ref_ident<'a>;
                type Owned = #ident;
                type Root = ();

                const FIXED_SIZE: ::std::option::Option<usize> = ::std::option::Option::None;
                const MIN_SIZE: usize = 1;

                fn verify<'a>(v: &mut ::vbare::Verifier<'a>) -> ::std::result::Result<#ref_ident<'a>, ::vbare::DecodeError> {
                    let offset = v.position();
                    match v.uint()? {
                        #(#verify_arms,)*
                        #invalid_tag,
                    }
                }

                fn read<'a>(r: &mut ::vbare::Reader<'a>) -> #ref_ident<'a> {
                    match r.uint() {
                        #(#read_arms,)*
                        _ => #read_fallback,
                    }
                }

                fn skip(r: &mut ::vbare::Reader<'_>) {
                    match r.uint() {
                        #(#skip_arms,)*
                        _ => {}
                    }
                }

                fn read_span(mut r: ::vbare::Reader<'_>) -> #ref_ident<'_> {
                    match r.uint() {
                        #(#span_arms,)*
                        _ => #span_fallback,
                    }
                }

                fn root(_: &#ref_ident<'_>) {}

                fn from_root(r: ::vbare::Reader<'_>, _: ()) -> #ref_ident<'_> {
                    <Self as ::vbare::Wire>::read_span(r)
                }

                fn read_owned(r: &mut ::vbare::Reader<'_>) -> #ident {
                    match r.uint() {
                        #(#owned_arms,)*
                        _ => #owned_fallback,
                    }
                }

                #[inline]
                fn verify_owned(v: &mut ::vbare::Verifier<'_>) -> ::std::result::Result<#ident, ::vbare::DecodeError> {
                    let offset = v.position();
                    match v.uint()? {
                        #(#verify_owned_arms,)*
                        #invalid_tag,
                    }
                }

                fn encoded_len(value: &#ident) -> usize {
                    match value {
                        #(#len_arms,)*
                    }
                }

                fn encode<S: ::vbare::Sink>(value: &#ident, out: &mut S) {
                    match value {
                        #(#encode_arms)*
                    }
                }

                fn ref_encoded_len(value: #ref_ident<'_>) -> usize {
                    match value {
                        #(#ref_len_arms,)*
                    }
                }

                fn encode_ref<S: ::vbare::Sink>(value: #ref_ident<'_>, out: &mut S) {
                    match value {
                        #(#ref_encode_arms)*
                    }
                }
            }

            impl ::vbare::Encode for #ident {
                fn encoded_len(&self) -> usize {
                    <#wire_ident as ::vbare::Wire>::encoded_len(self)
                }

                fn encode<S: ::vbare::Sink>(&self, out: &mut S) {
                    <#wire_ident as ::vbare::Wire>::encode(self, out)
                }
            }

            #ref_encode

            impl ::vbare::Message for #ident {
                type Wire = #wire_ident;
            }

            pub type #view_ident = ::vbare::View<#wire_ident>;
        })
    }
}

/// The variant name for a union member that was not hoisted: the type name for a named type,
/// the keyword for a primitive, and `Member{index}` for other anonymous types.
fn member_variant_name(definition: &AnyType, index: usize) -> String {
    match definition {
        AnyType::TypeReference(name) => name.clone(),
        AnyType::Primitive(primitive) => match primitive {
            PrimitiveType::UInt => "Uint".into(),
            PrimitiveType::U64 => "U64".into(),
            PrimitiveType::U32 => "U32".into(),
            PrimitiveType::U16 => "U16".into(),
            PrimitiveType::U8 => "U8".into(),
            PrimitiveType::Int => "Int".into(),
            PrimitiveType::I64 => "I64".into(),
            PrimitiveType::I32 => "I32".into(),
            PrimitiveType::I16 => "I16".into(),
            PrimitiveType::I8 => "I8".into(),
            PrimitiveType::F64 => "F64".into(),
            PrimitiveType::F32 => "F32".into(),
            PrimitiveType::Str => "Str".into(),
            PrimitiveType::Data(None) => "Data".into(),
            PrimitiveType::Data(Some(length)) => format!("Data{length}"),
            PrimitiveType::Void => "Void".into(),
            PrimitiveType::Bool => "Bool".into(),
        },
        AnyType::List { .. }
        | AnyType::Struct(_)
        | AnyType::Enum(_)
        | AnyType::Map { .. }
        | AnyType::Union(_)
        | AnyType::Optional(_) => format!("Member{index}"),
    }
}
