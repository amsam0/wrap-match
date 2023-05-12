fn main() {}

#[wrap_match::wrap_match(option = 1)]
fn invalid_option_name() -> Result<(), CustomError> {
    Err(CustomError::Error)?;
    Ok(())
}
