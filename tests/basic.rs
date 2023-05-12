use std::{error::Error, fmt::Debug};

use futures_executor::block_on;

#[test]
fn basic_wrapper() {
    use env_logger::{Builder, Env};
    Builder::from_env(Env::default().default_filter_or("trace")).init();
    ok().unwrap();
    silent_ok().unwrap();
    err().unwrap_err();
    dyn_error().unwrap_err();
    Test::err().unwrap_err();
    err_into(false, false).unwrap_err();
    block_on(unsafe { err_into_async_unsafe(false, false) }).unwrap_err();
    generic::err_into_generic(false, false).unwrap_err();
}

#[wrap_match::wrap_match(success_message = "success")]
fn ok() -> Result<(), ()> {
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
    Ok(())
}

#[wrap_match::wrap_match(error_message = "test {function} {error:?} {expr}")]
#[allow(clippy::let_unit_value)]
fn err_into(_arg1: bool, _arg2: bool) -> Result<(), CustomError> {
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
    pub fn err_into_generic<T>(_arg1: T, _arg2: bool) -> Result<(), CustomError> {
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
}
