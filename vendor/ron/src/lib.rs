//! (no_std port for the GBA/thumbv4t build: `#![no_std]` + alloc on
//! `target_os = "none"`; hosted builds keep std unchanged. `std::io` is
//! shimmed — see the `io` item below.)
#![doc = include_str!("../README.md")]
#![doc(html_root_url = "https://docs.rs/ron/0.8.1")]
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", feature(prelude_import))]
#![cfg_attr(target_os = "none", allow(internal_features))]

use crate::alloc_prelude::*;
extern crate alloc;

#[allow(unused_imports)]
mod alloc_prelude {
    pub use core::prelude::v1::*;
    pub use core::convert::{TryFrom, TryInto};
    pub use alloc::borrow::ToOwned;
    pub use core::iter::FromIterator;
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
    pub use core::{assert_eq, assert_ne, matches, todo, unimplemented, write, writeln};
    pub use core::debug_assert;
}

#[cfg_attr(target_os = "none", prelude_import)]
#[allow(unused_imports)]
use alloc_prelude::*;

// `io` for the serializer/deserializer bounds: real `std::io` on hosted
// targets; a minimal byte-sink-only shim on bare-metal (ron's serializer
// writes into `Vec<u8>`/`String` sinks, never files/sockets there).
#[cfg(not(target_os = "none"))]
pub(crate) use std::io;

#[cfg(target_os = "none")]
pub(crate) mod io {
    use alloc::string::String;
    use alloc::vec::Vec;

    /// Stand-in for `std::io::Error`: carries the wrapped ron error (write
    /// sinks on bare-metal cannot fail except for formatting/alloc issues).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Error(pub crate::error::Error);

    impl core::fmt::Display for Error {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            core::fmt::Display::fmt(&self.0, f)
        }
    }

    pub type Result<T> = core::result::Result<T, Error>;

    pub trait Write {
        fn write_all(&mut self, buf: &[u8]) -> Result<()>;
        fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<()>;
    }

    impl Write for Vec<u8> {
        fn write_all(&mut self, buf: &[u8]) -> Result<()> {
            self.extend_from_slice(buf);
            Ok(())
        }

        fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<()> {
            let mut s = String::new();
            core::fmt::write(&mut s, args)
                .map_err(|_| Error(crate::error::Error::Message("fmt write failed".to_string())))?;
            self.extend_from_slice(s.as_bytes());
            Ok(())
        }
    }

    impl Write for &mut Vec<u8> {
        fn write_all(&mut self, buf: &[u8]) -> Result<()> {
            (**self).write_all(buf)
        }

        fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<()> {
            (**self).write_fmt(args)
        }
    }

    impl Write for String {
        fn write_all(&mut self, buf: &[u8]) -> Result<()> {
            self.push_str(
                &core::str::from_utf8(buf)
                    .map_err(|_| Error(crate::error::Error::Message("invalid utf-8".to_string())))?,
            );
            Ok(())
        }

        fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<()> {
            core::fmt::write(self, args)
                .map_err(|_| Error(crate::error::Error::Message("fmt write failed".to_string())))?;
            Ok(())
        }
    }

    impl Write for &mut String {
        fn write_all(&mut self, buf: &[u8]) -> Result<()> {
            (**self).write_all(buf)
        }

        fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<()> {
            (**self).write_fmt(args)
        }
    }
}

pub mod de;
pub mod ser;

pub mod error;
pub mod value;

pub mod extensions;

pub mod options;

pub use de::{from_str, Deserializer};
pub use error::{Error, Result};
pub use options::Options;
pub use ser::{to_string, Serializer};
pub use value::{Map, Number, Value};

mod parse;
