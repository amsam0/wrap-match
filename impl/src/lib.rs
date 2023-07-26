//! Please see <https://docs.rs/wrap-match>

#![allow(clippy::enum_glob_use, clippy::match_bool)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
    fold::Fold, parse_macro_input, parse_quote, spanned::Spanned, FnArg, ItemFn, Pat, ReturnType,
    Type, Visibility,
};

mod add_error_info;
use self::add_error_info::AddErrorInfo;

mod options;
use self::options::Options;

#[proc_macro_attribute]
/// See crate level documentation for usage
pub fn wrap_match(args: TokenStream, input: TokenStream) -> TokenStream {
    let options = parse_macro_input!(args as Options);
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
    let args_without_types: Vec<TokenStream2> = input
        .sig
        .inputs
        .iter()
        .map(|a| match a {
            FnArg::Receiver(_) => {
                has_self_argument = true;
                quote!()
            }
            FnArg::Typed(a) => {
                if let Pat::Ident(mut a) = *a.pat.clone() {
                    a.attrs.clear();
                    a.mutability = None;
                    a.into_token_stream()
                } else {
                    a.pat.clone().into_token_stream()
                }
            }
        })
        .collect();

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
    let inner_name = format_ident!("_wrap_match_inner_{}", orig_name);

    let mut input = AddErrorInfo.fold_item_fn(input);
    input.sig.ident = inner_name.clone();
    input.vis = Visibility::Inherited; // make sure the inner function isn't leaked to the public
    input.attrs = vec![
        // we will put the original attributes on the function we make
        // we also don't want the inner function to appear in docs or autocomplete (if they do, they should be deprecated and give a warning if they are used)
        parse_quote!(#[doc(hidden)]),
        parse_quote!(#[deprecated = "inner function for wrap-match. Please do not use!"]),
        parse_quote!(#[allow(clippy::useless_conversion)]), // clippy will warn us for using .into() for every .map_err
        parse_quote!(#[inline(always)]), // let's make sure we don't produce more overhead than we need to, the output should produce similar assembly to the input (besides the end)
    ];

    let success_message = options
        .success_message
        .replace("{function}", &format!("{orig_name}"));

    let error_message = options
        .error_message
        .replace("{function}", &format!("{orig_name}"));
    let mut error_message_format_parameters = vec![];
    if error_message.contains("{line}") {
        error_message_format_parameters.push(quote!(line = _line));
    }
    if error_message.contains("{expr}") {
        error_message_format_parameters.push(quote!(expr = _expr));
    }
    if error_message.contains("{error}")
        || error_message.contains("{error:?}")
        || error_message.contains("{error:#?}")
    {
        error_message_format_parameters.push(quote!(error = e.inner));
    }

    let error_message_without_info = options
        .error_message_without_info
        .replace("{function}", &format!("{orig_name}"));
    let error_message_without_info_format_parameters = if error_message_without_info
        .contains("{error}")
        || error_message_without_info.contains("{error:?}")
        || error_message_without_info.contains("{error:#?}")
    {
        quote!(error = e.inner)
    } else {
        quote!()
    };

    let success_log = if options.log_success {
        quote!(::log::info!(#success_message);)
    } else {
        quote!()
    };

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
                    #success_log
                    #ok
                }
                Err(e) => {
                    if let Some((_line, _expr)) = e.line_and_expr {
                        ::log::error!(#error_message, #(#error_message_format_parameters),*);
                    } else {
                        ::log::error!(#error_message_without_info, #error_message_without_info_format_parameters);
                    }
                    #err
                }
            }
        }
    }
    .into()
}
