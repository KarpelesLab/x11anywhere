//! X11 protocol requests
//!
//! This module defines the X11 request opcodes and structures for parsing requests.

use super::types::*;
use super::errors::*;

/// X11 request opcodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RequestOpcode {
    CreateWindow = 1,
    ChangeWindowAttributes = 2,
    GetWindowAttributes = 3,
    DestroyWindow = 4,
    DestroySubwindows = 5,
    ChangeSaveSet = 6,
    ReparentWindow = 7,
    MapWindow = 8,
    MapSubwindows = 9,
    UnmapWindow = 10,
    UnmapSubwindows = 11,
    ConfigureWindow = 12,
    CirculateWindow = 13,
    GetGeometry = 14,
    QueryTree = 15,
    InternAtom = 16,
    GetAtomName = 17,
    ChangeProperty = 18,
    DeleteProperty = 19,
    GetProperty = 20,
    ListProperties = 21,
    SetSelectionOwner = 22,
    GetSelectionOwner = 23,
    ConvertSelection = 24,
    SendEvent = 25,
    GrabPointer = 26,
    UngrabPointer = 27,
    GrabButton = 28,
    UngrabButton = 29,
    ChangeActivePointerGrab = 30,
    GrabKeyboard = 31,
    UngrabKeyboard = 32,
    GrabKey = 33,
    UngrabKey = 34,
    AllowEvents = 35,
    GrabServer = 36,
    UngrabServer = 37,
    QueryPointer = 38,
    GetMotionEvents = 39,
    TranslateCoordinates = 40,
    WarpPointer = 41,
    SetInputFocus = 42,
    GetInputFocus = 43,
    QueryKeymap = 44,
    OpenFont = 45,
    CloseFont = 46,
    QueryFont = 47,
    QueryTextExtents = 48,
    ListFonts = 49,
    ListFontsWithInfo = 50,
    SetFontPath = 51,
    GetFontPath = 52,
    CreatePixmap = 53,
    FreePixmap = 54,
    CreateGC = 55,
    ChangeGC = 56,
    CopyGC = 57,
    SetDashes = 58,
    SetClipRectangles = 59,
    FreeGC = 60,
    ClearArea = 61,
    CopyArea = 62,
    CopyPlane = 63,
    PolyPoint = 64,
    PolyLine = 65,
    PolySegment = 66,
    PolyRectangle = 67,
    PolyArc = 68,
    FillPoly = 69,
    PolyFillRectangle = 70,
    PolyFillArc = 71,
    PutImage = 72,
    GetImage = 73,
    PolyText8 = 74,
    PolyText16 = 75,
    ImageText8 = 76,
    ImageText16 = 77,
    CreateColormap = 78,
    FreeColormap = 79,
    CopyColormapAndFree = 80,
    InstallColormap = 81,
    UninstallColormap = 82,
    ListInstalledColormaps = 83,
    AllocColor = 84,
    AllocNamedColor = 85,
    AllocColorCells = 86,
    AllocColorPlanes = 87,
    FreeColors = 88,
    StoreColors = 89,
    StoreNamedColor = 90,
    QueryColors = 91,
    LookupColor = 92,
    CreateCursor = 93,
    CreateGlyphCursor = 94,
    FreeCursor = 95,
    RecolorCursor = 96,
    QueryBestSize = 97,
    QueryExtension = 98,
    ListExtensions = 99,
    ChangeKeyboardMapping = 100,
    GetKeyboardMapping = 101,
    ChangeKeyboardControl = 102,
    GetKeyboardControl = 103,
    Bell = 104,
    ChangePointerControl = 105,
    GetPointerControl = 106,
    SetScreenSaver = 107,
    GetScreenSaver = 108,
    ChangeHosts = 109,
    ListHosts = 110,
    SetAccessControl = 111,
    SetCloseDownMode = 112,
    KillClient = 113,
    RotateProperties = 114,
    ForceScreenSaver = 115,
    SetPointerMapping = 116,
    GetPointerMapping = 117,
    SetModifierMapping = 118,
    GetModifierMapping = 119,
    NoOperation = 127,
}

