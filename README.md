# The Problem

If you ever want to log when an error occurs and what caused it, you may find yourself using a `match` statement for every possible error instead of using the `?` operator.

This results in extremely verbose code. It's a pain to write and maintain.

# Introducing wrap-match!

wrap-match is an attribute macro that wraps your function in a `match` statement. Additionally, **it attaches rich error information to all statements using the `?` operator (aka try expressions).**
This allows you to know exactly what line and expression caused the error.

wrap-match supports both `log` and `tracing`. It defaults to `log`, but it will use `tracing` if the `tracing` feature is enabled. See [`tracing` support](#tracing-support) for more info.

> **Note**
>
> wrap-match uses the `log` or `tracing` crate to log success and error messages. It does not expose the `log` or `tracing` crate for expanded functions to use; you must depend on them yourself.
>
> Additionally, **no messages will appear unless you use a logging implementation or `tracing` subscriber.** For `log`, I recommend `env_logger`, but you can find a full list
> [here](https://docs.rs/log/#available-logging-implementations). For `tracing`, I recommend `tracing-subscriber`, but you can find a full list [here](https://docs.rs//tracing/#related-crates).

## Example

First, add this to your `Cargo.toml`:

```toml
[dependencies]
# For log users:
wrap-match = "1"
log = "*"
# You'll also want a logging implementation, for example `env_logger`
# More info here: https://docs.rs/log/#available-logging-implementations

# For tracing users:
wrap-match = { version = "1", features = ["tracing"] }
tracing = "0.1"
# You'll also want a `tracing` subscriber, for example `tracing-subscriber`
# More info here: https://docs.rs//tracing/#related-crates
```

Now you can use the `wrap_match` attribute macro:

```rust
#[wrap_match::wrap_match]
fn my_function() -> Result<(), CustomError> {
    Err(CustomError::Error)?; // notice the ?; when the macro is expanded, it will be modified to include line number and expression
    // If you need to return an error, just do `Err(CustomError::Error.into())`
    Ok(())
}
```

This would expand to something like this (comments are not included normally, and some of the output is simplified; use `cargo-expand` if you are curious what the macro really expands to):

```rust
fn my_function() -> Result<(), CustomError> {
    // This is where the original function is
    fn _wrap_match_inner_my_function() -> Result<(), WrapMatchError<CustomError>> {
        Err(CustomError::Error)
            .map_err(|e| WrapMatchError {
                // Here, line number and expression are added to the error
                line_and_expr: Some((3, "Err(CustomError::Error)")),
                inner: e.into(), // This is so you can have `Box<dyn Error>` as your error type
            })?;
        // If you need to return an error, just do `Err(CustomError::Error.into())`
        Ok(())
    }

    match _wrap_match_inner_my_function() {
        Ok(r) => {
            ::log::info!("Successfully ran my_function"); // when the tracing feature is enabled, it will use tracing macros instead
            Ok(r)
        }
        Err(e) => {
            if let Some((_line, _expr)) = e.line_and_expr {
                ::log::error!("An error occurred when running my_function (when running `{_expr}` on line {_line}): {:?}", e.inner); // when the tracing feature is enabled, it will use tracing macros instead
            } else {
                ::log::error!("An error occurred when running my_function: {:?}", e.inner); // when the tracing feature is enabled, it will use tracing macros instead
            }
            Err(e.inner)
        }
    }
}
```

If we run this code, it would log this:

```log
[ERROR] An error occurred when running my_function (when running `Err(CustomError::Error)` on line 3): Error
```

As you can see, wrap-match makes error logging extremely easy while still logging information like what caused the error.

## `tracing` support

Added in wrap-match 1.0.5, wrap-match supports `tracing` if the `tracing` feature is enabled. wrap-match **does not** do anything with spans. Additionally, you will not be able to manually create
spans in functions you use wrap-match on. This is because the span will be dropped before wrap-match logs anything.

To put both the function and wrap-match logs in a span, you have to use the `tracing::instrument` attribute macro. The ordering of the attribute macros is important; **it must go after wrap-match**.

Example:

```rust
#[wrap_match::wrap_match(success_message = "still in span!")]
#[tracing::instrument]
fn tracing_example() -> Result<(), ()> {
    tracing::info!("hello from tracing!");
    Ok(())
}
```

## Customization

wrap-match allows the user to customize success and error messages, as well as choosing whether or not to log anything on success.

### `success_message`

The message that's logged on success.

Available format parameters:

-   `{function}`: The original function name.

Default value: `Successfully ran {function}`

Example:

```rust
#[wrap_match::wrap_match(success_message = "{function} ran successfully!! 🎉🎉")]
fn my_function() -> Result<(), CustomError> {
    Ok(())
}
```

This would log:

```log
[INFO] my_function ran successfully!! 🎉🎉
```

### `error_message`

The message that's logged on error, when line and expression info **is** available. Currently, this is only for try expressions (expressions with a `?` after them).

Available format parameters:

-   `{function}`: The original function name.
-   `{line}`: The line the error occurred on.
-   `{expr}`: The expression that caused the error.
-   `{error}` or `{error:?}`: The error.

Default value: `` An error occurred when running {function} (caused by `{expr}` on line {line}): {error:?} ``

Example:

```rust
#[wrap_match::wrap_match(error_message = "oh no, {function} failed! `{expr}` on line {line} caused the error: {error:?}")]
fn my_function() -> Result<(), CustomError> {
    Err(CustomError::Error)?;
    Ok(())
}
```

This would log:

```log
[ERROR] oh no, my_function failed! `Err(CustomError::Error)` on line 3 caused the error: Error
```

### `error_message_without_info`

The message that's logged on error, when line and expression info **is not** available. This is usually triggered if you return an error yourself and use `.into()`.

Available format parameters:

-   `{function}`: The original function name.
-   `{error}` or `{error:?}`: The error.

Default value: `An error occurred when running {function}: {error:?}`

Example:

```rust
#[wrap_match::wrap_match(error_message_without_info = "oh no, {function} failed with this error: {error:?}")]
fn my_function() -> Result<(), CustomError> {
    Err(CustomError::Error.into())
}
```

This would log:

```log
[ERROR] oh no, my_function failed with this error: Error
```

### `log_success`

If `false`, nothing will be logged on success.

Default value: `true`

Example:

```rust
#[wrap_match::wrap_match(log_success = false)]
fn my_function() -> Result<(), CustomError> {
    Ok(())
}
```

This would log nothing.

### `disregard_result`

If `true`, the resulting function will return `()` and throw away whatever the `Result` is. Useful for `main` functions.

Default value: `false`

Example:

```rust
#[wrap_match::wrap_match(disregard_result = true)]
fn main() -> Result<(), CustomError> {
    Ok(())
}
```

The `main` function would be turned into this:

```rust
fn main() {
    fn _wrap_match_inner_main() -> Result<(), WrapMatchError<CustomError>> {
        Ok(())
    }

    match _wrap_match_inner_main() {
        // the Result would be logged like normal, but it is not returned
    }
}
```

## Limitations

wrap-match currently has the following limitations:

1.  ~~wrap-match cannot be used on functions in implementations that take a `self` parameter. If you need support for this, please create a GitHub issue with your use case.~~ This is now supported!
    However, it does require wrap-match to move the inner function out of the generated one, so it will add a new method to the implementation. This method is marked as deprecated, made private, and
    is not shown in documentation. Hopefully this won't cause any issues.

1.  wrap-match only supports `Result`s. If you need support for `Option`s, please create a GitHub issue with your use case.

1.  `error_message` and `error_message_without_info` only support formatting `error` using the `Debug` or `Display` formatters. This is because of how we determine what formatting specifiers are used.
    If you need support for other formatting specifiers, please create a GitHub issue with your use case.

1.  wrap-match cannot be used on `const` functions. This is because the `log` crate cannot be used in `const` contexts.

If wrap-match doesn't work for something not on this list, please create a GitHub issue!
