//! Please see <https://docs.rs/wrap-match>

#![allow(
    clippy::enum_glob_use,
    clippy::match_bool,
    clippy::if_not_else,
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::implicit_clone
)]

use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
    fold::Fold, parse_macro_input, parse_quote, spanned::Spanned, FnArg, ItemFn, Pat, ReturnType,
    Type, Visibility,
};

mod add_error_info;
use self::add_error_info::AddErrorInfo;

mod options;
use self::options::Options;

mod log_statement;
use self::log_statement::build_log_statement;

#[proc_macro_attribute]
#[allow(clippy::too_many_lines)]
/// See crate level documentation for usage
pub fn wrap_match(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut options = parse_macro_input!(args as Options);
    let input = parse_macro_input!(input as ItemFn);

    if match input.sig.output {
        ReturnType::Default => true,
        ReturnType::Type(_, ref ty) => match &**ty {
            Type::Path(p) => p
                .path
                .segments
                .last()
                .map_or(true, |s| !s.ident.to_string().contains("Result")),
            _ => true,
        },
    } {
        let span = if let ReturnType::Type(_, t) = &input.sig.output {
            t.span()
        } else {
            input.sig.span()
        };
        return quote_spanned! {span=>
            compile_error!("wrap_match currently only supports functions that return `Result`s");
        }
        .into();
    }

    if let Some(constness) = &input.sig.constness {
        return quote_spanned! {constness.span()=>
            compile_error!("wrap_match cannot be used on const functions because the log crate cannot be used in const contexts");
        }
        .into();
    }

    let mut has_self_argument = false;
    // remove types from args for use when calling the inner function
    let mut args_without_types = vec![];
    let mut args_without_types_including_self = vec![];
    for arg in &input.sig.inputs {
        match arg {
            FnArg::Receiver(_) => {
                has_self_argument = true;
                args_without_types_including_self.push(quote!(self));
            }
            FnArg::Typed(arg) => {
                let tokens = if let Pat::Ident(mut a) = *arg.pat.clone() {
                    a.attrs.clear();
                    a.mutability = None;
                    a.into_token_stream()
                } else {
                    arg.pat.clone().into_token_stream()
                };
                args_without_types.push(tokens.clone());
                args_without_types_including_self.push(tokens);
            }
        }
    }

    let self_dot = if has_self_argument {
        quote!(self.)
    } else {
        quote!()
    };

    let asyncness_await = match input.sig.asyncness {
        Some(_) => quote!(.await),
        None => quote!(),
    };

    let attrs = input.attrs.clone();
    let vis = input.vis.clone();
    let mut sig = input.sig.clone();
    if options.disregard_result {
        sig.output = ReturnType::Default;
    }

    let orig_name = input.sig.ident.clone();
    options.replace_function_in_messages(orig_name.to_string());
    let inner_name = format_ident!("_wrap_match_inner_{}", orig_name);

    let mut input = AddErrorInfo.fold_item_fn(input);
    input.sig.ident = inner_name.clone();
    input.vis = Visibility::Inherited; // make sure the inner function isn't leaked to the public
    input.attrs = vec![
        // we will put the original attributes on the function we make
        // we also don't want the inner function to appear in docs or autocomplete (if they do, they should be deprecated and give a warning if they are used)
        parse_quote!(#[doc(hidden)]),
        parse_quote!(#[deprecated = "inner function for wrap-match. Please do not use!"]),
        parse_quote!(#[inline(always)]), // let's make sure we don't produce more overhead than we need to, the output should produce similar assembly to the input (besides the end)
    ];

    let log_success = if options.log_success {
        Some(build_log_statement(
            &options.success_message,
            &[],
            &args_without_types_including_self,
            quote!(info),
        ))
    } else {
        None
    };

    let log_error = build_log_statement(
        &options.error_message,
        &[
            ("line", quote!(_line)),
            ("expr", quote!(_expr)),
            ("error", quote!(e.inner)),
        ],
        &args_without_types_including_self,
        quote!(error),
    );

    let log_error_without_info = build_log_statement(
        &options.error_message_without_info,
        &[("error", quote!(e.inner))],
        &args_without_types_including_self,
        quote!(error),
    );

    let ok = if !options.disregard_result {
        quote!(Ok(r))
    } else {
        quote!()
    };
    let err = if !options.disregard_result {
        quote!(Err(e.inner))
    } else {
        quote!()
    };

    // for functions that take a self argument, we will need to put the inner function outside of our new function since we don't know what type self is
    let (outer_input, inner_input) = if has_self_argument {
        (Some(input), None)
    } else {
        (None, Some(input))
    };

    quote! {
        #outer_input

        #(#attrs)* #vis #sig {
            #inner_input

            #[allow(deprecated)]
            match #self_dot #inner_name(#(#args_without_types),*) #asyncness_await {
                Ok(r) => {
                    #log_success
                    #ok
                }
                Err(e) => {
                    if let Some((_line, _expr)) = e.line_and_expr {
                        #log_error
                    } else {
                        #log_error_without_info
                    }
                    #err
                }
            }
        }
    }
    .into()
}