impl RequestOpcode {
    pub fn from_u8(opcode: u8) -> Option<Self> {
        match opcode {
            1 => Some(RequestOpcode::CreateWindow),
            2 => Some(RequestOpcode::ChangeWindowAttributes),
            3 => Some(RequestOpcode::GetWindowAttributes),
            4 => Some(RequestOpcode::DestroyWindow),
            5 => Some(RequestOpcode::DestroySubwindows),
            6 => Some(RequestOpcode::ChangeSaveSet),
            7 => Some(RequestOpcode::ReparentWindow),
            8 => Some(RequestOpcode::MapWindow),
            9 => Some(RequestOpcode::MapSubwindows),
            10 => Some(RequestOpcode::UnmapWindow),
            11 => Some(RequestOpcode::UnmapSubwindows),
            12 => Some(RequestOpcode::ConfigureWindow),
            13 => Some(RequestOpcode::CirculateWindow),
            14 => Some(RequestOpcode::GetGeometry),
            15 => Some(RequestOpcode::QueryTree),
            16 => Some(RequestOpcode::InternAtom),
            17 => Some(RequestOpcode::GetAtomName),
            18 => Some(RequestOpcode::ChangeProperty),
            19 => Some(RequestOpcode::DeleteProperty),
            20 => Some(RequestOpcode::GetProperty),
            21 => Some(RequestOpcode::ListProperties),
            22 => Some(RequestOpcode::SetSelectionOwner),
            23 => Some(RequestOpcode::GetSelectionOwner),
            24 => Some(RequestOpcode::ConvertSelection),
            25 => Some(RequestOpcode::SendEvent),
            43 => Some(RequestOpcode::GetInputFocus),
            45 => Some(RequestOpcode::OpenFont),
            46 => Some(RequestOpcode::CloseFont),
            47 => Some(RequestOpcode::QueryFont),
            53 => Some(RequestOpcode::CreatePixmap),
            54 => Some(RequestOpcode::FreePixmap),
            55 => Some(RequestOpcode::CreateGC),
            56 => Some(RequestOpcode::ChangeGC),
            60 => Some(RequestOpcode::FreeGC),
            61 => Some(RequestOpcode::ClearArea),
            62 => Some(RequestOpcode::CopyArea),
            64 => Some(RequestOpcode::PolyPoint),
            65 => Some(RequestOpcode::PolyLine),
            66 => Some(RequestOpcode::PolySegment),
            67 => Some(RequestOpcode::PolyRectangle),
            68 => Some(RequestOpcode::PolyArc),
            69 => Some(RequestOpcode::FillPoly),
            70 => Some(RequestOpcode::PolyFillRectangle),
            71 => Some(RequestOpcode::PolyFillArc),
            72 => Some(RequestOpcode::PutImage),
            73 => Some(RequestOpcode::GetImage),
            74 => Some(RequestOpcode::PolyText8),
            76 => Some(RequestOpcode::ImageText8),
            85 => Some(RequestOpcode::AllocNamedColor),
            94 => Some(RequestOpcode::CreateGlyphCursor),
            98 => Some(RequestOpcode::QueryExtension),
            99 => Some(RequestOpcode::ListExtensions),
            104 => Some(RequestOpcode::Bell),
            127 => Some(RequestOpcode::NoOperation),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RequestOpcode::CreateWindow => "CreateWindow",
            RequestOpcode::ChangeWindowAttributes => "ChangeWindowAttributes",
            RequestOpcode::GetWindowAttributes => "GetWindowAttributes",
            RequestOpcode::DestroyWindow => "DestroyWindow",
            RequestOpcode::MapWindow => "MapWindow",
            RequestOpcode::UnmapWindow => "UnmapWindow",
            RequestOpcode::ConfigureWindow => "ConfigureWindow",
            RequestOpcode::InternAtom => "InternAtom",
            RequestOpcode::GetAtomName => "GetAtomName",
            RequestOpcode::ChangeProperty => "ChangeProperty",
            RequestOpcode::GetProperty => "GetProperty",
            RequestOpcode::CreatePixmap => "CreatePixmap",
            RequestOpcode::FreePixmap => "FreePixmap",
            RequestOpcode::CreateGC => "CreateGC",
            RequestOpcode::ChangeGC => "ChangeGC",
            RequestOpcode::FreeGC => "FreeGC",
            RequestOpcode::PolyRectangle => "PolyRectangle",
            RequestOpcode::PolyFillRectangle => "PolyFillRectangle",
            RequestOpcode::CopyArea => "CopyArea",
            RequestOpcode::ImageText8 => "ImageText8",
            _ => "Unknown",
        }
    }
}

/// Request header (common to all requests)
#[derive(Debug, Clone)]
pub struct RequestHeader {
    pub opcode: u8,
    pub detail: u8,  // Request-specific detail byte
    pub length: u16,  // Length in 4-byte units
}

impl RequestHeader {
    /// Parse request header from buffer
    pub fn parse(buffer: &[u8]) -> Result<Self, X11Error> {
        if buffer.len() < 4 {
            return Err(X11Error::bad_length(0, 0));
        }

        Ok(RequestHeader {
            opcode: buffer[0],
            detail: buffer[1],
            length: u16::from_ne_bytes([buffer[2], buffer[3]]),
        })
    }

    /// Get total request size in bytes
    pub fn size(&self) -> usize {
        (self.length as usize) * 4
    }
}

/// Helper to read values from request buffer
pub struct RequestReader<'a> {
    buffer: &'a [u8],
    offset: usize,
}

impl<'a> RequestReader<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        RequestReader { buffer, offset: 0 }
    }

    pub fn skip(&mut self, n: usize) {
        self.offset += n;
    }

    pub fn read_u8(&mut self) -> u8 {
        let val = self.buffer[self.offset];
        self.offset += 1;
        val
    }

    pub fn read_u16(&mut self) -> u16 {
        let val = u16::from_ne_bytes([
            self.buffer[self.offset],
            self.buffer[self.offset + 1],
        ]);
        self.offset += 2;
        val
    }

    pub fn read_u32(&mut self) -> u32 {
        let val = u32::from_ne_bytes([
            self.buffer[self.offset],
            self.buffer[self.offset + 1],
            self.buffer[self.offset + 2],
            self.buffer[self.offset + 3],
        ]);
        self.offset += 4;
        val
    }

    pub fn read_i16(&mut self) -> i16 {
        let val = i16::from_ne_bytes([
            self.buffer[self.offset],
            self.buffer[self.offset + 1],
        ]);
        self.offset += 2;
        val
    }

    pub fn read_window(&mut self) -> Window {
        Window::new(self.read_u32())
    }

    pub fn read_pixmap(&mut self) -> Pixmap {
        Pixmap::new(self.read_u32())
    }

    pub fn read_drawable(&mut self) -> Drawable {
        Drawable::from_id(self.read_u32())
    }

    pub fn read_gcontext(&mut self) -> GContext {
        GContext::new(self.read_u32())
    }

    pub fn read_atom(&mut self) -> Atom {
        Atom::new(self.read_u32())
    }

    pub fn read_bytes(&mut self, len: usize) -> &'a [u8] {
        let slice = &self.buffer[self.offset..self.offset + len];
        self.offset += len;
        slice
    }

    pub fn remaining(&self) -> &'a [u8] {
        &self.buffer[self.offset..]
    }
}
