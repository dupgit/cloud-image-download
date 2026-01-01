# Unwrap Unreachable

Provides `Option::unreachable()` and `Result::unreachable()` methods by providing an `UnwrapUnreachable` extension trait.
Completely copied from [unwrap_todo](https://github.com/zkldi/unwrap_todo) crate. This crate exists because I think it
may value to test [Zk proposal](https://zkrising.com/writing/three-unwraps/) in real crates. It may be useful to you
or not ?

## Usage

Add the crate to your dependencies:

```toml
[dependencies]
unwrap_unreachable = "0.1.1"
```

Then use `.unreachable()` in lieu of `.unwrap()` to indicate that error handling is not needed by design.

```rust,no_run
// Make sure you import the trait. Otherwise, these functions will not be available.
use unwrap_unreachable::UnwrapUnreachable;

// This regex is known to compile at compile time… so it will not panic
// at execution
static H5AI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"powered by h5ai (v\d+.\d+.\d+)").unreachable());

// unreachable() here is ok since we know this should not return anything
// else than Ok(httpdir) if it does it should panic as the test fail.
let httpdir = prepare_httpdir().filter_by_name("debian").unreachable();
```

## Why?

Because of this [article](https://zkrising.com/writing/three-unwraps/) and that this crate did
not exist already whereas the [unwrap_todo](https://crates.io/crates/unwrap_todo) did and that I
have some pieces of code where `.unwrap()` is used for its `.unreachable()` meaning (type 2 in
the article).

A good example of this would be static Regex construction:

```rust
static REGEX: LazyCell<Regex> = LazyCell::new(|| Regex::new("^[a-f]{3}$").unwrap());
```

which cannot be elegantly done without an unwrap. This unwrap is *100%* intentional, and is not a
placeholder (TODO) for proper error handling.

As such, this crate provides `Option::unreachable()` and `Result::unreachable()` methods that perform
identically to `unwrap()`, but give a different error message (`"internal error: entered unreachable code"`)
and are visually distinct function calls in your codebase.

This makes it quite easy to see intentional `unwrap()` use, and then know where you need to focus
on your code (left `.unwrap()` or `.todo()`) to clean up before publishing.

This is - in a sense - very similar to the difference between the `panic!()` and `unreachable!()` macros.

## I'm having issues.

Make sure you use `unwrap_unreachable::UnwrapUnreachable` trait.
