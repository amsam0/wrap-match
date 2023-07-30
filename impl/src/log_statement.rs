use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};

pub fn build_log_statement(
    (input, input_span): &(String, Span),
    builtin_parameters: &[(&'static str, TokenStream2)],
    other_parameters: &Vec<TokenStream2>,
    level: TokenStream2,
) -> TokenStream2 {
    #[cfg(not(feature = "tracing"))]
    let logging_crate = quote!(log);
    #[cfg(feature = "tracing")]
    let logging_crate = quote!(tracing);

    let mut parameters = vec![];

    fn contains_parameter(input: &String, parameter_name: impl AsRef<str>) -> bool {
        let parameter_name = parameter_name.as_ref();
        // These are all of the basic formats, and I don't really want to implement this: https://doc.rust-lang.org/stable/std/fmt/index.html#syntax
        input.contains(&format!("{{{parameter_name}}}"))
            || input.contains(&format!("{{{parameter_name}:?}}"))
            || input.contains(&format!("{{{parameter_name}:#?}}"))
            || input.contains(&format!("{{{parameter_name}:x?}}"))
            || input.contains(&format!("{{{parameter_name}:X?}}"))
            || input.contains(&format!("{{{parameter_name}:x}}"))
            || input.contains(&format!("{{{parameter_name}:X}}"))
            || input.contains(&format!("{{{parameter_name}:o}}"))
            || input.contains(&format!("{{{parameter_name}:b}}"))
            || input.contains(&format!("{{{parameter_name}:p}}"))
            || input.contains(&format!("{{{parameter_name}:e}}"))
            || input.contains(&format!("{{{parameter_name}:E}}"))
    }

    for (parameter_name, parameter_var_name) in builtin_parameters {
        if contains_parameter(input, parameter_name) {
            let parameter_name = Ident::new(&parameter_name, Span::call_site());
            parameters.push(quote!(#parameter_name = #parameter_var_name));
        }
    }

    for parameter_name in other_parameters {
        let parameter_name = parameter_name.to_string();
        if contains_parameter(input, &parameter_name) {
            let parameter_name = Ident::new(&parameter_name, Span::call_site());
            parameters.push(quote!(#parameter_name = #parameter_name));
        }
    }

    quote_spanned! {input_span.to_owned()=>
        ::#logging_crate::#level!(#input, #(#parameters),*);
    }
}
