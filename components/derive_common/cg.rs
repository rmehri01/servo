/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use darling::{FromDeriveInput, FromField, FromVariant};
use proc_macro2::{Span, TokenStream};
use quote::{quote, TokenStreamExt};
use syn::{
    self, parse_quote, AngleBracketedGenericArguments, Binding, DeriveInput, Field,
    GenericArgument, GenericParam, Ident, Path, PathArguments, PathSegment, QSelf, Type, TypeArray,
    TypeGroup, TypeParam, TypeParen, TypePath, TypeSlice, TypeTuple, Variant, WherePredicate,
};
use synstructure::{self, BindStyle, BindingInfo, VariantAst, VariantInfo};

/// Given an input type which has some where clauses already, like:
///
/// struct InputType<T>
/// where
///     T: Zero,
/// {
///     ...
/// }
///
/// Add the necessary `where` clauses so that the output type of a trait
/// fulfils them.
///
/// For example:
///
/// ```ignore
///     <T as ToComputedValue>::ComputedValue: Zero,
/// ```
///
/// This needs to run before adding other bounds to the type parameters.
pub fn propagate_clauses_to_output_type(
    where_clause: &mut Option<syn::WhereClause>,
    generics: &syn::Generics,
    trait_path: &Path,
    trait_output: &Ident,
) {
    let where_clause = match *where_clause {
        Some(ref mut clause) => clause,
        None => return,
    };
    let mut extra_bounds = vec![];
    for pred in &where_clause.predicates {
        let ty = match *pred {
            syn::WherePredicate::Type(ref ty) => ty,
            ref predicate => panic!("Unhanded complex where predicate: {:?}", predicate),
        };

        let path = match ty.bounded_ty {
            syn::Type::Path(ref p) => &p.path,
            ref ty => panic!("Unhanded complex where type: {:?}", ty),
        };

        assert!(
            ty.lifetimes.is_none(),
            "Unhanded complex lifetime bound: {:?}",
            ty,
        );

        let ident = match path_to_ident(path) {
            Some(i) => i,
            None => panic!("Unhanded complex where type path: {:?}", path),
        };

        if generics.type_params().any(|param| param.ident == *ident) {
            extra_bounds.push(ty.clone());
        }
    }

    for bound in extra_bounds {
        let ty = bound.bounded_ty;
        let bounds = bound.bounds;
        where_clause
            .predicates
            .push(parse_quote!(<#ty as #trait_path>::#trait_output: #bounds))
    }
}

pub fn add_predicate(where_clause: &mut Option<syn::WhereClause>, pred: WherePredicate) {
    where_clause
        .get_or_insert(parse_quote!(where))
        .predicates
        .push(pred);
}

pub fn fmap_match<F>(input: &DeriveInput, bind_style: BindStyle, f: F) -> TokenStream
where
    F: FnMut(&BindingInfo) -> TokenStream,
{
    fmap2_match(input, bind_style, f, |_| None)
}

pub fn fmap2_match<F, G>(
    input: &DeriveInput,
    bind_style: BindStyle,
    mut f: F,
    mut g: G,
) -> TokenStream
where
    F: FnMut(&BindingInfo) -> TokenStream,
    G: FnMut(&BindingInfo) -> Option<TokenStream>,
{
    let mut s = synstructure::Structure::new(input);
    s.variants_mut().iter_mut().for_each(|v| {
        v.bind_with(|_| bind_style);
    });
    s.each_variant(|variant| {
        let (mapped, mapped_fields) = value(variant, "mapped");
        let fields_pairs = variant.bindings().iter().zip(mapped_fields.iter());
        let mut computations = quote!();
        computations.append_all(fields_pairs.map(|(field, mapped_field)| {
            let expr = f(field);
            quote! { let #mapped_field = #expr; }
        }));
        computations.append_all(
            mapped_fields
                .iter()
                .map(|mapped_field| match g(mapped_field) {
                    Some(expr) => quote! { let #mapped_field = #expr; },
                    None => quote!(),
                }),
        );
        computations.append_all(mapped);
        Some(computations)
    })
}

pub fn fmap_trait_output(input: &DeriveInput, trait_path: &Path, trait_output: &Ident) -> Path {
    let segment = PathSegment {
        ident: input.ident.clone(),
        arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
            args: input
                .generics
                .params
                .iter()
                .map(|arg| match arg {
                    &GenericParam::Lifetime(ref data) => {
                        GenericArgument::Lifetime(data.lifetime.clone())
                    },
                    &GenericParam::Type(ref data) => {
                        let ident = &data.ident;
                        GenericArgument::Type(parse_quote!(<#ident as #trait_path>::#trait_output))
                    },
                    &GenericParam::Const(ref inner) => {
                        let ident = &inner.ident;
                        GenericArgument::Const(parse_quote!(#ident))
                    },
                })
                .collect(),
            colon2_token: Default::default(),
            gt_token: Default::default(),
            lt_token: Default::default(),
        }),
    };
    segment.into()
}

pub fn map_type_params<F>(ty: &Type, params: &[&TypeParam], self_type: &Path, f: &mut F) -> Type
where
    F: FnMut(&Ident) -> Type,
{
    match *ty {
        Type::Slice(ref inner) => Type::from(TypeSlice {
            elem: Box::new(map_type_params(&inner.elem, params, self_type, f)),
            ..inner.clone()
        }),
        Type::Array(ref inner) => {
            //ref ty, ref expr) => {
            Type::from(TypeArray {
                elem: Box::new(map_type_params(&inner.elem, params, self_type, f)),
                ..inner.clone()
            })
        },
        ref ty @ Type::Never(_) => ty.clone(),
        Type::Tuple(ref inner) => Type::from(TypeTuple {
            elems: inner
                .elems
                .iter()
                .map(|ty| map_type_params(&ty, params, self_type, f))
                .collect(),
            ..inner.clone()
        }),
        Type::Path(TypePath {
            qself: None,
            ref path,
        }) => {
            if let Some(ident) = path_to_ident(path) {
                if params.iter().any(|ref param| &param.ident == ident) {
                    return f(ident);
                }
                if ident == "Self" {
                    return Type::from(TypePath {
                        qself: None,
                        path: self_type.clone(),
                    });
                }
            }
            Type::from(TypePath {
                qself: None,
                path: map_type_params_in_path(path, params, self_type, f),
            })
        },
        Type::Path(TypePath {
            ref qself,
            ref path,
        }) => Type::from(TypePath {
            qself: qself.as_ref().map(|qself| QSelf {
                ty: Box::new(map_type_params(&qself.ty, params, self_type, f)),
                position: qself.position,
                ..qself.clone()
            }),
            path: map_type_params_in_path(path, params, self_type, f),
        }),
        Type::Paren(ref inner) => Type::from(TypeParen {
            elem: Box::new(map_type_params(&inner.elem, params, self_type, f)),
            ..inner.clone()
        }),
        Type::Group(ref inner) => Type::from(TypeGroup {
            elem: Box::new(map_type_params(&inner.elem, params, self_type, f)),
            ..inner.clone()
        }),
        ref ty => panic!("type {:?} cannot be mapped yet", ty),
    }
}

fn map_type_params_in_path<F>(
    path: &Path,
    params: &[&TypeParam],
    self_type: &Path,
    f: &mut F,
) -> Path
where
    F: FnMut(&Ident) -> Type,
{
    Path {
        leading_colon: path.leading_colon,
        segments: path
            .segments
            .iter()
            .map(|segment| PathSegment {
                ident: segment.ident.clone(),
                arguments: match segment.arguments {
                    PathArguments::AngleBracketed(ref data) => {
                        PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            args: data
                                .args
                                .iter()
                                .map(|arg| match arg {
                                    ty @ &GenericArgument::Lifetime(_) => ty.clone(),
                                    &GenericArgument::Type(ref data) => GenericArgument::Type(
                                        map_type_params(data, params, self_type, f),
                                    ),
                                    &GenericArgument::Binding(ref data) => {
                                        GenericArgument::Binding(Binding {
                                            ty: map_type_params(&data.ty, params, self_type, f),
                                            ..data.clone()
                                        })
                                    },
                                    ref arg => panic!("arguments {:?} cannot be mapped yet", arg),
                                })
                                .collect(),
                            ..data.clone()
                        })
                    },
                    ref arg @ PathArguments::None => arg.clone(),
                    ref parameters => panic!("parameters {:?} cannot be mapped yet", parameters),
                },
            })
            .collect(),
    }
}

fn path_to_ident(path: &Path) -> Option<&Ident> {
    match *path {
        Path {
            leading_colon: None,
            ref segments,
        } if segments.len() == 1 => {
            if segments[0].arguments.is_empty() {
                Some(&segments[0].ident)
            } else {
                None
            }
        },
        _ => None,
    }
}

pub fn parse_field_attrs<A>(field: &Field) -> A
where
    A: FromField,
{
    match A::from_field(field) {
        Ok(attrs) => attrs,
        Err(e) => panic!("failed to parse field attributes: {}", e),
    }
}

pub fn parse_input_attrs<A>(input: &DeriveInput) -> A
where
    A: FromDeriveInput,
{
    match A::from_derive_input(input) {
        Ok(attrs) => attrs,
        Err(e) => panic!("failed to parse input attributes: {}", e),
    }
}

pub fn parse_variant_attrs_from_ast<A>(variant: &VariantAst) -> A
where
    A: FromVariant,
{
    let v = Variant {
        ident: variant.ident.clone(),
        attrs: variant.attrs.to_vec(),
        fields: variant.fields.clone(),
        discriminant: variant.discriminant.clone(),
    };
    parse_variant_attrs(&v)
}

pub fn parse_variant_attrs<A>(variant: &Variant) -> A
where
    A: FromVariant,
{
    match A::from_variant(variant) {
        Ok(attrs) => attrs,
        Err(e) => panic!("failed to parse variant attributes: {}", e),
    }
}

pub fn ref_pattern<'a>(
    variant: &'a VariantInfo,
    prefix: &str,
) -> (TokenStream, Vec<BindingInfo<'a>>) {
    let mut v = variant.clone();
    v.bind_with(|_| BindStyle::Ref);
    v.bindings_mut().iter_mut().for_each(|b| {
        b.binding = Ident::new(&format!("{}_{}", b.binding, prefix), Span::call_site())
    });
    (v.pat(), v.bindings().to_vec())
}

pub fn value<'a>(variant: &'a VariantInfo, prefix: &str) -> (TokenStream, Vec<BindingInfo<'a>>) {
    let mut v = variant.clone();
    v.bindings_mut().iter_mut().for_each(|b| {
        b.binding = Ident::new(&format!("{}_{}", b.binding, prefix), Span::call_site())
    });
    v.bind_with(|_| BindStyle::Move);
    (v.pat(), v.bindings().to_vec())
}

/// Transforms "FooBar" to "foo-bar".
///
/// If the first Camel segment is "Moz", "Webkit", or "Servo", the result string
/// is prepended with "-".
pub fn to_css_identifier(mut camel_case: &str) -> String {
    camel_case = camel_case.trim_end_matches('_');
    let mut first = true;
    let mut result = String::with_capacity(camel_case.len());
    while let Some(segment) = split_camel_segment(&mut camel_case) {
        if first {
            match segment {
                "Moz" | "Webkit" | "Servo" => first = false,
                _ => {},
            }
        }
        if !first {
            result.push('-');
        }
        first = false;
        result.push_str(&segment.to_lowercase());
    }
    result
}

/// Transforms foo-bar to FOO_BAR.
pub fn to_scream_case(css_case: &str) -> String {
    css_case.to_uppercase().replace('-', "_")
}

/// Given "FooBar", returns "Foo" and sets `camel_case` to "Bar".
fn split_camel_segment<'input>(camel_case: &mut &'input str) -> Option<&'input str> {
    let index = match camel_case.chars().next() {
        None => return None,
        Some(ch) => ch.len_utf8(),
    };
    let end_position = camel_case[index..]
        .find(char::is_uppercase)
        .map_or(camel_case.len(), |pos| index + pos);
    let result = &camel_case[..end_position];
    *camel_case = &camel_case[end_position..];
    Some(result)
}
