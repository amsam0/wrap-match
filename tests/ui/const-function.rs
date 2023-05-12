fn main() {}

#[wrap_match::wrap_match]
const fn const_function() -> Result<(), ()> {
    Ok(())
}
