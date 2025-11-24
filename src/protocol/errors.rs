//! X11 protocol error codes and error handling

use super::types::*;
use std::fmt;

/// X11 error codes as defined in the protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorCode {
    Request = 1,
    Value = 2,
    Window = 3,
    Pixmap = 4,
    Atom = 5,
    Cursor = 6,
    Font = 7,
    Match = 8,
    Drawable = 9,
    Access = 10,
    Alloc = 11,
    Colormap = 12,
    GContext = 13,
    IDChoice = 14,
    Name = 15,
    Length = 16,
    Implementation = 17,
}

impl ErrorCode {
    pub fn from_u8(code: u8) -> Option<Self> {
        match code {
            1 => Some(ErrorCode::Request),
            2 => Some(ErrorCode::Value),
            3 => Some(ErrorCode::Window),
            4 => Some(ErrorCode::Pixmap),
            5 => Some(ErrorCode::Atom),
            6 => Some(ErrorCode::Cursor),
            7 => Some(ErrorCode::Font),
            8 => Some(ErrorCode::Match),
            9 => Some(ErrorCode::Drawable),
            10 => Some(ErrorCode::Access),
            11 => Some(ErrorCode::Alloc),
            12 => Some(ErrorCode::Colormap),
            13 => Some(ErrorCode::GContext),
            14 => Some(ErrorCode::IDChoice),
            15 => Some(ErrorCode::Name),
            16 => Some(ErrorCode::Length),
            17 => Some(ErrorCode::Implementation),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::Request => "Request: bad request code",
            ErrorCode::Value => "Value: integer parameter out of range",
            ErrorCode::Window => "Window: invalid Window parameter",
            ErrorCode::Pixmap => "Pixmap: invalid Pixmap parameter",
            ErrorCode::Atom => "Atom: invalid Atom parameter",
            ErrorCode::Cursor => "Cursor: invalid Cursor parameter",
            ErrorCode::Font => "Font: invalid Font parameter",
            ErrorCode::Match => "Match: parameter mismatch",
            ErrorCode::Drawable => "Drawable: invalid Drawable parameter",
            ErrorCode::Access => "Access: attempt to access private resource",
            ErrorCode::Alloc => "Alloc: insufficient resources",
            ErrorCode::Colormap => "Colormap: invalid Colormap parameter",
            ErrorCode::GContext => "GContext: invalid GC parameter",
            ErrorCode::IDChoice => "IDChoice: invalid resource ID for this connection",
            ErrorCode::Name => "Name: font or color name doesn't exist",
            ErrorCode::Length => "Length: request length incorrect",
            ErrorCode::Implementation => "Implementation: server implementation error",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// X11 error packet
#[derive(Debug, Clone)]
pub struct X11Error {
    pub code: ErrorCode,
    pub sequence: u16,
    pub bad_value: u32,
    pub minor_opcode: u16,
    pub major_opcode: u8,
}

impl X11Error {
    pub fn new(
        code: ErrorCode,
        sequence: u16,
        bad_value: u32,
        minor_opcode: u16,
        major_opcode: u8,
    ) -> Self {
        X11Error {
            code,
            sequence,
            bad_value,
            minor_opcode,
            major_opcode,
        }
    }

    /// Encode error to wire format (32 bytes)
    pub fn encode(&self, buffer: &mut [u8]) {
        assert!(buffer.len() >= 32, "Error buffer must be at least 32 bytes");

        buffer[0] = 0; // Error reply type
        buffer[1] = self.code as u8;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.bad_value.to_ne_bytes());
        buffer[8..10].copy_from_slice(&self.minor_opcode.to_ne_bytes());
        buffer[10] = self.major_opcode;
        buffer[11..32].fill(0); // Padding
    }
}

impl fmt::Display for X11Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "X11 Error: {} (sequence: {}, value: 0x{:08x}, major: {}, minor: {})",
            self.code, self.sequence, self.bad_value, self.major_opcode, self.minor_opcode
        )
    }
}

/// Result type for X11 operations
pub type X11Result<T> = Result<T, X11Error>;

/// Helper functions to create common errors
impl X11Error {
    pub fn bad_request(sequence: u16, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Request, sequence, 0, 0, major_opcode)
    }

    pub fn bad_value(sequence: u16, value: u32, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Value, sequence, value, 0, major_opcode)
    }

    pub fn bad_window(sequence: u16, window: Window, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Window, sequence, window.id().get(), 0, major_opcode)
    }

    pub fn bad_pixmap(sequence: u16, pixmap: Pixmap, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Pixmap, sequence, pixmap.id().get(), 0, major_opcode)
    }

    pub fn bad_atom(sequence: u16, atom: Atom, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Atom, sequence, atom.get(), 0, major_opcode)
    }

    pub fn bad_drawable(sequence: u16, drawable: Drawable, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Drawable, sequence, drawable.id().get(), 0, major_opcode)
    }

    pub fn bad_gc(sequence: u16, gc: GContext, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::GContext, sequence, gc.id().get(), 0, major_opcode)
    }

    pub fn bad_match(sequence: u16, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Match, sequence, 0, 0, major_opcode)
    }

    pub fn bad_access(sequence: u16, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Access, sequence, 0, 0, major_opcode)
    }

    pub fn bad_alloc(sequence: u16, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Alloc, sequence, 0, 0, major_opcode)
    }

    pub fn bad_id_choice(sequence: u16, id: u32, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::IDChoice, sequence, id, 0, major_opcode)
    }

    pub fn bad_name(sequence: u16, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Name, sequence, 0, 0, major_opcode)
    }

    pub fn bad_length(sequence: u16, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Length, sequence, 0, 0, major_opcode)
    }

    pub fn implementation_error(sequence: u16, major_opcode: u8) -> Self {
        X11Error::new(ErrorCode::Implementation, sequence, 0, 0, major_opcode)
    }
}
