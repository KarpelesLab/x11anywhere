//! X11 request parser
//!
//! This module parses X11 requests from the wire protocol.

use super::*;

/// Parsed X11 request
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone)]
pub enum Request {
    CreateWindow(CreateWindowRequest),
    ChangeWindowAttributes(ChangeWindowAttributesRequest),
    GetWindowAttributes(GetWindowAttributesRequest),
    DestroyWindow(DestroyWindowRequest),
    MapWindow(MapWindowRequest),
    UnmapWindow(UnmapWindowRequest),
    ConfigureWindow(ConfigureWindowRequest),
    GetGeometry(GetGeometryRequest),
    QueryTree(QueryTreeRequest),
    InternAtom(InternAtomRequest),
    GetAtomName(GetAtomNameRequest),
    ChangeProperty(ChangePropertyRequest),
    GetProperty(GetPropertyRequest),
    CreateGC(CreateGCRequest),
    ChangeGC(ChangeGCRequest),
    FreeGC(FreeGCRequest),
    ClearArea(ClearAreaRequest),
    PolyRectangle(PolyRectangleRequest),
    PolyFillRectangle(PolyFillRectangleRequest),
    PolyLine(PolyLineRequest),
    PolyPoint(PolyPointRequest),
    ImageText8(ImageText8Request),
    CreatePixmap(CreatePixmapRequest),
    FreePixmap(FreePixmapRequest),
    PutImage(PutImageRequest),
    OpenFont(OpenFontRequest),
    CloseFont(CloseFontRequest),
    CreateGlyphCursor(CreateGlyphCursorRequest),
    AllocNamedColor(AllocNamedColorRequest),
    AllocColor(AllocColorRequest),
    QueryFont(QueryFontRequest),
    QueryExtension(QueryExtensionRequest),
    SetSelectionOwner(SetSelectionOwnerRequest),
    GetSelectionOwner(GetSelectionOwnerRequest),
    ConvertSelection(ConvertSelectionRequest),
    GetInputFocus,
    PolySegment(PolySegmentRequest),
    PolyArc(PolyArcRequest),
    PolyFillArc(PolyFillArcRequest),
    CopyArea(CopyAreaRequest),
    GetImage(GetImageRequest),
    FillPoly(FillPolyRequest),
    ExtensionRequest { opcode: u8, data: Vec<u8> },
    NoOperation,
}

