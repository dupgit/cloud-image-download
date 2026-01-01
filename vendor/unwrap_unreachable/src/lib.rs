#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![no_std]
//! # Unwrap unreachable
//!
//! Provides `Option::unreachable()` and `Result::unreachable()` methods by providing an [`UnwrapUnreachable`] extension trait.
//!
//! # Usage
//!
//! ```ignore
//! // Make sure you import the trait. Otherwise, these functions will not be available.
//! use unwrap_unreachable::UnwrapUnreachable;
//!
//! // This regex is known to compile at compile time… so it will not panic
//! // at execution
//! static H5AI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"powered by h5ai (v\d+.\d+.\d+)").unreachable());
//!
//! // unreachable() here is ok since we know this should not return anything
//! // else than Ok(httpdir) if it does it should panic as the test fail.
//! let httpdir = prepare_httpdir().filter_by_name("debian").unreachable();
//! ```

use core::fmt::Debug;

/// Provide the `unreachable()` method on [`Option`] and [`Result`] types.
///
/// # Usage
///
/// Use the `.unreachable()` function like you would use [`Result::unwrap`] or [`Option::unwrap`].
/// This, however, will panic with the error message "internal error: entered unreachable code".
///
/// You should use this function in code that you know by design that it can not return anything
/// else than `Ok()` or `Some()` to clarify your intention.
pub trait UnwrapUnreachable {
    /// What `.unreachable()` should return.
    type Target;

    /// Unwrap the [`UnwrapUnreachable::Target`] value out of this type, panicking on other cases
    /// with a "internal error: entered unreachable code" message.
    fn unreachable(self) -> Self::Target;
}

impl<T> UnwrapUnreachable for Option<T> {
    type Target = T;

    /// Returns the contained [`Some`] value, consuming the self value.
    ///
    /// This will panic on [`None`], with the following message:
    /// "internal error: entered unreachable code"
    ///
    /// # Panics
    ///
    /// Panics if the value is [`None`].
    ///
    /// # Usage
    ///
    /// Use [`Option::unreachable`] like you would use [`Option::unwrap`], but in scenarios
    /// where panicking on None should never happen by design.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use unwrap_unreachable::UnwrapUnreachable;
    ///
    /// let date = NaiveDate::from_ymd_opt(2025, 05, 20).unreachable().and_hms_opt(20, 19, 00).unreachable();
    /// ```
    #[inline]
    #[track_caller]
    fn unreachable(self) -> Self::Target {
        match self {
            Some(t) => t,
            None => unwrap_none(),
        }
    }
}

#[cold]
#[inline(never)]
#[track_caller]
fn unwrap_none() -> ! {
    panic!("internal error: entered unreachable code")
}

impl<T, E> UnwrapUnreachable for Result<T, E>
where
    E: Debug,
{
    type Target = T;

    /// Returns the contained [`Ok`] value, consuming the self value.
    ///
    /// This will panic on error, with the following message:
    /// "internal error: entered unreachable code"
    ///
    /// # Panics
    ///
    /// Panics if the value is an [`Err`].
    ///
    /// # Usage
    ///
    /// Use [`Result::unreachable`] like you would use [`Result::unwrap`], but in scenarios
    /// where panicking on an error should never happen by design.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use unwrap_unreachable::UnwrapUnreachable;
    ///
    /// // This regex is known to compile at compile time… so it will not panic
    /// // at execution
    /// static H5AI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"powered by h5ai (v\d+.\d+.\d+)").unreachable());
    ///
    /// // In a test where you control the values:
    /// // unreachable() here is ok since we know this should not return anything
    /// // else than Ok(httpdir) if it does it should panic as the test fails.
    /// let httpdir = prepare_httpdir().filter_by_name("debian").unreachable();
    /// ```
    #[inline]
    #[track_caller]
    fn unreachable(self) -> Self::Target {
        match self {
            Ok(t) => t,
            Err(e) => unwrap_err(&e),
        }
    }
}

#[cold]
#[inline(never)]
#[track_caller]
fn unwrap_err(e: &dyn Debug) -> ! {
    panic!("internal error: entered unreachable code: {e:?}")
}
