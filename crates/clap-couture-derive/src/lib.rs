//! Derive macro for [`clap-couture`](https://docs.rs/clap-couture).

mod clap_compat;
mod couture_impl;
mod input;
mod text;

use darling::FromDeriveInput as _;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{DeriveInput, parse_macro_input};

use crate::{couture_impl::CoutureImpl, input::CoutureInput};

/// Enable [`clap-couture`](https://docs.rs/clap-couture) on your clap subcommand enum.
#[proc_macro_derive(Couture, attributes(category, couture))]
pub fn derive_couture(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    CoutureInput::from_derive_input(&input)
        .and_then(CoutureImpl::try_from)
        .map_or_else(darling::Error::write_errors, ToTokens::into_token_stream)
        .into()
}