/// Create window request
#[derive(Debug, Clone)]
pub struct CreateWindowRequest {
    pub depth: u8,
    pub wid: Window,
    pub parent: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub class: WindowClass,
    pub visual: VisualID,
    pub background_pixel: Option<u32>,
    pub border_pixel: Option<u32>,
    pub event_mask: Option<u32>,
    pub cursor: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ChangeWindowAttributesRequest {
    pub window: Window,
    pub background_pixel: Option<u32>,
    pub event_mask: Option<u32>,
    pub cursor: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct GetWindowAttributesRequest {
    pub window: Window,
}

#[derive(Debug, Clone)]
pub struct DestroyWindowRequest {
    pub window: Window,
}

#[derive(Debug, Clone)]
pub struct MapWindowRequest {
    pub window: Window,
}

#[derive(Debug, Clone)]
pub struct UnmapWindowRequest {
    pub window: Window,
}

#[derive(Debug, Clone)]
pub struct ConfigureWindowRequest {
    pub window: Window,
    pub x: Option<i16>,
    pub y: Option<i16>,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub border_width: Option<u16>,
    pub stack_mode: Option<StackMode>,
}

#[derive(Debug, Clone)]
pub struct GetGeometryRequest {
    pub drawable: Drawable,
}

#[derive(Debug, Clone)]
pub struct QueryTreeRequest {
    pub window: Window,
}

#[derive(Debug, Clone)]
pub struct InternAtomRequest {
    pub only_if_exists: bool,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct GetAtomNameRequest {
    pub atom: Atom,
}

#[derive(Debug, Clone)]
pub struct ChangePropertyRequest {
    pub window: Window,
    pub property: Atom,
    pub type_: Atom,
    pub format: u8,
    pub mode: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct GetPropertyRequest {
    pub delete: bool,
    pub window: Window,
    pub property: Atom,
    pub type_: Atom,
    pub long_offset: u32,
    pub long_length: u32,
}

#[derive(Debug, Clone)]
pub struct SetSelectionOwnerRequest {
    pub owner: Window,
    pub selection: Atom,
    pub time: u32,
}

#[derive(Debug, Clone)]
pub struct GetSelectionOwnerRequest {
    pub selection: Atom,
}

#[derive(Debug, Clone)]
pub struct ConvertSelectionRequest {
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub property: Atom,
    pub time: u32,
}

#[derive(Debug, Clone)]
pub struct CreateGCRequest {
    pub cid: GContext,
    pub drawable: Drawable,
    pub foreground: Option<u32>,
    pub background: Option<u32>,
    pub line_width: Option<u16>,
    pub function: Option<GCFunction>,
}

#[derive(Debug, Clone)]
pub struct ChangeGCRequest {
    pub gc: GContext,
    pub foreground: Option<u32>,
    pub background: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct FreeGCRequest {
    pub gc: GContext,
}

#[derive(Debug, Clone)]
pub struct ClearAreaRequest {
    pub exposures: bool,
    pub window: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone)]
pub struct PolyRectangleRequest {
    pub drawable: Drawable,
    pub gc: GContext,
    pub rectangles: Vec<Rectangle>,
}

#[derive(Debug, Clone)]
pub struct PolyFillRectangleRequest {
    pub drawable: Drawable,
    pub gc: GContext,
    pub rectangles: Vec<Rectangle>,
}

#[derive(Debug, Clone)]
pub struct PolyLineRequest {
    pub coordinate_mode: u8,
    pub drawable: Drawable,
    pub gc: GContext,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone)]
pub struct PolyPointRequest {
    pub coordinate_mode: u8,
    pub drawable: Drawable,
    pub gc: GContext,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone)]
pub struct ImageText8Request {
    pub drawable: Drawable,
    pub gc: GContext,
    pub x: i16,
    pub y: i16,
    pub string: String,
}

#[derive(Debug, Clone)]
pub struct CreatePixmapRequest {
    pub depth: u8,
    pub pid: Pixmap,
    pub drawable: Drawable,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone)]
pub struct FreePixmapRequest {
    pub pixmap: Pixmap,
}

#[derive(Debug, Clone)]
pub struct PutImageRequest {
    pub format: u8,
    pub drawable: Drawable,
    pub gc: GContext,
    pub width: u16,
    pub height: u16,
    pub dst_x: i16,
    pub dst_y: i16,
    pub left_pad: u8,
    pub depth: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct OpenFontRequest {
    pub fid: u32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct CloseFontRequest {
    pub font: u32,
}

#[derive(Debug, Clone)]
pub struct CreateGlyphCursorRequest {
    pub cid: u32,
    pub source_font: u32,
    pub mask_font: u32,
}

#[derive(Debug, Clone)]
pub struct AllocNamedColorRequest {
    pub colormap: u32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct AllocColorRequest {
    pub colormap: u32,
    pub red: u16,
    pub green: u16,
    pub blue: u16,
}

#[derive(Debug, Clone)]
pub struct QueryFontRequest {
    pub font: u32,
}

#[derive(Debug, Clone)]
pub struct QueryExtensionRequest {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct PolySegmentRequest {
    pub drawable: Drawable,
    pub gc: GContext,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone)]
pub struct PolyArcRequest {
    pub drawable: Drawable,
    pub gc: GContext,
    pub arcs: Vec<Arc>,
}

#[derive(Debug, Clone)]
pub struct PolyFillArcRequest {
    pub drawable: Drawable,
    pub gc: GContext,
    pub arcs: Vec<Arc>,
}

#[derive(Debug, Clone)]
pub struct CopyAreaRequest {
    pub src_drawable: Drawable,
    pub dst_drawable: Drawable,
    pub gc: GContext,
    pub src_x: i16,
    pub src_y: i16,
    pub dst_x: i16,
    pub dst_y: i16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone)]
pub struct GetImageRequest {
    pub format: u8,
    pub drawable: Drawable,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub plane_mask: u32,
}

#[derive(Debug, Clone)]
pub struct FillPolyRequest {
    pub drawable: Drawable,
    pub gc: GContext,
    pub shape: u8,
    pub coordinate_mode: u8,
    pub points: Vec<Point>,
}

/// Request parser
pub struct ProtocolParser {
    byte_order: ByteOrder,
}

impl ProtocolParser {
    pub fn new(byte_order: ByteOrder) -> Self {
        ProtocolParser { byte_order }
    }

    /// Parse a request from buffer
    pub fn parse_request(&self, buffer: &[u8]) -> Result<(Request, u16), X11Error> {
        if buffer.len() < 4 {
            return Err(X11Error::bad_length(0, buffer[0]));
        }

        let opcode = buffer[0];
        let detail = buffer[1];
        let length = self.read_u16(&buffer[2..4]);
        let sequence = 0; // Sequence is maintained by connection, not in request

        let request_size = (length as usize) * 4;
        if buffer.len() < request_size {
            return Err(X11Error::bad_length(sequence, opcode));
        }

        let request_data = &buffer[4..request_size];

        log::debug!(
            "Parsing request: opcode={}, detail={}, length={}",
            opcode,
            detail,
            length
        );

        let request = match RequestOpcode::from_u8(opcode) {
            Some(RequestOpcode::CreateWindow) => self.parse_create_window(detail, request_data)?,
            Some(RequestOpcode::ChangeWindowAttributes) => {
                self.parse_change_window_attributes(request_data)?
            }
            Some(RequestOpcode::GetWindowAttributes) => {
                self.parse_get_window_attributes(request_data)?
            }
            Some(RequestOpcode::DestroyWindow) => self.parse_destroy_window(request_data)?,
            Some(RequestOpcode::MapWindow) => self.parse_map_window(request_data)?,
            Some(RequestOpcode::UnmapWindow) => self.parse_unmap_window(request_data)?,
            Some(RequestOpcode::ConfigureWindow) => self.parse_configure_window(request_data)?,
            Some(RequestOpcode::GetGeometry) => self.parse_get_geometry(request_data)?,
            Some(RequestOpcode::QueryTree) => self.parse_query_tree(request_data)?,
            Some(RequestOpcode::InternAtom) => self.parse_intern_atom(detail, request_data)?,
            Some(RequestOpcode::GetAtomName) => self.parse_get_atom_name(request_data)?,
            Some(RequestOpcode::ChangeProperty) => {
                self.parse_change_property(detail, request_data)?
            }
            Some(RequestOpcode::GetProperty) => self.parse_get_property(detail, request_data)?,
            Some(RequestOpcode::CreateGC) => self.parse_create_gc(request_data)?,
            Some(RequestOpcode::ChangeGC) => self.parse_change_gc(request_data)?,
            Some(RequestOpcode::FreeGC) => self.parse_free_gc(request_data)?,
            Some(RequestOpcode::ClearArea) => self.parse_clear_area(detail, request_data)?,
            Some(RequestOpcode::PolyRectangle) => self.parse_poly_rectangle(request_data)?,
            Some(RequestOpcode::PolyFillRectangle) => {
                self.parse_poly_fill_rectangle(request_data)?
            }
            Some(RequestOpcode::PolyLine) => self.parse_poly_line(detail, request_data)?,
            Some(RequestOpcode::PolyPoint) => self.parse_poly_point(detail, request_data)?,
            Some(RequestOpcode::ImageText8) => self.parse_image_text8(detail, request_data)?,
            Some(RequestOpcode::CreatePixmap) => self.parse_create_pixmap(detail, request_data)?,
            Some(RequestOpcode::FreePixmap) => self.parse_free_pixmap(request_data)?,
            Some(RequestOpcode::PutImage) => self.parse_put_image(detail, request_data)?,
            Some(RequestOpcode::OpenFont) => self.parse_open_font(request_data)?,
            Some(RequestOpcode::CloseFont) => self.parse_close_font(request_data)?,
            Some(RequestOpcode::CreateGlyphCursor) => {
                self.parse_create_glyph_cursor(request_data)?
            }
            Some(RequestOpcode::AllocNamedColor) => self.parse_alloc_named_color(request_data)?,
            Some(RequestOpcode::AllocColor) => self.parse_alloc_color(request_data)?,
            Some(RequestOpcode::QueryFont) => self.parse_query_font(request_data)?,
            Some(RequestOpcode::QueryExtension) => self.parse_query_extension(request_data)?,
            Some(RequestOpcode::SetSelectionOwner) => {
                self.parse_set_selection_owner(request_data)?
            }
            Some(RequestOpcode::GetSelectionOwner) => {
                self.parse_get_selection_owner(request_data)?
            }
            Some(RequestOpcode::ConvertSelection) => self.parse_convert_selection(request_data)?,
            Some(RequestOpcode::GetInputFocus) => Request::GetInputFocus,
            Some(RequestOpcode::PolySegment) => self.parse_poly_segment(request_data)?,
            Some(RequestOpcode::PolyArc) => self.parse_poly_arc(request_data)?,
            Some(RequestOpcode::FillPoly) => self.parse_fill_poly(detail, request_data)?,
            Some(RequestOpcode::PolyFillArc) => self.parse_poly_fill_arc(request_data)?,
            Some(RequestOpcode::CopyArea) => self.parse_copy_area(request_data)?,
            Some(RequestOpcode::GetImage) => self.parse_get_image(detail, request_data)?,
            Some(RequestOpcode::NoOperation) => Request::NoOperation,
            _ => {
                // Handle extension requests (opcodes >= 128)
                if opcode >= 128 {
                    log::debug!(
                        "Extension request: opcode={}, length={}",
                        opcode,
                        request_size
                    );
                    Request::ExtensionRequest {
                        opcode,
                        data: request_data.to_vec(),
                    }
                } else {
                    log::warn!("Unimplemented request opcode: {}", opcode);
                    return Err(X11Error::implementation_error(sequence, opcode));
                }
            }
        };

        Ok((request, length))
    }

    // Helper methods for reading with correct byte order
    fn read_u16(&self, bytes: &[u8]) -> u16 {
        match self.byte_order {
            ByteOrder::MSBFirst => u16::from_be_bytes([bytes[0], bytes[1]]),
            ByteOrder::LSBFirst => u16::from_le_bytes([bytes[0], bytes[1]]),
        }
    }

    fn read_u32(&self, bytes: &[u8]) -> u32 {
        match self.byte_order {
            ByteOrder::MSBFirst => u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            ByteOrder::LSBFirst => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        }
    }

    fn read_i16(&self, bytes: &[u8]) -> i16 {
        match self.byte_order {
            ByteOrder::MSBFirst => i16::from_be_bytes([bytes[0], bytes[1]]),
            ByteOrder::LSBFirst => i16::from_le_bytes([bytes[0], bytes[1]]),
        }
    }

    // Request parsers
    fn parse_create_window(&self, depth: u8, data: &[u8]) -> Result<Request, X11Error> {
        let wid = Window::new(self.read_u32(&data[0..4]));
        let parent = Window::new(self.read_u32(&data[4..8]));
        let x = self.read_i16(&data[8..10]);
        let y = self.read_i16(&data[10..12]);
        let width = self.read_u16(&data[12..14]);
        let height = self.read_u16(&data[14..16]);
        let border_width = self.read_u16(&data[16..18]);
        let class = WindowClass::from_u16(self.read_u16(&data[18..20]))
            .ok_or_else(|| X11Error::bad_value(0, 0, 1))?;
        let visual = VisualID::new(self.read_u32(&data[20..24]));
        let value_mask = self.read_u32(&data[24..28]);

        // Parse value list - values appear in bit order
        let mut background_pixel = None;
        let mut border_pixel = None;
        let mut event_mask = None;
        let mut cursor = None;
        let mut offset = 28;

        // Bit 0: BackPixmap (skip)
        if value_mask & (1 << 0) != 0 {
            offset += 4;
        }
        // Bit 1: BackPixel
        if value_mask & (1 << 1) != 0 {
            background_pixel = Some(self.read_u32(&data[offset..offset + 4]));
            offset += 4;
        }
        // Bit 2: BorderPixmap (skip)
        if value_mask & (1 << 2) != 0 {
            offset += 4;
        }
        // Bit 3: BorderPixel
        if value_mask & (1 << 3) != 0 {
            border_pixel = Some(self.read_u32(&data[offset..offset + 4]));
            offset += 4;
        }
        // Bits 4-10: Skip (BitGravity, WinGravity, BackingStore, BackingPlanes, BackingPixel, OverrideRedirect, SaveUnder)
        for bit in 4..=10 {
            if value_mask & (1 << bit) != 0 {
                offset += 4;
            }
        }
        // Bit 11: EventMask
        if value_mask & (1 << 11) != 0 {
            event_mask = Some(self.read_u32(&data[offset..offset + 4]));
            offset += 4;
        }
        // Bit 12: DoNotPropagateMask (skip)
        if value_mask & (1 << 12) != 0 {
            offset += 4;
        }
        // Bit 13: Colormap (skip)
        if value_mask & (1 << 13) != 0 {
            offset += 4;
        }
        // Bit 14: Cursor
        if value_mask & (1 << 14) != 0 {
            cursor = Some(self.read_u32(&data[offset..offset + 4]));
        }

        Ok(Request::CreateWindow(CreateWindowRequest {
            depth,
            wid,
            parent,
            x,
            y,
            width,
            height,
            border_width,
            class,
            visual,
            background_pixel,
            border_pixel,
            event_mask,
            cursor,
        }))
    }

    fn parse_change_window_attributes(&self, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        let value_mask = self.read_u32(&data[4..8]);

        // Parse value list - values appear in bit order
        let mut background_pixel = None;
        let mut event_mask = None;
        let mut cursor = None;
        let mut offset = 8;

        // Bit 0: BackPixmap (skip)
        if value_mask & (1 << 0) != 0 {
            offset += 4;
        }
        // Bit 1: BackPixel
        if value_mask & (1 << 1) != 0 {
            background_pixel = Some(self.read_u32(&data[offset..offset + 4]));
            offset += 4;
        }
        // Bits 2-10: Skip
        for bit in 2..=10 {
            if value_mask & (1 << bit) != 0 {
                offset += 4;
            }
        }
        // Bit 11: EventMask
        if value_mask & (1 << 11) != 0 {
            event_mask = Some(self.read_u32(&data[offset..offset + 4]));
            offset += 4;
        }
        // Bits 12-13: Skip
        for bit in 12..=13 {
            if value_mask & (1 << bit) != 0 {
                offset += 4;
            }
        }
        // Bit 14: Cursor
        if value_mask & (1 << 14) != 0 {
            cursor = Some(self.read_u32(&data[offset..offset + 4]));
        }

        Ok(Request::ChangeWindowAttributes(
            ChangeWindowAttributesRequest {
                window,
                background_pixel,
                event_mask,
                cursor,
            },
        ))
    }

    fn parse_get_window_attributes(&self, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        Ok(Request::GetWindowAttributes(GetWindowAttributesRequest {
            window,
        }))
    }

    fn parse_destroy_window(&self, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        Ok(Request::DestroyWindow(DestroyWindowRequest { window }))
    }

    fn parse_map_window(&self, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        Ok(Request::MapWindow(MapWindowRequest { window }))
    }

    fn parse_unmap_window(&self, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        Ok(Request::UnmapWindow(UnmapWindowRequest { window }))
    }

    fn parse_configure_window(&self, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        Ok(Request::ConfigureWindow(ConfigureWindowRequest {
            window,
            x: None,
            y: None,
            width: None,
            height: None,
            border_width: None,
            stack_mode: None,
        }))
    }

    fn parse_get_geometry(&self, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        Ok(Request::GetGeometry(GetGeometryRequest { drawable }))
    }

    fn parse_query_tree(&self, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        Ok(Request::QueryTree(QueryTreeRequest { window }))
    }

    fn parse_intern_atom(&self, only_if_exists: u8, data: &[u8]) -> Result<Request, X11Error> {
        let name_len = self.read_u16(&data[0..2]) as usize;
        let name = String::from_utf8_lossy(&data[4..4 + name_len]).to_string();
        Ok(Request::InternAtom(InternAtomRequest {
            only_if_exists: only_if_exists != 0,
            name,
        }))
    }

    fn parse_get_atom_name(&self, data: &[u8]) -> Result<Request, X11Error> {
        let atom = Atom::new(self.read_u32(&data[0..4]));
        Ok(Request::GetAtomName(GetAtomNameRequest { atom }))
    }

    fn parse_change_property(&self, mode: u8, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        let property = Atom::new(self.read_u32(&data[4..8]));
        let type_ = Atom::new(self.read_u32(&data[8..12]));
        let format = data[12];
        let data_len = self.read_u32(&data[16..20]) as usize;
        let prop_data = data[20..20 + data_len].to_vec();

        Ok(Request::ChangeProperty(ChangePropertyRequest {
            window,
            property,
            type_,
            format,
            mode,
            data: prop_data,
        }))
    }

    fn parse_get_property(&self, delete: u8, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        let property = Atom::new(self.read_u32(&data[4..8]));
        let type_ = Atom::new(self.read_u32(&data[8..12]));
        let long_offset = self.read_u32(&data[12..16]);
        let long_length = self.read_u32(&data[16..20]);

        Ok(Request::GetProperty(GetPropertyRequest {
            delete: delete != 0,
            window,
            property,
            type_,
            long_offset,
            long_length,
        }))
    }

    fn parse_create_gc(&self, data: &[u8]) -> Result<Request, X11Error> {
        let cid = GContext::new(self.read_u32(&data[0..4]));
        let drawable = Drawable::from_id(self.read_u32(&data[4..8]));

        Ok(Request::CreateGC(CreateGCRequest {
            cid,
            drawable,
            foreground: None,
            background: None,
            line_width: None,
            function: None,
        }))
    }

    fn parse_change_gc(&self, data: &[u8]) -> Result<Request, X11Error> {
        let gc = GContext::new(self.read_u32(&data[0..4]));

        Ok(Request::ChangeGC(ChangeGCRequest {
            gc,
            foreground: None,
            background: None,
        }))
    }

    fn parse_free_gc(&self, data: &[u8]) -> Result<Request, X11Error> {
        let gc = GContext::new(self.read_u32(&data[0..4]));
        Ok(Request::FreeGC(FreeGCRequest { gc }))
    }

    fn parse_clear_area(&self, exposures: u8, data: &[u8]) -> Result<Request, X11Error> {
        let window = Window::new(self.read_u32(&data[0..4]));
        let x = self.read_i16(&data[4..6]);
        let y = self.read_i16(&data[6..8]);
        let width = self.read_u16(&data[8..10]);
        let height = self.read_u16(&data[10..12]);

        Ok(Request::ClearArea(ClearAreaRequest {
            exposures: exposures != 0,
            window,
            x,
            y,
            width,
            height,
        }))
    }

    fn parse_poly_rectangle(&self, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));

        let mut rectangles = Vec::new();
        let mut offset = 8;
        while offset + 8 <= data.len() {
            let x = self.read_i16(&data[offset..offset + 2]);
            let y = self.read_i16(&data[offset + 2..offset + 4]);
            let width = self.read_u16(&data[offset + 4..offset + 6]);
            let height = self.read_u16(&data[offset + 6..offset + 8]);
            rectangles.push(Rectangle {
                x,
                y,
                width,
                height,
            });
            offset += 8;
        }

        Ok(Request::PolyRectangle(PolyRectangleRequest {
            drawable,
            gc,
            rectangles,
        }))
    }

