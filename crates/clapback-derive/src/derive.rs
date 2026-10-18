use crate::attr::{ContainerAttrs, FieldArgKind, ParsedField};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields};

/// Generates the `ToArgs`, `Command`, and `Display` implementations for `#[derive(Command)]`.
pub(crate) fn expand_derive_command(input: &DeriveInput) -> syn::Result<TokenStream> {
    let fields = extract_struct_fields(input, "Command")?;
    let container = ContainerAttrs::from_attrs(&input.attrs)?;
    let parsed_fields = parse_all_fields(fields)?;

    let program_body = build_program_body(input, &container, &parsed_fields)?;
    let current_dir_body = build_current_dir_body(input, &parsed_fields)?;
    let append_envs_body = build_append_envs_body(&parsed_fields);
    let ensure_user_path_body =
        build_ensure_user_path_body(container.ensure_user_path, &parsed_fields);
    let to_args_impl = build_to_args_impl(input, &container.prefix_args, &parsed_fields);

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        #to_args_impl

        #[automatically_derived]
        impl #impl_generics ::clapback::Command for #name #ty_generics #where_clause {
            fn program(&self) -> &::std::ffi::OsStr {
                #program_body
            }

            fn current_dir_path(&self) -> ::std::option::Option<&::std::path::Path> {
                #current_dir_body
            }

            fn append_envs(
                &self,
                out: &mut ::std::vec::Vec<(::std::ffi::OsString, ::std::ffi::OsString)>,
            ) {
                let _ = out;
                #append_envs_body
            }

            fn ensure_user_path(&self) -> bool {
                #ensure_user_path_body
            }
        }

        #[automatically_derived]
        impl #impl_generics ::std::fmt::Display for #name #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&::clapback::CommandExt::display(self), f)
            }
        }
    })
}

/// Generates the `ToArgs` implementation for `#[derive(Args)]`.
pub(crate) fn expand_derive_args(input: &DeriveInput) -> syn::Result<TokenStream> {
    let fields = extract_struct_fields(input, "Args")?;
    let container = ContainerAttrs::from_attrs(&input.attrs)?;
    let parsed_fields = parse_all_fields(fields)?;

    Ok(build_to_args_impl(
        input,
        &container.prefix_args,
        &parsed_fields,
    ))
}

fn extract_struct_fields<'a>(input: &'a DeriveInput, macro_name: &str) -> syn::Result<&'a Fields> {
    match &input.data {
        Data::Struct(data) => Ok(&data.fields),
        _ => Err(Error::new_spanned(
            input,
            format!("derive({macro_name}) can only be applied to structs"),
        )),
    }
}

fn parse_all_fields(fields: &Fields) -> syn::Result<Vec<ParsedField>> {
    fields
        .iter()
        .enumerate()
        .map(|(idx, field)| ParsedField::from_field(idx, field))
        .collect()
}

