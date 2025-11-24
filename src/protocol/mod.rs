/// X11 protocol implementation
///
/// This module implements the X11 wire protocol, including types, requests,
/// replies, events, and errors.

pub mod types;
pub mod errors;
pub mod events;
pub mod requests;
pub mod setup;
pub mod parser;
pub mod encoder;

pub use types::*;
pub use errors::*;
pub use events::*;
pub use requests::*;
pub use setup::*;
pub use parser::*;
pub use encoder::*;

/// X11 protocol version
pub const PROTOCOL_MAJOR_VERSION: u16 = 11;
pub const PROTOCOL_MINOR_VERSION: u16 = 0;

/// Padding helper - X11 requires data to be padded to 4-byte boundaries
pub fn pad(n: usize) -> usize {
    (4 - (n % 4)) % 4
}

/// Calculate padded length
pub fn padded_len(n: usize) -> usize {
    n + pad(n)
}
