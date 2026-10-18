use quote::quote;
use syn::{DeriveInput, Error, LitStr};

/// Generates the `ToArgs` and `Command` implementations for the given [`DeriveInput`].
pub(crate) fn expand_derive_command(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut program_lit: Option<LitStr> = None;

    for attr in &input.attrs {
        if attr.path().is_ident("command") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("program") || meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    program_lit = Some(lit);
                    Ok(())
                } else {
                    Err(meta.error("expected `program = \"...\"` or `name = \"...\"`"))
                }
            })?;
        }
    }

    let Some(program) = program_lit else {
        return Err(Error::new_spanned(
            input,
            "derive(Command) requires a #[command(program = \"...\")] attribute",
        ));
    };

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::clapback::ToArgs for #name #ty_generics #where_clause {
            fn append_args(&self, _out: &mut ::std::vec::Vec<::std::ffi::OsString>) {}
        }

        #[automatically_derived]
        impl #impl_generics ::clapback::Command for #name #ty_generics #where_clause {
            fn program(&self) -> &::std::ffi::OsStr {
                ::std::ffi::OsStr::new(#program)
            }
        }
    })
}