fn build_to_args_impl(
    input: &DeriveInput,
    prefix_args: &[String],
    parsed_fields: &[ParsedField],
) -> TokenStream {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let container_prefixes = prefix_args.iter().map(|arg| {
        quote! {
            out.push(::std::ffi::OsString::from(#arg));
        }
    });

    let field_stmts = parsed_fields.iter().map(build_field_to_args_stmt);

    quote! {
        #[automatically_derived]
        impl #impl_generics ::clapback::ToArgs for #name #ty_generics #where_clause {
            fn append_args(&self, out: &mut ::std::vec::Vec<::std::ffi::OsString>) {
                let _ = out;
                #(#container_prefixes)*
                #(#field_stmts)*
            }
        }
    }
}

pub(crate) fn build_field_to_args_stmt(field: &ParsedField) -> TokenStream {
    build_expr_to_args_stmt(&field.arg_kind, &field.prefix_args, &{
        let member = &field.member;
        quote!(&self.#member)
    })
}

pub(crate) fn build_expr_to_args_stmt(
    arg_kind: &FieldArgKind,
    prefix_args: &[String],
    value_expr: &TokenStream,
) -> TokenStream {
    if matches!(arg_kind, FieldArgKind::MetadataOnly) {
        return TokenStream::new();
    }

    let prefixes = prefix_args.iter().map(|arg| {
        quote! {
            out.push(::std::ffi::OsString::from(#arg));
        }
    });

    let body = match arg_kind {
        FieldArgKind::MetadataOnly => TokenStream::new(),
        FieldArgKind::Positional { last: false } => quote! {
            ::clapback::ArgValue::append_positional(#value_expr, out);
        },
        FieldArgKind::Positional { last: true } => quote! {
            ::clapback::ArgValue::append_last(#value_expr, out);
        },
        FieldArgKind::Named {
            flag,
            false_flag,
            require_equals,
            value_delimiter,
        } => {
            let false_flag_expr = match false_flag {
                Some(ff) => quote!(::std::option::Option::Some(#ff)),
                None => quote!(::std::option::Option::None),
            };
            let delimiter_expr = match value_delimiter {
                Some(ch) => quote!(::std::option::Option::Some(#ch)),
                None => quote!(::std::option::Option::None),
            };
            quote! {
                let spec = ::clapback::NamedArgSpec {
                    flag: #flag,
                    false_flag: #false_flag_expr,
                    require_equals: #require_equals,
                    value_delimiter: #delimiter_expr,
                };
                ::clapback::ArgValue::append_named(#value_expr, out, &spec);
            }
        }
        FieldArgKind::Subcommand | FieldArgKind::Flatten => quote! {
            ::clapback::ToArgs::append_args(#value_expr, out);
        },
        FieldArgKind::Wrap {
            last,
            is_option: false,
        } => quote! {
            ::clapback::append_wrapped_command(#value_expr, out, #last);
        },
        FieldArgKind::Wrap {
            last,
            is_option: true,
        } => quote! {
            ::clapback::append_optional_wrapped_command(#value_expr, out, #last);
        },
    };

    quote! {
        #(#prefixes)*
        #body
    }
}

fn build_program_body(
    input: &DeriveInput,
    container: &ContainerAttrs,
    parsed_fields: &[ParsedField],
) -> syn::Result<TokenStream> {
    let program_fields: Vec<&ParsedField> = parsed_fields.iter().filter(|f| f.is_program).collect();

    if program_fields.len() > 1 {
        return Err(Error::new_spanned(
            input,
            "at most one field may be annotated with #[command(program)]",
        ));
    }

    if let Some(field) = program_fields.first() {
        let member = &field.member;
        let default_expr = match &container.program {
            Some(lit) => quote!(::std::option::Option::Some(#lit)),
            None => quote!(::std::option::Option::None),
        };
        return Ok(quote! {
            ::clapback::AsProgramOsStr::as_program_os_str(&self.#member, #default_expr)
        });
    }

    let Some(program) = &container.program else {
        return Err(Error::new_spanned(
            input,
            "derive(Command) requires #[command(program = \"...\")] on the struct or #[command(program)] on a field",
        ));
    };

    Ok(quote! {
        ::std::ffi::OsStr::new(#program)
    })
}

fn build_current_dir_body(
    input: &DeriveInput,
    parsed_fields: &[ParsedField],
) -> syn::Result<TokenStream> {
    let cwd_fields: Vec<&ParsedField> = parsed_fields.iter().filter(|f| f.is_current_dir).collect();

    if cwd_fields.len() > 1 {
        return Err(Error::new_spanned(
            input,
            "at most one field may be annotated with #[command(current_dir)]",
        ));
    }

    let fallback = build_wrapped_current_dir_fallback(parsed_fields);

    if let Some(field) = cwd_fields.first() {
        let member = &field.member;
        Ok(quote! {
            ::clapback::AsOptionalPath::as_optional_path(&self.#member).or_else(|| #fallback)
        })
    } else {
        Ok(fallback)
    }
}

fn build_wrapped_current_dir_fallback(parsed_fields: &[ParsedField]) -> TokenStream {
    let mut expr = quote!(::std::option::Option::None);
    for field in parsed_fields.iter().rev() {
        let member = &field.member;
        match field.arg_kind {
            FieldArgKind::Wrap {
                is_option: false, ..
            } => {
                expr = quote! {
                    ::clapback::Command::current_dir_path(&self.#member).or_else(|| #expr)
                };
            }
            FieldArgKind::Wrap {
                is_option: true, ..
            } => {
                expr = quote! {
                    ::clapback::optional_wrapped_current_dir_path(&self.#member).or_else(|| #expr)
                };
            }
            _ => {}
        }
    }
    expr
}

fn build_append_envs_body(parsed_fields: &[ParsedField]) -> TokenStream {
    let wrap_stmts = parsed_fields.iter().filter_map(|field| {
        let member = &field.member;
        match field.arg_kind {
            FieldArgKind::Wrap {
                is_option: false, ..
            } => Some(quote! {
                ::clapback::Command::append_envs(&self.#member, out);
            }),
            FieldArgKind::Wrap {
                is_option: true, ..
            } => Some(quote! {
                ::clapback::append_optional_wrapped_envs(&self.#member, out);
            }),
            _ => None,
        }
    });

    let env_stmts = parsed_fields.iter().flat_map(|field| {
        let member = &field.member;
        field.env_vars.iter().map(move |var_name| {
            quote! {
                if let ::std::option::Option::Some(val) = ::clapback::EnvValue::to_env_os_string(&self.#member) {
                    out.push((::std::ffi::OsString::from(#var_name), val));
                }
            }
        })
    });

    quote! {
        #(#wrap_stmts)*
        #(#env_stmts)*
    }
}

fn build_ensure_user_path_body(
    container_ensure_user_path: bool,
    parsed_fields: &[ParsedField],
) -> TokenStream {
    let mut expr = quote!(#container_ensure_user_path);
    for field in parsed_fields {
        let member = &field.member;
        match field.arg_kind {
            FieldArgKind::Wrap {
                is_option: false, ..
            } => {
                expr = quote!(#expr || ::clapback::Command::ensure_user_path(&self.#member));
            }
            FieldArgKind::Wrap {
                is_option: true, ..
            } => {
                expr =
                    quote!(#expr || ::clapback::optional_wrapped_ensure_user_path(&self.#member));
            }
            _ => {}
        }
    }
    expr
}
