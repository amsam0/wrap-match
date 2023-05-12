fn main() {}

struct Error;

#[wrap_match::wrap_match(error_message = "{error:?}")]
fn no_debug() -> Result<(), Error> {
    Err(Error)?;
    Ok(())
}

#[wrap_match::wrap_match(error_message = "{error}", error_message_without_info = "")]
fn no_display() -> Result<(), Error> {
    Err(Error)?;
    Ok(())
}
