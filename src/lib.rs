/*!
# The Problem

If you ever want to log when an error occurs and what caused it, you may find yourself using a `match` statement for every possible error instead of using the `?` operator.

This results in extremely verbose code. It's a pain to write and maintain.

# Introducing wrap-match!

wrap-match is an attribute macro that wraps your function in a `match` statement. Additionally, **it attaches rich error information to all statements using the `?` operator (aka try expressions).**
This allows you to know exactly what line and expression caused the error.

> **Note**
>
> wrap-match uses the `log` crate to log success and error messages. It does not expose the log crate for expanded functions to use; you must depend on it yourself.
>
> Additionally, **no messages will appear unless you use a logging implementation.** I recommend `env_logger`, but you can find a full list
> [here](https://docs.rs/log/#available-logging-implementations).

## Example

First, add this to your `Cargo.toml`:

```toml
[dependencies]
wrap-match = "1"
log = "*"
# You'll also want a logging implementation, for example `env_logger`
# More info here: https://docs.rs/log/#available-logging-implementations
```

Now you can use the `wrap_match` attribute macro:

```
#[wrap_match::wrap_match]
fn my_function() -> Result<(), CustomError> {
    Err(CustomError::Error)?; // notice the ?; when the macro is expanded, it will be modified to include line number and expression
    Ok(())
}
```

This would expand to something like this (comments are not included normally):

```
fn my_function() -> Result<(), CustomError> {
    struct _wrap_match_error<E> {
        line_and_expr: Option<(u32, String)>,
        inner: E,
    }

    // This allows you to return `Err(CustomError::Error.into())`
    impl<E> From<E> for _wrap_match_error<E> {
        fn from(inner: E) -> Self {
            Self { line_and_expr: None, inner }
        }
    }

    // This is where the original function is
    fn _wrap_match_inner_my_function() -> Result<(), _wrap_match_error<CustomError>> {
        Err(CustomError::Error)
            .map_err(|e| _wrap_match_error {
                // Here, line number and expression are added to the error
                line_and_expr: Some((3, "Err(CustomError::Error)".to_owned())),
                inner: e.into(),
            })?;
        Ok(())
    }

    match _wrap_match_inner_my_function() {
        Ok(r) => {
            ::log::info!("Successfully ran my_function");
            Ok(r)
        }
        Err(e) => {
            if let Some((_line, _expr)) = e.line_and_expr {
                ::log::error!("An error occurred when running my_function (when running `{_expr}` on line {_line}): {:?}", e.inner);
            } else {
                ::log::error!("An error occurred when running my_function: {:?}", e.inner);
            }
            Err(e.inner)
        }
    }
}
```

If we run this code, it would log this:

```log
[ERROR] An error occurred when running my_function (when running `Err(CustomError::Error)` on line 3): Error
```

As you can see, wrap-match makes error logging extremely easy while still logging information like what caused the error.

## Customization

wrap-match allows the user to customize success and error messages, as well as choosing whether or not to log anything on success.

### `success_message`

The message that's logged on success.

Available format specifiers:

-   `{function}`: The original function name.

Default value: `Successfully ran {function}`

Example:

```
#[wrap_match::wrap_match(success_message = "{function} ran successfully!! 🎉🎉")]
fn my_function() -> Result<(), CustomError> {
    Ok(())
}
```

This would log:

```log
[INFO] my_function ran successfully!! 🎉🎉
```

### `error_message`

The message that's logged on error, when line and expression info **is** available. Currently, this is only for try expressions (expressions with a `?` after them).

Available format specifiers:

-   `{function}`: The original function name.
-   `{line}`: The line the error occurred on.
-   `{expr}`: The expression that caused the error.
-   `{error}` or `{error:?}`: The error.

Default value: `` An error occurred when running {function} (caused by `{expr}` on line {line}): {error:?} ``

Example:

```
#[wrap_match::wrap_match(error_message = "oh no, {function} failed! `{expr}` on line {line} caused the error: {error:?}")]
fn my_function() -> Result<(), CustomError> {
    Err(CustomError::Error)?;
    Ok(())
}
```

This would log:

```log
[ERROR] oh no, my_function failed! `Err(CustomError::Error)` on line 3 caused the error: Error
```

### `error_message_without_info`

The message that's logged on error, when line and expression info **is not** available. This is usually triggered if you return an error yourself and use `.into()`.

Available format specifiers:

-   `{function}`: The original function name.
-   `{error}` or `{error:?}`: The error.

Default value: `An error occurred when running {function}: {error:?}`

Example:

```
#[wrap_match::wrap_match(error_message_without_info = "oh no, {function} failed with this error: {error:?}")]
fn my_function() -> Result<(), CustomError> {
    Err(CustomError::Error.into())
}
```

This would log:

```log
[ERROR] oh no, my_function failed with this error: Error
```

### `log_success`

If `false`, nothing will be logged on success.

Default value: `true`

Example:

```
#[wrap_match::wrap_match(log_success = false)]
fn my_function() -> Result<(), CustomError> {
    Ok(())
}
```

This would log nothing.

## Limitations

wrap-match currently has the following limitations:

1.  wrap-match cannot be used on functions in implementations that take a `self` parameter. If you need support for this, please create a GitHub issue with your use case.

1.  wrap-match only supports `Result`s. If you need support for `Option`s, please create a GitHub issue with your use case.

1.  `error_message` and `error_message_without_info` only support formatting `error` using the `Debug` or `Display` formatters. This is because of how we determine what formatting specifiers are used.
    If you need support for other formatting specifiers (such as `:#?`), please create a GitHub issue with your use case.

1.  wrap-match cannot be used on `const` functions. This is because the `log` crate cannot be used in `const` contexts.

If wrap-match doesn't work for something not on this list, please create a GitHub issue!
*/

