#[deny(deprecated)]
fn main() {
    InnerDeprecated._wrap_match_inner_test().unwrap_err();
}

struct InnerDeprecated;

impl InnerDeprecated {
    #[wrap_match::wrap_match]
    pub fn test(&self) -> Result<(), ()> {
        Err(())?;
        Ok(())
    }
}
