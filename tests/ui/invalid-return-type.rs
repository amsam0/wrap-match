fn main() {}

#[wrap_match::wrap_match]
fn invalid_return_type() -> Option<()> {
    Some(())
}