use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
    ext::IdentExt,
    fold::{self, Fold},
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote, parse_quote_spanned,
    spanned::Spanned,
    Error, ExprTry, FnArg, ItemFn, LitBool, LitStr, PathArguments, ReturnType, Token, Type,
    Visibility,
};

// TODO: info on no impl self support

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
            #expr.map_err(|e| _wrap_match_error { line_and_expr: Some((::core::line!(), #expr_str.to_owned())), inner: e.into() })
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
                                args.args.push(parse_quote!(_wrap_match_error<#err_type>));
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

    // remove types from args for use when calling the inner function
    let args: Vec<TokenStream2> = input
        .sig
        .inputs
        .iter()
        .map(|a| match a {
            FnArg::Receiver(a) => a.into_token_stream(),
            FnArg::Typed(a) => a.pat.clone().into_token_stream(),
        })
        .collect();

    let asyncness_await = match input.sig.asyncness {
        Some(_) => quote!(.await),
        None => quote!(),
    };

    let attrs = input.attrs.clone();
    let vis = input.vis.clone();
    let sig = input.sig.clone();

    let orig_name = input.sig.ident.clone();
    let inner_name = format_ident!("_wrap_match_inner_{}", orig_name);

    let mut input = AddErrorInfo.fold_item_fn(input);
    input.sig.ident = inner_name.clone();
    input.vis = Visibility::Inherited; // make sure the inner function isn't leaked to the public
    input.attrs.clear(); // we will put the original attributes on the function we make

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

    quote! {
        #(#attrs)* #vis #sig {
            #[allow(non_camel_case_types)]
            struct _wrap_match_error<E> {
                line_and_expr: Option<(u32, ::std::string::String)>,
                inner: E,
            }

            impl<E> From<E> for _wrap_match_error<E> {
                fn from(inner: E) -> Self {
                    Self {
                        line_and_expr: None,
                        inner
                    }
                }
            }

            #input

            match #inner_name(#(#args),*) #asyncness_await {
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
