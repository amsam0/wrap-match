//! Please see <https://docs.rs/wrap-match>

use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
    ext::IdentExt,
    fold::{self, Fold},
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote, parse_quote_spanned,
    spanned::Spanned,
    Error, ExprTry, FnArg, ItemFn, LitBool, LitStr, Pat, PathArguments, ReturnType, Token, Type,
    Visibility,
};

struct Options {
    success_message: String,
    error_message: String,
    error_message_without_info: String,

    log_success: bool,
}

impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut options = Options {
            success_message: "Successfully ran {function}".into(),
            error_message: "An error occurred when running {function} (caused by `{expr}` on line {line}): {error:?}".into(),
            error_message_without_info: "An error occurred when running {function}: {error:?}".into(),

            log_success: true,
        };

        while input.peek(Ident::peek_any) {
            enum OptionName {
                SuccessMessage,
                ErrorMessage,
                ErrorMessageWithoutInfo,

                LogSuccess,
            }
            use OptionName::*;

            let name: Ident = input.parse()?;

            let option = match name.to_string().as_str() {
                "success_message" => SuccessMessage,
                "error_message" => ErrorMessage,
                "error_message_without_info" => ErrorMessageWithoutInfo,

                "log_success" => LogSuccess,

                _ => return Err(Error::new(name.span(), "wrap_match: unknown configuration option (expected `success_message`, `error_message`, `error_message_without_info`, or `log_success`)"))
            };

            let _: Token![=] = input.parse()?;

            match option {
                SuccessMessage | ErrorMessage | ErrorMessageWithoutInfo => {
                    let value: LitStr = input.parse()?;
                    let value = value.value();

                    match option {
                        SuccessMessage => options.success_message = value,
                        ErrorMessage => options.error_message = value,
                        ErrorMessageWithoutInfo => options.error_message_without_info = value,
                        _ => unreachable!(),
                    }
                }
                LogSuccess => {
                    let value: LitBool = input.parse()?;
                    let value = value.value();

                    match option {
                        LogSuccess => options.log_success = value,
                        _ => unreachable!(),
                    }
                }
            }

            // remove the next comma so we can parse an ident
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }

        Ok(options)
    }
}

struct AddErrorInfo;

impl Fold for AddErrorInfo {
    fn fold_expr_try(&mut self, mut i: ExprTry) -> ExprTry {
        // Adds error metadata (line number and expression that caused it) to try expressions
        let expr = *i.expr.clone();
        let expr_str = &expr
            .to_token_stream()
            .to_string()
            // there probably won't be any spaces in the expression, converting token stream to string adds unnecessary spaces
            // however, we need to catch parameters
            .replace(", ", ",/*WRAP_MATCH_SPACE*/")
            .replace(' ', "")
            .replace(",/*WRAP_MATCH_SPACE*/", ", ");
        i.expr = parse_quote_spanned! {i.span()=>
            #expr.map_err(|e| ::wrap_match::__private::WrapMatchError { line_and_expr: Some((::core::line!(), #expr_str.to_owned())), inner: e.into() })
        };
        fold::fold_expr_try(self, i)
    }

    fn fold_return_type(&mut self, i: ReturnType) -> ReturnType {
        match i {
            ReturnType::Default => fold::fold_return_type(self, i),
            ReturnType::Type(arrow, ty) => {
                let mut ty = *ty;
                // Change the Result error type to use our special error
                if let Type::Path(p) = &mut ty {
                    for segment in &mut p.path.segments {
                        if segment.ident.to_string().contains("Result") {
                            if let PathArguments::AngleBracketed(args) = &mut segment.arguments {
                                let err_type = args.args.pop().unwrap().value().clone();
                                args.args.push(parse_quote!(::wrap_match::__private::WrapMatchError<#err_type>));
                            }
                        }
                    }
                }
                fold::fold_return_type(self, ReturnType::Type(arrow, Box::new(ty)))
            }
        }
    }
}

#[proc_macro_attribute]
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
                .map(|s| !s.ident.to_string().contains("Result"))
                .unwrap_or(true),
            _ => true,
        },
    } {
        let span = if let ReturnType::Type(_, t) = &input.sig.output {
            t.span()
        } else {
            input.sig.span()
        };
        return quote_spanned! {span=>
            compile_error!("wrap_match currently only supports functions that return Results");
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
    let args: Vec<TokenStream2> = input
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

    let asyncness_await = if input.sig.asyncness.is_some() {
        quote!(.await)
    } else {
        quote!()
    };

    let attrs = input.attrs.clone();
    let vis = input.vis.clone();
    let sig = input.sig.clone();

    let orig_name = input.sig.ident.clone();
    let inner_name = format_ident!("_wrap_match_inner_{}", orig_name);

    let mut input = AddErrorInfo.fold_item_fn(input);
    input.sig.ident = inner_name.clone();
    input.vis = Visibility::Inherited; // make sure the inner function isn't leaked to the public
    input.attrs = vec![parse_quote!(#[doc(hidden)])]; // we will put the original attributes on the function we make (we also don't want the inner function to appear in docs or autocomplete)

    let success_message = options
        .success_message
        .replace("{function}", &format!("{}", orig_name));

    let error_message = options
        .error_message
        .replace("{function}", &format!("{}", orig_name));
    let mut error_message_format_parameters = vec![];
    if error_message.contains("{line}") {
        error_message_format_parameters.push(quote!(line = _line));
    }
    if error_message.contains("{expr}") {
        error_message_format_parameters.push(quote!(expr = _expr));
    }
    if error_message.contains("{error}") || error_message.contains("{error:?}") {
        error_message_format_parameters.push(quote!(error = e.inner));
    }

    let error_message_without_info = options
        .error_message_without_info
        .replace("{function}", &format!("{}", orig_name));
    let error_message_without_info_format_parameters = if error_message_without_info
        .contains("{error}")
        || error_message_without_info.contains("{error:?}")
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

    // for functions that take a self argument, we will need to put the inner function outside of our new function since we don't know what type self is
    let outer_input = if has_self_argument {
        input.attrs.push(
            parse_quote!(#[deprecated = "inner function for wrap-match. Please do not use!"]),
        );
        Some(&input)
    } else {
        None
    };
    let allow_deprecated = match has_self_argument {
        true => Some(quote!(#[allow(deprecated)])),
        false => None,
    };
    let inner_input = match has_self_argument {
        true => None,
        false => Some(&input),
    };

    quote! {
        #outer_input

        #(#attrs)* #vis #sig {
            #inner_input

            #allow_deprecated
            match #self_dot #inner_name(#(#args),*) #asyncness_await {
                Ok(r) => {
                    #success_log
                    Ok(r)
                }
                Err(e) => {
                    if let Some((_line, _expr)) = e.line_and_expr {
                        ::log::error!(#error_message, #(#error_message_format_parameters),*);
                    } else {
                        ::log::error!(#error_message_without_info, #error_message_without_info_format_parameters);
                    }
                    Err(e.inner)
                }
            }
        }
    }
    .into()
}
