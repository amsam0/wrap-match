use std::{error::Error, fmt::Debug};

#[test]
fn basic_wrapper() {
    tracing_subscriber::fmt::init();
    ok(1).unwrap();
    silent_ok().unwrap();
    err().unwrap_err();
    dyn_error().unwrap_err();
    err_into(false, &false).unwrap_err();
    pollster::block_on(unsafe { err_into_async_unsafe(false, false) }).unwrap_err();
    generic::err_into_generic(false, &false).unwrap_err();
    Test::err().unwrap_err();
    Test.err_self().unwrap_err();
    err_mut_arg(false).unwrap_err();
    err_disregard_result();
    err_lifetime().unwrap_err();
    err_lifetime_generics("").unwrap_err();
}

#[wrap_match::wrap_match(success_message = "success {_a}")]
#[tracing::instrument]
fn ok(_a: i64) -> Result<(), ()> {
    tracing::info!("hello from tracing!");
    Ok(())
}

#[wrap_match::wrap_match(log_success = false)]
fn silent_ok() -> Result<(), ()> {
    Ok(())
}

#[derive(Debug)]
pub enum CustomError {
    Error,
}

impl std::fmt::Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CustomError")
    }
}
impl Error for CustomError {}

#[wrap_match::wrap_match]
fn err() -> Result<(), CustomError> {
    Err(CustomError::Error)?;
    Ok(())
}

#[wrap_match::wrap_match]
fn dyn_error() -> Result<(), Box<dyn Error>> {
    err()?;
    Err(CustomError::Error)?; // this will never be triggered, just to ensure it compiles fine
    Ok(())
}

#[wrap_match::wrap_match(error_message = "test {function} {error:?} {expr}")]
#[allow(clippy::let_unit_value)]
fn err_into(_arg1: bool, _arg2: &bool) -> Result<(), CustomError> {
    let _ = generic::err_into_generic(false, _arg2)?;
    Ok(())
}

#[wrap_match::wrap_match(error_message_without_info = "oh no an error occurred: {error:?}")]
async unsafe fn err_into_async_unsafe(_arg1: bool, _arg2: bool) -> Result<(), CustomError> {
    Err(CustomError::Error.into())
}

mod generic {
    use super::*;

    #[wrap_match::wrap_match(error_message_without_info = "oh no an error occurred")]
    pub fn err_into_generic<T>(_arg1: T, _arg2: &bool) -> Result<(), CustomError> {
        Err(CustomError::Error.into())
    }
}

struct Test;

impl Test {
    #[wrap_match::wrap_match]
    pub fn err() -> Result<(), CustomError> {
        Err(CustomError::Error)?;
        Ok(())
    }

    #[wrap_match::wrap_match]
    pub fn err_self(&self) -> Result<(), CustomError> {
        Err(CustomError::Error)?;
        Ok(())
    }
}

#[wrap_match::wrap_match]
#[allow(unused_mut)]
fn err_mut_arg(mut arg1: bool) -> Result<(), CustomError> {
    err_ref_mut_arg(&mut arg1)?;
    Err(CustomError::Error)?;
    Ok(())
}

#[wrap_match::wrap_match]
fn err_ref_mut_arg(_arg1: &mut bool) -> Result<(), CustomError> {
    Ok(())
}

#[wrap_match::wrap_match(disregard_result = true)]
fn err_disregard_result() -> Result<(), CustomError> {
    Err(CustomError::Error)?;
    Ok(())
}

#[wrap_match::wrap_match]
fn err_lifetime<'lt>() -> Result<&'lt str, CustomError> {
    Err(CustomError::Error)?;
    Ok("")
}

#[wrap_match::wrap_match]
fn err_lifetime_generics<'lt, ANY>(_any: ANY) -> Result<&'lt str, CustomError> {
    Err(CustomError::Error)?;
    Ok("")
}
