use quote::ToTokens;
use syn::{
    fold::{self, Fold},
    parse_quote, parse_quote_spanned,
    spanned::Spanned,
    ExprTry, Generics, PathArguments, ReturnType, Type,
};

pub struct AddErrorInfo;

impl Fold for AddErrorInfo {
    /// Adds error metadata/info (line number and expression that caused it) to try expressions
    fn fold_expr_try(&mut self, mut i: ExprTry) -> ExprTry {
        let span = i.span();
        let expr = *i.expr;
        let expr_str = {
            // https://github.com/dtolnay/prettyplease/issues/57
            // https://github.com/dtolnay/prettyplease/issues/5
            let lines: Vec<_> = prettyplease::unparse(
                &syn::parse_file(&format!("fn main() {{\n{}\n}}", expr.to_token_stream()))
                    .expect("invalid expression? something made syn fail to parse the file"),
            )
            .trim()
            .lines()
            .map(|line| {
                let mut line = line.to_owned();
                // Remove the first indent, if there is one
                if line.starts_with("    ") {
                    line.drain(..4);
                }
                line
            })
            .collect();
            lines[1..(lines.len() - 1)].join("\n")
        };
        i.expr = parse_quote_spanned! {span=>
            #expr.map_err(|e| ::wrap_match::__private::WrapMatchError {
                    line_and_expr: Some((::core::line!(), #expr_str)),
                    #[allow(clippy::useless_conversion)]
                    inner: e.into()
                }
            )
        };
        fold::fold_expr_try(self, i)
    }

    /// Changes the Result error type to use our special error
    fn fold_return_type(&mut self, i: ReturnType) -> ReturnType {
        match i {
            ReturnType::Default => fold::fold_return_type(self, i),
            ReturnType::Type(arrow, ty) => {
                let mut ty = *ty;
                if let Type::Path(p) = &mut ty {
                    for segment in &mut p.path.segments {
                        if segment.ident.to_string().contains("Result") {
                            if let PathArguments::AngleBracketed(args) = &mut segment.arguments {
                                let err_type = args.args.pop().unwrap().value().clone();
                                args.args.push(parse_quote!(::wrap_match::__private::WrapMatchError<'_wrap_match_error, #err_type>));
                            }
                        }
                    }
                }
                fold::fold_return_type(self, ReturnType::Type(arrow, Box::new(ty)))
            }
        }
    }

    /// Add the lifetime `WrapMatchError`s will use
    fn fold_generics(&mut self, mut i: Generics) -> Generics {
        i.params.insert(0, parse_quote!('_wrap_match_error));
        fold::fold_generics(self, i)
    }
}
