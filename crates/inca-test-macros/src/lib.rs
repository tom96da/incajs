// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test attribute macros for the inca crates.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, FnArg, ItemFn, Token, parse_macro_input};

/// Generates one test per `#[case]`, each carrying the other attributes of the
/// function (`#[gpui::test]`, `#[test]`, `#[allow(..)]`).
///
/// The last parameters take the values of a case, in order. The leading
/// parameters, such as `cx: &mut TestAppContext`, stay on the generated
/// function for the test attribute to supply. The tests live in a module
/// named after the function and are named like `rstest` names them:
/// `case_<N>` or `case_<N>_<id>` for `#[case::id(..)]`, with `N` counted from 1
/// and zero-padded to the width of the case count.
///
/// ```ignore
/// #[gpui_cases]
/// #[gpui::test]
/// #[case::left(MouseButton::Left, 0)]
/// #[case::right(MouseButton::Right, 2)]
/// fn click_reports_button(cx: &mut TestAppContext, button: MouseButton, expected: i16) {}
/// ```
#[proc_macro_attribute]
pub fn gpui_cases(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        let args = TokenStream2::from(args);
        return syn::Error::new_spanned(args, "gpui_cases takes no arguments")
            .to_compile_error()
            .into();
    }
    let function = parse_macro_input!(item as ItemFn);
    expand(&function)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

struct Case {
    id: Option<String>,
    values: Vec<Expr>,
}

/// `None` for an attribute that is not `case` or `case::<id>`.
fn parse_case(attr: &Attribute) -> syn::Result<Option<Case>> {
    let segments: Vec<_> = attr.path().segments.iter().collect();
    if segments.len() > 2 || segments.first().is_none_or(|s| s.ident != "case") {
        return Ok(None);
    }
    let values = attr.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)?;
    Ok(Some(Case {
        id: segments.get(1).map(|s| s.ident.to_string()),
        values: values.into_iter().collect(),
    }))
}

fn expand(function: &ItemFn) -> syn::Result<TokenStream2> {
    let mut cases = Vec::new();
    let mut kept = Vec::new();
    for attr in &function.attrs {
        match parse_case(attr)? {
            Some(case) => cases.push(case),
            None => kept.push(attr),
        }
    }
    let Some(first) = cases.first() else {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "gpui_cases needs at least one #[case]",
        ));
    };
    let value_count = first.values.len();
    let params: Vec<_> = function.sig.inputs.iter().collect();
    if value_count > params.len() || cases.iter().any(|c| c.values.len() != value_count) {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "every #[case] needs one value per case parameter, and no more than the parameters",
        ));
    }
    let (passed, case_params) = params.split_at(params.len() - value_count);
    let typed = case_params
        .iter()
        .map(|param| match param {
            FnArg::Typed(typed) => Ok(typed),
            FnArg::Receiver(receiver) => {
                Err(syn::Error::new_spanned(receiver, "a test has no receiver"))
            }
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let width = cases.len().to_string().len();
    let name = &function.sig.ident;
    let output = &function.sig.output;
    let body = &function.block;
    let tests = cases.iter().enumerate().map(|(index, case)| {
        let id = case
            .id
            .as_ref()
            .map_or_else(String::new, |id| format!("_{id}"));
        let test = format_ident!("case_{:0width$}{id}", index + 1);
        let bindings = typed.iter().zip(&case.values).map(|(param, value)| {
            let (pat, ty) = (&param.pat, &param.ty);
            quote!(let #pat: #ty = #value;)
        });
        quote! {
            #(#kept)*
            fn #test(#(#passed),*) #output {
                #(#bindings)*
                #body
            }
        }
    });
    Ok(quote! {
        mod #name {
            use super::*;
            #(#tests)*
        }
    })
}
