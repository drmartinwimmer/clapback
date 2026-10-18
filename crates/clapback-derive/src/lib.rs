//! Procedural derive macros for `clapback`.

mod derive;

use derive::expand_derive_command;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derives the `Command` and `ToArgs` traits for a CLI command struct.
#[proc_macro_derive(Command, attributes(command, arg))]
pub fn derive_command(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_derive_command(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
