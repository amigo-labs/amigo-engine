//! Proc-macro crate for `#[derive(Reflect)]`.
//!
//! Generates an implementation of the `Reflect` trait for named-field structs.
//! Supports field attributes: `#[reflect(skip)]`, `#[reflect(read_only)]`,
//! `#[reflect(label = "...")]`, `#[reflect(range = 0.0..=100.0)]`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Derive macro that implements `Reflect` for a named-field struct.
///
/// # Field Attributes
///
/// - `#[reflect(skip)]` -- exclude from reflection
/// - `#[reflect(read_only)]` -- visible but not editable in inspector
/// - `#[reflect(label = "Display Name")]` -- custom display label
/// - `#[reflect(range = 0.0..=100.0)]` -- numeric range hint for sliders
///
/// # Example
///
/// ```ignore
/// #[derive(Reflect)]
/// struct Health {
///     #[reflect(range = 0.0..=1000.0)]
///     pub current: i32,
///     #[reflect(read_only)]
///     pub max: i32,
/// }
/// ```
#[proc_macro_derive(Reflect, attributes(reflect))]
pub fn derive_reflect(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_reflect(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

struct FieldAttrsParsed {
    skip: bool,
    read_only: bool,
    label: Option<String>,
    range: Option<(f64, f64)>,
}

impl Default for FieldAttrsParsed {
    fn default() -> Self {
        Self {
            skip: false,
            read_only: false,
            label: None,
            range: None,
        }
    }
}

fn parse_field_attrs(field: &syn::Field) -> syn::Result<FieldAttrsParsed> {
    let mut result = FieldAttrsParsed::default();

    for attr in &field.attrs {
        if !attr.path().is_ident("reflect") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                result.skip = true;
                return Ok(());
            }
            if meta.path.is_ident("read_only") {
                result.read_only = true;
                return Ok(());
            }
            if meta.path.is_ident("label") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                result.label = Some(lit.value());
                return Ok(());
            }
            if meta.path.is_ident("range") {
                let value = meta.value()?;
                // Parse as a range expression: `0.0..=100.0`
                let expr: syn::Expr = value.parse()?;
                match &expr {
                    syn::Expr::Range(range) => {
                        let lo = range.start.as_ref().ok_or_else(|| {
                            syn::Error::new_spanned(&expr, "range must have a start value")
                        })?;
                        let hi = range.end.as_ref().ok_or_else(|| {
                            syn::Error::new_spanned(&expr, "range must have an end value")
                        })?;
                        let lo_val = parse_float_expr(lo)?;
                        let hi_val = parse_float_expr(hi)?;
                        result.range = Some((lo_val, hi_val));
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &expr,
                            "expected range expression (e.g. 0.0..=100.0)",
                        ));
                    }
                }
                return Ok(());
            }
            Err(meta.error("unknown reflect attribute"))
        })?;
    }

    Ok(result)
}

fn parse_float_expr(expr: &syn::Expr) -> syn::Result<f64> {
    match expr {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Float(f) => f
                .base10_parse::<f64>()
                .map_err(|e| syn::Error::new_spanned(expr, e)),
            syn::Lit::Int(i) => i
                .base10_parse::<f64>()
                .map_err(|e| syn::Error::new_spanned(expr, e)),
            _ => Err(syn::Error::new_spanned(expr, "expected numeric literal")),
        },
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
            let inner = parse_float_expr(&unary.expr)?;
            Ok(-inner)
        }
        _ => Err(syn::Error::new_spanned(expr, "expected numeric literal")),
    }
}

