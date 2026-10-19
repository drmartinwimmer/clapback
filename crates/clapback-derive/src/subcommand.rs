use crate::attr::{ParsedField, parse_optional_bool_value, parse_string_array_value};
use crate::case::to_kebab_case;
use crate::derive::build_expr_to_args_stmt;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::meta::ParseNestedMeta;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Data, DeriveInput, Error, Field, Fields, Ident, LitStr, Token, Variant};

/// Variant-level delegation mode for `#[derive(Subcommand)]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum VariantMode {
    /// Standard subcommand variant that emits its subcommand name token first.
    #[default]
    Standard,
    /// Variant annotated with `#[command(subcommand)]` whose tuple payload is a nested `ToArgs`.
    NestedSubcommand,
    /// Variant annotated with `#[command(flatten)]` that omits the variant name token.
    Flatten,
}

/// Parsed `#[command(...)]` attributes on a `#[derive(Subcommand)]` enum variant.
#[derive(Debug, Default)]
struct VariantAttrs {
    name_override: Option<String>,
    prefix_args: Vec<String>,
    mode: VariantMode,
}

impl VariantAttrs {
    fn from_attrs(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut out = Self::default();
        for attr in attrs {
            if attr.path().is_ident("command") {
                attr.parse_nested_meta(|meta| out.apply_meta(&meta))?;
            }
        }
        Ok(out)
    }

    fn apply_meta(&mut self, meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
        match meta.path.get_ident().map(ToString::to_string).as_deref() {
            Some("name") => {
                let lit: LitStr = meta.value()?.parse()?;
                self.name_override = Some(lit.value());
                Ok(())
            }
            Some("subcommand") => {
                if parse_optional_bool_value(meta)? {
                    self.mode = VariantMode::NestedSubcommand;
                }
                Ok(())
            }
            Some("flatten" | "transparent") => {
                if parse_optional_bool_value(meta)? {
                    self.mode = VariantMode::Flatten;
                }
                Ok(())
            }
            Some("arg") => {
                let lit: LitStr = meta.value()?.parse()?;
                self.prefix_args.push(lit.value());
                Ok(())
            }
            Some("args") => {
                self.prefix_args.extend(parse_string_array_value(meta)?);
                Ok(())
            }
            _ => Err(meta.error(
                "unsupported variant #[command(...)] attribute; expected `name`, `subcommand`, `flatten`, `arg`, or `args`",
            )),
        }
    }

    fn emit_name_tokens(&self, variant_ident: &Ident) -> TokenStream {
        let has_name_override = self.name_override.is_some();
        if self.mode == VariantMode::Flatten || (!self.prefix_args.is_empty() && !has_name_override)
        {
            return TokenStream::new();
        }
        let subcommand_name = self
            .name_override
            .clone()
            .unwrap_or_else(|| to_kebab_case(&variant_ident.to_string()));
        quote! {
            out.push(::std::ffi::OsString::from(#subcommand_name));
        }
    }

    fn emit_prefix_tokens(&self) -> Vec<TokenStream> {
        self.prefix_args
            .iter()
            .map(|arg| {
                quote! {
                    out.push(::std::ffi::OsString::from(#arg));
                }
            })
            .collect()
    }
}

/// Generates the `ToArgs` implementation for `#[derive(Subcommand)]`.
pub(crate) fn expand_derive_subcommand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Enum(data_enum) = &input.data else {
        return Err(Error::new_spanned(
            input,
            "derive(Subcommand) can only be applied to enums",
        ));
    };

    let arms: Vec<TokenStream> = data_enum
        .variants
        .iter()
        .map(build_subcommand_variant_arm)
        .collect::<syn::Result<_>>()?;

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::clapback::ToArgs for #name #ty_generics #where_clause {
            fn append_args(&self, out: &mut ::std::vec::Vec<::std::ffi::OsString>) {
                let _ = out;
                match self {
                    #(#arms)*
                }
            }
        }
    })
}

fn build_subcommand_variant_arm(variant: &Variant) -> syn::Result<TokenStream> {
    let v_attrs = VariantAttrs::from_attrs(&variant.attrs)?;
    let v_ident = &variant.ident;
    let emit_name = v_attrs.emit_name_tokens(v_ident);
    let prefix_stmts = v_attrs.emit_prefix_tokens();

    match &variant.fields {
        Fields::Unit => {
            if v_attrs.mode != VariantMode::Standard {
                return Err(Error::new_spanned(
                    variant,
                    "unit variant cannot use #[command(subcommand)] or #[command(flatten)]",
                ));
            }
            Ok(quote! {
                Self::#v_ident => {
                    #emit_name
                    #(#prefix_stmts)*
                }
            })
        }
        Fields::Unnamed(unnamed) => build_unnamed_variant_arm(
            variant,
            &unnamed.unnamed,
            v_attrs.mode,
            &emit_name,
            &prefix_stmts,
        ),
        Fields::Named(named) => {
            build_named_variant_arm(variant, &named.named, &emit_name, &prefix_stmts)
        }
    }
}

