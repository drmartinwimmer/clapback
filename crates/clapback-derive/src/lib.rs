//! Procedural derive macros for `clapback`.

mod derive;

use derive::CommandDerive;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derives the `Command` trait for a CLI command struct.
#[proc_macro_derive(Command, attributes(command, arg))]
pub fn derive_command(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match CommandDerive::expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