fn impl_reflect(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let struct_name = &input.ident;
    let struct_name_str = struct_name.to_string();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    input,
                    "Reflect derive only supports named-field structs, not tuple structs",
                ));
            }
            Fields::Unit => {
                return Err(syn::Error::new_spanned(
                    input,
                    "Reflect derive only supports named-field structs, not unit structs",
                ));
            }
        },
        Data::Enum(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "Reflect derive only supports structs, not enums",
            ));
        }
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "Reflect derive only supports structs, not unions",
            ));
        }
    };

    // Parse field attributes and collect non-skipped fields
    let mut field_infos = Vec::new();
    let mut all_field_names = Vec::new(); // including skipped, for clone

    for field in fields {
        let ident = field.ident.as_ref().ok_or_else(|| {
            syn::Error::new_spanned(field, "unnamed fields not supported")
        })?;
        let attrs = parse_field_attrs(field)?;
        all_field_names.push(ident.clone());
        if !attrs.skip {
            field_infos.push((ident.clone(), field.ty.clone(), attrs));
        }
    }

    let field_count = field_infos.len();

    // Generate the static FIELDS array entries
    let field_info_entries: Vec<TokenStream2> = field_infos
        .iter()
        .map(|(ident, ty, attrs)| {
            let name_str = ident.to_string();
            let label_expr = match &attrs.label {
                Some(l) => quote! { ::core::option::Option::Some(#l) },
                None => quote! { ::core::option::Option::None },
            };
            let range_expr = match &attrs.range {
                Some((lo, hi)) => quote! { ::core::option::Option::Some((#lo, #hi)) },
                None => quote! { ::core::option::Option::None },
            };
            let read_only = attrs.read_only;
            let skip = false; // skipped fields are not included

            quote! {
                amigo_reflect::FieldInfo {
                    name: #name_str,
                    type_name: ::core::any::type_name::<#ty>(),
                    type_id: ::core::any::TypeId::of::<#ty>(),
                    offset: ::core::mem::offset_of!(#struct_name, #ident),
                    attrs: amigo_reflect::FieldAttrs {
                        label: #label_expr,
                        range: #range_expr,
                        read_only: #read_only,
                        skip: #skip,
                    },
                }
            }
        })
        .collect();

    // Generate field() match arms
    let field_match_arms: Vec<TokenStream2> = field_infos
        .iter()
        .enumerate()
        .map(|(i, (ident, _ty, _attrs))| {
            let name_str = ident.to_string();
            quote! {
                #name_str => {
                    ::core::option::Option::Some(amigo_reflect::FieldRef {
                        info: &TYPE_INFO.fields[#i],
                        value: &self.#ident,
                    })
                }
            }
        })
        .collect();

    // Generate field_mut() match arms
    let field_mut_match_arms: Vec<TokenStream2> = field_infos
        .iter()
        .enumerate()
        .map(|(i, (ident, _ty, _attrs))| {
            let name_str = ident.to_string();
            quote! {
                #name_str => {
                    ::core::option::Option::Some(amigo_reflect::FieldMut {
                        info: &TYPE_INFO.fields[#i],
                        value: &mut self.#ident,
                    })
                }
            }
        })
        .collect();

    // Generate fields() entries
    let fields_entries: Vec<TokenStream2> = field_infos
        .iter()
        .enumerate()
        .map(|(i, (ident, _ty, _attrs))| {
            quote! {
                amigo_reflect::FieldRef {
                    info: &TYPE_INFO.fields[#i],
                    value: &self.#ident,
                }
            }
        })
        .collect();

    // Generate fields_mut() entries -- we need to use unsafe pointer math
    // to get multiple mutable references to different fields simultaneously
    let fields_mut_entries: Vec<TokenStream2> = field_infos
        .iter()
        .enumerate()
        .map(|(i, (_ident, ty, _attrs))| {
            quote! {
                {
                    let ptr = base_ptr.add(TYPE_INFO.fields[#i].offset) as *mut #ty;
                    amigo_reflect::FieldMut {
                        info: &TYPE_INFO.fields[#i],
                        value: &mut *ptr,
                    }
                }
            }
        })
        .collect();

    // Generate clone fields
    let clone_fields: Vec<TokenStream2> = all_field_names
        .iter()
        .map(|ident| {
            quote! { #ident: ::core::clone::Clone::clone(&self.#ident) }
        })
        .collect();

    // Generate apply_patch arms
    let apply_patch_arms: Vec<TokenStream2> = field_infos
        .iter()
        .map(|(ident, ty, attrs)| {
            let name_str = ident.to_string();
            if attrs.read_only {
                // Read-only fields cannot be patched
                quote! {
                    #name_str => {}
                }
            } else {
                quote! {
                    #name_str => {
                        if let ::core::option::Option::Some(val) = value.downcast_ref::<#ty>() {
                            self.#ident = ::core::clone::Clone::clone(val);
                            count += 1;
                        }
                    }
                }
            }
        })
        .collect();

    let _type_path_str = struct_name_str.clone(); // simplified; full path would need module_path!

    let output = quote! {
        impl amigo_reflect::Reflect for #struct_name {
            fn type_info() -> &'static amigo_reflect::TypeInfo {
                static FIELDS: [amigo_reflect::FieldInfo; #field_count] = [
                    #(#field_info_entries),*
                ];
                static TYPE_INFO: amigo_reflect::TypeInfo = amigo_reflect::TypeInfo {
                    short_name: #struct_name_str,
                    type_path: #struct_name_str,
                    type_id: ::core::any::TypeId::of::<#struct_name>(),
                    fields: &FIELDS,
                };
                &TYPE_INFO
            }

            fn reflected_type_info(&self) -> &'static amigo_reflect::TypeInfo {
                // We need a local static that matches the one in type_info().
                // Re-use the same static by calling the sized method.
                <Self as amigo_reflect::Reflect>::type_info()
            }

            fn field(&self, name: &str) -> ::core::option::Option<amigo_reflect::FieldRef<'_>> {
                // We need access to TYPE_INFO for the field info references.
                static FIELDS: [amigo_reflect::FieldInfo; #field_count] = [
                    #(#field_info_entries),*
                ];
                static TYPE_INFO: amigo_reflect::TypeInfo = amigo_reflect::TypeInfo {
                    short_name: #struct_name_str,
                    type_path: #struct_name_str,
                    type_id: ::core::any::TypeId::of::<#struct_name>(),
                    fields: &FIELDS,
                };
                match name {
                    #(#field_match_arms)*
                    _ => ::core::option::Option::None,
                }
            }

            fn field_mut(&mut self, name: &str) -> ::core::option::Option<amigo_reflect::FieldMut<'_>> {
                static FIELDS: [amigo_reflect::FieldInfo; #field_count] = [
                    #(#field_info_entries),*
                ];
                static TYPE_INFO: amigo_reflect::TypeInfo = amigo_reflect::TypeInfo {
                    short_name: #struct_name_str,
                    type_path: #struct_name_str,
                    type_id: ::core::any::TypeId::of::<#struct_name>(),
                    fields: &FIELDS,
                };
                match name {
                    #(#field_mut_match_arms)*
                    _ => ::core::option::Option::None,
                }
            }

            fn fields(&self) -> ::std::vec::Vec<amigo_reflect::FieldRef<'_>> {
                static FIELDS: [amigo_reflect::FieldInfo; #field_count] = [
                    #(#field_info_entries),*
                ];
                static TYPE_INFO: amigo_reflect::TypeInfo = amigo_reflect::TypeInfo {
                    short_name: #struct_name_str,
                    type_path: #struct_name_str,
                    type_id: ::core::any::TypeId::of::<#struct_name>(),
                    fields: &FIELDS,
                };
                ::std::vec![
                    #(#fields_entries),*
                ]
            }

            fn fields_mut(&mut self) -> ::std::vec::Vec<amigo_reflect::FieldMut<'_>> {
                static FIELDS: [amigo_reflect::FieldInfo; #field_count] = [
                    #(#field_info_entries),*
                ];
                static TYPE_INFO: amigo_reflect::TypeInfo = amigo_reflect::TypeInfo {
                    short_name: #struct_name_str,
                    type_path: #struct_name_str,
                    type_id: ::core::any::TypeId::of::<#struct_name>(),
                    fields: &FIELDS,
                };
                let base_ptr = self as *mut Self as *mut u8;
                // SAFETY: Each field offset is computed by offset_of! and each pointer
                // is to a distinct field within the same struct. The struct is behind
                // a &mut reference, so we have exclusive access.
                unsafe {
                    ::std::vec![
                        #(#fields_mut_entries),*
                    ]
                }
            }

            fn apply_patch(&mut self, patch: &amigo_reflect::ReflectPatch) -> usize {
                let mut count = 0usize;
                for (name, value) in patch.iter() {
                    match name {
                        #(#apply_patch_arms)*
                        _ => {}
                    }
                }
                count
            }

            fn clone_reflect(&self) -> ::std::boxed::Box<dyn amigo_reflect::Reflect> {
                ::std::boxed::Box::new(#struct_name {
                    #(#clone_fields),*
                })
            }
        }
    };

    Ok(output)
}
