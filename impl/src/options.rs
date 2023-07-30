use proc_macro2::Ident;
use syn::{
    ext::IdentExt,
    parse::{Parse, ParseStream},
    Error, LitBool, LitStr, Token,
};

pub struct Options {
    pub success_message: String,
    pub error_message: String,
    pub error_message_without_info: String,

    pub log_success: bool,
    pub disregard_result: bool,
}

impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut options = Options {
            success_message: "Successfully ran {function}".to_owned(),
            error_message: "An error occurred when running {function} (caused by `{expr}` on line {line}): {error:?}".to_owned(),
            error_message_without_info: "An error occurred when running {function}: {error:?}".to_owned(),

            log_success: true,
            disregard_result: false,
        };

        while input.peek(Ident::peek_any) {
            enum OptionName {
                SuccessMessage,
                ErrorMessage,
                ErrorMessageWithoutInfo,

                LogSuccess,
                DisregardResult,
            }
            use OptionName::*;

            let name: Ident = input.parse()?;

            let option = match name.to_string().as_str() {
                "success_message" => SuccessMessage,
                "error_message" => ErrorMessage,
                "error_message_without_info" => ErrorMessageWithoutInfo,

                "log_success" => LogSuccess,
                "disregard_result" => DisregardResult,

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
                LogSuccess | DisregardResult => {
                    let value: LitBool = input.parse()?;
                    let value = value.value();

                    match option {
                        LogSuccess => options.log_success = value,
                        DisregardResult => options.disregard_result = value,
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