fn build_unnamed_variant_arm(
    variant: &Variant,
    fields: &Punctuated<Field, Token![,]>,
    mode: VariantMode,
    emit_name: &TokenStream,
    prefix_stmts: &[TokenStream],
) -> syn::Result<TokenStream> {
    let v_ident = &variant.ident;
    let bindings: Vec<Ident> = fields
        .iter()
        .enumerate()
        .map(|(idx, f)| format_ident!("__field_{idx}", span = f.span()))
        .collect();

    if matches!(mode, VariantMode::NestedSubcommand | VariantMode::Flatten) {
        let [first_binding] = bindings.as_slice() else {
            return Err(Error::new_spanned(
                variant,
                "variant with #[command(subcommand)] or #[command(flatten)] must have exactly 1 field",
            ));
        };
        return Ok(quote! {
            Self::#v_ident(#(#bindings),*) => {
                #emit_name
                #(#prefix_stmts)*
                ::clapback::ToArgs::append_args(#first_binding, out);
            }
        });
    }

    let mut field_stmts = Vec::with_capacity(fields.len());
    for (idx, (field, binding)) in fields.iter().zip(bindings.iter()).enumerate() {
        let parsed = ParsedField::from_field(idx, field)?;
        let binding_expr = quote!(#binding);
        field_stmts.push(build_expr_to_args_stmt(
            &parsed.arg_kind,
            &parsed.prefix_args,
            &binding_expr,
        ));
    }

    Ok(quote! {
        Self::#v_ident(#(#bindings),*) => {
            #emit_name
            #(#prefix_stmts)*
            #(#field_stmts)*
        }
    })
}

fn build_named_variant_arm(
    variant: &Variant,
    fields: &Punctuated<Field, Token![,]>,
    emit_name: &TokenStream,
    prefix_stmts: &[TokenStream],
) -> syn::Result<TokenStream> {
    let v_ident = &variant.ident;
    let mut bindings = Vec::with_capacity(fields.len());
    let mut field_stmts = Vec::with_capacity(fields.len());

    for (idx, field) in fields.iter().enumerate() {
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| Error::new_spanned(field, "expected named field"))?;
        bindings.push(ident);
        let parsed = ParsedField::from_field(idx, field)?;
        let binding_expr = quote!(#ident);
        field_stmts.push(build_expr_to_args_stmt(
            &parsed.arg_kind,
            &parsed.prefix_args,
            &binding_expr,
        ));
    }

    Ok(quote! {
        Self::#v_ident { #(#bindings),* } => {
            #emit_name
            #(#prefix_stmts)*
            #(#field_stmts)*
        }
    })
}

/// Parsed `#[value(...)]` or `#[command(...)]` attributes on a `#[derive(ValueEnum)]` variant.
#[derive(Debug, Default)]
struct ValueEnumVariantAttrs {
    name_override: Option<String>,
}

impl ValueEnumVariantAttrs {
    fn from_attrs(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut out = Self::default();
        for attr in attrs {
            if attr.path().is_ident("value") || attr.path().is_ident("command") {
                attr.parse_nested_meta(|meta| out.apply_meta(&meta))?;
            }
        }
        Ok(out)
    }

    fn apply_meta(&mut self, meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
        match meta.path.get_ident().map(ToString::to_string).as_deref() {
            Some("name" | "alias") => {
                let lit: LitStr = meta.value()?.parse()?;
                self.name_override = Some(lit.value());
                Ok(())
            }
            _ => Err(meta.error("expected `name = \"...\"` in #[value(...)]")),
        }
    }

    fn resolve_name(self, variant_ident: &Ident) -> String {
        self.name_override
            .unwrap_or_else(|| to_kebab_case(&variant_ident.to_string()))
    }
}

/// Generates `ValueEnum`, `ArgValue`, `EnvValue`, and `Display` implementations for `#[derive(ValueEnum)]`.
pub(crate) fn expand_derive_value_enum(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Enum(data_enum) = &input.data else {
        return Err(Error::new_spanned(
            input,
            "derive(ValueEnum) can only be applied to enums",
        ));
    };

    let arms: Vec<TokenStream> = data_enum
        .variants
        .iter()
        .map(build_value_enum_variant_arm)
        .collect::<syn::Result<_>>()?;

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::clapback::ValueEnum for #name #ty_generics #where_clause {
            fn as_arg_str(&self) -> &str {
                match self {
                    #(#arms)*
                }
            }
        }

        #[automatically_derived]
        impl #impl_generics ::clapback::ArgValue for #name #ty_generics #where_clause {
            fn append_positional(&self, out: &mut ::std::vec::Vec<::std::ffi::OsString>) {
                let s = ::clapback::ValueEnum::as_arg_str(self);
                ::clapback::ArgValue::append_positional(s, out);
            }

            fn append_named(
                &self,
                out: &mut ::std::vec::Vec<::std::ffi::OsString>,
                spec: &::clapback::NamedArgSpec<'_>,
            ) {
                let s = ::clapback::ValueEnum::as_arg_str(self);
                ::clapback::ArgValue::append_named(s, out, spec);
            }
        }

        #[automatically_derived]
        impl #impl_generics ::clapback::EnvValue for #name #ty_generics #where_clause {
            fn to_env_os_string(&self) -> ::std::option::Option<::std::ffi::OsString> {
                ::std::option::Option::Some(::std::ffi::OsString::from(
                    ::clapback::ValueEnum::as_arg_str(self),
                ))
            }
        }

        #[automatically_derived]
        impl #impl_generics ::std::fmt::Display for #name #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(::clapback::ValueEnum::as_arg_str(self))
            }
        }
    })
}

fn build_value_enum_variant_arm(variant: &Variant) -> syn::Result<TokenStream> {
    if !matches!(variant.fields, Fields::Unit) {
        return Err(Error::new_spanned(
            variant,
            "derive(ValueEnum) only supports unit variants",
        ));
    }
    let v_ident = &variant.ident;
    let val_name = ValueEnumVariantAttrs::from_attrs(&variant.attrs)?.resolve_name(v_ident);
    Ok(quote! {
        Self::#v_ident => #val_name,
    })
}