    fn parse_poly_fill_rectangle(&self, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));

        let mut rectangles = Vec::new();
        let mut offset = 8;
        while offset + 8 <= data.len() {
            let x = self.read_i16(&data[offset..offset + 2]);
            let y = self.read_i16(&data[offset + 2..offset + 4]);
            let width = self.read_u16(&data[offset + 4..offset + 6]);
            let height = self.read_u16(&data[offset + 6..offset + 8]);
            rectangles.push(Rectangle {
                x,
                y,
                width,
                height,
            });
            offset += 8;
        }

        Ok(Request::PolyFillRectangle(PolyFillRectangleRequest {
            drawable,
            gc,
            rectangles,
        }))
    }

    fn parse_poly_line(&self, coordinate_mode: u8, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));

        let mut points = Vec::new();
        let mut offset = 8;
        while offset + 4 <= data.len() {
            let x = self.read_i16(&data[offset..offset + 2]);
            let y = self.read_i16(&data[offset + 2..offset + 4]);
            points.push(Point { x, y });
            offset += 4;
        }

        Ok(Request::PolyLine(PolyLineRequest {
            coordinate_mode,
            drawable,
            gc,
            points,
        }))
    }

    fn parse_poly_point(&self, coordinate_mode: u8, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));

        let mut points = Vec::new();
        let mut offset = 8;
        while offset + 4 <= data.len() {
            let x = self.read_i16(&data[offset..offset + 2]);
            let y = self.read_i16(&data[offset + 2..offset + 4]);
            points.push(Point { x, y });
            offset += 4;
        }

        Ok(Request::PolyPoint(PolyPointRequest {
            coordinate_mode,
            drawable,
            gc,
            points,
        }))
    }

    fn parse_image_text8(&self, string_len: u8, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));
        let x = self.read_i16(&data[8..10]);
        let y = self.read_i16(&data[10..12]);
        let string = String::from_utf8_lossy(&data[12..12 + string_len as usize]).to_string();

        Ok(Request::ImageText8(ImageText8Request {
            drawable,
            gc,
            x,
            y,
            string,
        }))
    }

    fn parse_create_pixmap(&self, depth: u8, data: &[u8]) -> Result<Request, X11Error> {
        let pid = Pixmap::new(self.read_u32(&data[0..4]));
        let drawable = Drawable::from_id(self.read_u32(&data[4..8]));
        let width = self.read_u16(&data[8..10]);
        let height = self.read_u16(&data[10..12]);

        Ok(Request::CreatePixmap(CreatePixmapRequest {
            depth,
            pid,
            drawable,
            width,
            height,
        }))
    }

    fn parse_free_pixmap(&self, data: &[u8]) -> Result<Request, X11Error> {
        let pixmap = Pixmap::new(self.read_u32(&data[0..4]));
        Ok(Request::FreePixmap(FreePixmapRequest { pixmap }))
    }

    fn parse_put_image(&self, format: u8, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));
        let width = self.read_u16(&data[8..10]);
        let height = self.read_u16(&data[10..12]);
        let dst_x = self.read_i16(&data[12..14]);
        let dst_y = self.read_i16(&data[14..16]);
        let left_pad = data[16];
        let depth = data[17];
        let image_data = data[20..].to_vec();

        Ok(Request::PutImage(PutImageRequest {
            format,
            drawable,
            gc,
            width,
            height,
            dst_x,
            dst_y,
            left_pad,
            depth,
            data: image_data,
        }))
    }

    fn parse_open_font(&self, data: &[u8]) -> Result<Request, X11Error> {
        let fid = self.read_u32(&data[0..4]);
        let name_len = self.read_u16(&data[4..6]) as usize;
        let name = String::from_utf8_lossy(&data[8..8 + name_len]).to_string();
        Ok(Request::OpenFont(OpenFontRequest { fid, name }))
    }

    fn parse_close_font(&self, data: &[u8]) -> Result<Request, X11Error> {
        let font = self.read_u32(&data[0..4]);
        Ok(Request::CloseFont(CloseFontRequest { font }))
    }

    fn parse_create_glyph_cursor(&self, data: &[u8]) -> Result<Request, X11Error> {
        let cid = self.read_u32(&data[0..4]);
        let source_font = self.read_u32(&data[4..8]);
        let mask_font = self.read_u32(&data[8..12]);
        Ok(Request::CreateGlyphCursor(CreateGlyphCursorRequest {
            cid,
            source_font,
            mask_font,
        }))
    }

    fn parse_alloc_named_color(&self, data: &[u8]) -> Result<Request, X11Error> {
        let colormap = self.read_u32(&data[0..4]);
        let name_len = self.read_u16(&data[4..6]) as usize;
        let name = String::from_utf8_lossy(&data[8..8 + name_len]).to_string();
        Ok(Request::AllocNamedColor(AllocNamedColorRequest {
            colormap,
            name,
        }))
    }

    fn parse_alloc_color(&self, data: &[u8]) -> Result<Request, X11Error> {
        let colormap = self.read_u32(&data[0..4]);
        let red = self.read_u16(&data[4..6]);
        let green = self.read_u16(&data[6..8]);
        let blue = self.read_u16(&data[8..10]);
        Ok(Request::AllocColor(AllocColorRequest {
            colormap,
            red,
            green,
            blue,
        }))
    }

    fn parse_query_font(&self, data: &[u8]) -> Result<Request, X11Error> {
        let font = self.read_u32(&data[0..4]);
        Ok(Request::QueryFont(QueryFontRequest { font }))
    }

    fn parse_query_extension(&self, data: &[u8]) -> Result<Request, X11Error> {
        let name_len = self.read_u16(&data[0..2]) as usize;
        let name = String::from_utf8_lossy(&data[4..4 + name_len]).to_string();
        Ok(Request::QueryExtension(QueryExtensionRequest { name }))
    }

    fn parse_set_selection_owner(&self, data: &[u8]) -> Result<Request, X11Error> {
        let owner = Window::new(self.read_u32(&data[0..4]));
        let selection = Atom::new(self.read_u32(&data[4..8]));
        let time = self.read_u32(&data[8..12]);
        Ok(Request::SetSelectionOwner(SetSelectionOwnerRequest {
            owner,
            selection,
            time,
        }))
    }

    fn parse_get_selection_owner(&self, data: &[u8]) -> Result<Request, X11Error> {
        let selection = Atom::new(self.read_u32(&data[0..4]));
        Ok(Request::GetSelectionOwner(GetSelectionOwnerRequest {
            selection,
        }))
    }

    fn parse_convert_selection(&self, data: &[u8]) -> Result<Request, X11Error> {
        let requestor = Window::new(self.read_u32(&data[0..4]));
        let selection = Atom::new(self.read_u32(&data[4..8]));
        let target = Atom::new(self.read_u32(&data[8..12]));
        let property = Atom::new(self.read_u32(&data[12..16]));
        let time = self.read_u32(&data[16..20]);
        Ok(Request::ConvertSelection(ConvertSelectionRequest {
            requestor,
            selection,
            target,
            property,
            time,
        }))
    }

    fn parse_poly_segment(&self, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));

        let mut segments = Vec::new();
        let mut offset = 8;
        while offset + 8 <= data.len() {
            let x1 = self.read_i16(&data[offset..offset + 2]);
            let y1 = self.read_i16(&data[offset + 2..offset + 4]);
            let x2 = self.read_i16(&data[offset + 4..offset + 6]);
            let y2 = self.read_i16(&data[offset + 6..offset + 8]);
            segments.push(Segment { x1, y1, x2, y2 });
            offset += 8;
        }

        Ok(Request::PolySegment(PolySegmentRequest {
            drawable,
            gc,
            segments,
        }))
    }

    fn parse_poly_arc(&self, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));

        let mut arcs = Vec::new();
        let mut offset = 8;
        while offset + 12 <= data.len() {
            let x = self.read_i16(&data[offset..offset + 2]);
            let y = self.read_i16(&data[offset + 2..offset + 4]);
            let width = self.read_u16(&data[offset + 4..offset + 6]);
            let height = self.read_u16(&data[offset + 6..offset + 8]);
            let angle1 = self.read_i16(&data[offset + 8..offset + 10]);
            let angle2 = self.read_i16(&data[offset + 10..offset + 12]);
            arcs.push(Arc {
                x,
                y,
                width,
                height,
                angle1,
                angle2,
            });
            offset += 12;
        }

        Ok(Request::PolyArc(PolyArcRequest { drawable, gc, arcs }))
    }

    fn parse_poly_fill_arc(&self, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));

        let mut arcs = Vec::new();
        let mut offset = 8;
        while offset + 12 <= data.len() {
            let x = self.read_i16(&data[offset..offset + 2]);
            let y = self.read_i16(&data[offset + 2..offset + 4]);
            let width = self.read_u16(&data[offset + 4..offset + 6]);
            let height = self.read_u16(&data[offset + 6..offset + 8]);
            let angle1 = self.read_i16(&data[offset + 8..offset + 10]);
            let angle2 = self.read_i16(&data[offset + 10..offset + 12]);
            arcs.push(Arc {
                x,
                y,
                width,
                height,
                angle1,
                angle2,
            });
            offset += 12;
        }

        Ok(Request::PolyFillArc(PolyFillArcRequest {
            drawable,
            gc,
            arcs,
        }))
    }

    fn parse_fill_poly(&self, shape: u8, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let gc = GContext::new(self.read_u32(&data[4..8]));
        let coordinate_mode = data[9]; // byte 9 is coordinate-mode

        let mut points = Vec::new();
        let mut offset = 12; // Skip 2 unused bytes after coordinate-mode
        while offset + 4 <= data.len() {
            let x = self.read_i16(&data[offset..offset + 2]);
            let y = self.read_i16(&data[offset + 2..offset + 4]);
            points.push(Point { x, y });
            offset += 4;
        }

        Ok(Request::FillPoly(FillPolyRequest {
            drawable,
            gc,
            shape,
            coordinate_mode,
            points,
        }))
    }

    fn parse_copy_area(&self, data: &[u8]) -> Result<Request, X11Error> {
        let src_drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let dst_drawable = Drawable::from_id(self.read_u32(&data[4..8]));
        let gc = GContext::new(self.read_u32(&data[8..12]));
        let src_x = self.read_i16(&data[12..14]);
        let src_y = self.read_i16(&data[14..16]);
        let dst_x = self.read_i16(&data[16..18]);
        let dst_y = self.read_i16(&data[18..20]);
        let width = self.read_u16(&data[20..22]);
        let height = self.read_u16(&data[22..24]);

        Ok(Request::CopyArea(CopyAreaRequest {
            src_drawable,
            dst_drawable,
            gc,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        }))
    }

    fn parse_get_image(&self, format: u8, data: &[u8]) -> Result<Request, X11Error> {
        let drawable = Drawable::from_id(self.read_u32(&data[0..4]));
        let x = self.read_i16(&data[4..6]);
        let y = self.read_i16(&data[6..8]);
        let width = self.read_u16(&data[8..10]);
        let height = self.read_u16(&data[10..12]);
        let plane_mask = self.read_u32(&data[12..16]);

        Ok(Request::GetImage(GetImageRequest {
            format,
            drawable,
            x,
            y,
            width,
            height,
            plane_mask,
        }))
    }
}
