use quote::ToTokens;
use syn::{
    fold::{self, Fold},
    parse_quote, parse_quote_spanned,
    spanned::Spanned,
    ExprTry, Generics, PathArguments, ReturnType, Type,
};

pub struct AddErrorInfo;

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
            #expr.map_err(|e| ::wrap_match::__private::WrapMatchError {
                    line_and_expr: Some((::core::line!(), #expr_str)),
                    inner: e.into()
                }
            )
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
                                args.args.push(parse_quote!(::wrap_match::__private::WrapMatchError<'_wrap_match_error, #err_type>));
                            }
                        }
                    }
                }
                fold::fold_return_type(self, ReturnType::Type(arrow, Box::new(ty)))
            }
        }
    }

    fn fold_generics(&mut self, mut i: Generics) -> Generics {
        i.params.insert(0, parse_quote!('_wrap_match_error));
        i
    }
}
