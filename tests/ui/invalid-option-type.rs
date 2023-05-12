fn main() {}

#[wrap_match::wrap_match(success_message = true)]
fn invalid_option_type_1() -> Result<(), CustomError> {
    Err(CustomError::Error)?;
    Ok(())
}

#[wrap_match::wrap_match(log_success = "true")]
fn invalid_option_type_2() -> Result<(), CustomError> {
    Err(CustomError::Error)?;
    Ok(())
}
