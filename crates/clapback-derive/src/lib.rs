//! Procedural derive macros for `clapback`.

mod attr;
mod case;
mod derive;
mod subcommand;

use derive::{expand_derive_args, expand_derive_command};
use proc_macro::TokenStream;
use subcommand::{expand_derive_subcommand, expand_derive_value_enum};
use syn::{DeriveInput, parse_macro_input};

/// Derives the `Command`, `ToArgs`, and `Display` traits for a CLI command struct.
#[proc_macro_derive(Command, attributes(command, arg, env))]
pub fn derive_command(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_derive_command(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derives the `ToArgs` trait for a reusable CLI argument struct.
#[proc_macro_derive(Args, attributes(command, arg, env))]
pub fn derive_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_derive_args(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derives the `ToArgs` trait for a CLI subcommand enum.
#[proc_macro_derive(Subcommand, attributes(command, arg, env))]
pub fn derive_subcommand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_derive_subcommand(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derives `ValueEnum`, `ArgValue`, `EnvValue`, and `Display` for a unit enum.
#[proc_macro_derive(ValueEnum, attributes(value, command))]
pub fn derive_value_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_derive_value_enum(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
