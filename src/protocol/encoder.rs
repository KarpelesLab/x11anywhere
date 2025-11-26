//! X11 reply and event encoder
//!
//! This module encodes replies and events to the wire protocol.

use super::*;

/// Reply encoder
pub struct ProtocolEncoder {
    byte_order: ByteOrder,
}

impl ProtocolEncoder {
    pub fn new(byte_order: ByteOrder) -> Self {
        ProtocolEncoder { byte_order }
    }

    // Helper methods for writing with correct byte order
    fn write_u16(&self, value: u16) -> [u8; 2] {
        match self.byte_order {
            ByteOrder::MSBFirst => value.to_be_bytes(),
            ByteOrder::LSBFirst => value.to_le_bytes(),
        }
    }

    fn write_u32(&self, value: u32) -> [u8; 4] {
        match self.byte_order {
            ByteOrder::MSBFirst => value.to_be_bytes(),
            ByteOrder::LSBFirst => value.to_le_bytes(),
        }
    }

    fn write_i16(&self, value: i16) -> [u8; 2] {
        match self.byte_order {
            ByteOrder::MSBFirst => value.to_be_bytes(),
            ByteOrder::LSBFirst => value.to_le_bytes(),
        }
    }

    /// Encode GetWindowAttributes reply
    #[allow(clippy::too_many_arguments)]
    pub fn encode_get_window_attributes_reply(
        &self,
        sequence: u16,
        visual: VisualID,
        class: WindowClass,
        bit_gravity: u8,
        backing_store: BackingStore,
        backing_planes: u32,
        backing_pixel: u32,
        save_under: bool,
        map_is_installed: bool,
        map_state: MapState,
        override_redirect: bool,
        colormap: Colormap,
        all_event_masks: u32,
        your_event_mask: u32,
        do_not_propagate_mask: u16,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[1] = backing_store as u8;
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(3)); // Length (additional data / 4)
        buffer[8..12].copy_from_slice(&self.write_u32(visual.get()));
        buffer[12..14].copy_from_slice(&self.write_u16(class as u16));
        buffer[14] = bit_gravity;
        buffer[15] = backing_store as u8;
        buffer[16..20].copy_from_slice(&self.write_u32(backing_planes));
        buffer[20..24].copy_from_slice(&self.write_u32(backing_pixel));
        buffer[24] = if save_under { 1 } else { 0 };
        buffer[25] = if map_is_installed { 1 } else { 0 };
        buffer[26] = map_state as u8;
        buffer[27] = if override_redirect { 1 } else { 0 };
        buffer[28..32].copy_from_slice(&self.write_u32(colormap.id().get()));

        // Additional data
        buffer.extend_from_slice(&self.write_u32(all_event_masks));
        buffer.extend_from_slice(&self.write_u32(your_event_mask));
        buffer.extend_from_slice(&self.write_u16(do_not_propagate_mask));
        buffer.extend_from_slice(&[0u8; 2]); // Padding

        buffer
    }

    /// Encode GetGeometry reply
    #[allow(clippy::too_many_arguments)]
    pub fn encode_get_geometry_reply(
        &self,
        sequence: u16,
        depth: u8,
        root: Window,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[1] = depth;
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data
        buffer[8..12].copy_from_slice(&self.write_u32(root.id().get()));
        buffer[12..14].copy_from_slice(&self.write_i16(x));
        buffer[14..16].copy_from_slice(&self.write_i16(y));
        buffer[16..18].copy_from_slice(&self.write_u16(width));
        buffer[18..20].copy_from_slice(&self.write_u16(height));
        buffer[20..22].copy_from_slice(&self.write_u16(border_width));

        buffer
    }

    /// Encode QueryTree reply
    pub fn encode_query_tree_reply(
        &self,
        sequence: u16,
        root: Window,
        parent: Window,
        children: &[Window],
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(children.len() as u32));
        buffer[8..12].copy_from_slice(&self.write_u32(root.id().get()));
        buffer[12..16].copy_from_slice(&self.write_u32(parent.id().get()));
        buffer[16..18].copy_from_slice(&self.write_u16(children.len() as u16));

        // Append children
        for child in children {
            buffer.extend_from_slice(&self.write_u32(child.id().get()));
        }

        // Pad to 4-byte boundary
        while !buffer.len().is_multiple_of(4) {
            buffer.push(0);
        }

        buffer
    }

    /// Encode InternAtom reply
    pub fn encode_intern_atom_reply(&self, sequence: u16, atom: Atom) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data
        buffer[8..12].copy_from_slice(&self.write_u32(atom.get()));

        buffer
    }

    /// Encode GetAtomName reply
    pub fn encode_get_atom_name_reply(&self, sequence: u16, name: &str) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        let name_len = name.len();
        let name_padded = padded_len(name_len);

        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32((name_padded / 4) as u32));
        buffer[8..10].copy_from_slice(&self.write_u16(name_len as u16));

        // Append name
        buffer.extend_from_slice(name.as_bytes());

        // Pad to 4-byte boundary
        while !buffer.len().is_multiple_of(4) {
            buffer.push(0);
        }

        buffer
    }

    /// Encode GetProperty reply
    pub fn encode_get_property_reply(
        &self,
        sequence: u16,
        format: u8,
        type_: Atom,
        bytes_after: u32,
        value: &[u8],
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        let value_padded = padded_len(value.len());
        let value_len = match format {
            8 => value.len() as u32,
            16 => (value.len() / 2) as u32,
            32 => (value.len() / 4) as u32,
            _ => 0,
        };

        buffer[0] = 1; // Reply
        buffer[1] = format;
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32((value_padded / 4) as u32));
        buffer[8..12].copy_from_slice(&self.write_u32(type_.get()));
        buffer[12..16].copy_from_slice(&self.write_u32(bytes_after));
        buffer[16..20].copy_from_slice(&self.write_u32(value_len));

        // Append value
        buffer.extend_from_slice(value);

        // Pad to 4-byte boundary
        while !buffer.len().is_multiple_of(4) {
            buffer.push(0);
        }

        buffer
    }

    /// Encode QueryExtension reply
    pub fn encode_query_extension_reply(
        &self,
        sequence: u16,
        present: bool,
        major_opcode: u8,
        first_event: u8,
        first_error: u8,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data
        buffer[8] = if present { 1 } else { 0 };
        buffer[9] = major_opcode;
        buffer[10] = first_event;
        buffer[11] = first_error;

        buffer
    }

    /// Encode GetInputFocus reply
    pub fn encode_get_input_focus_reply(
        &self,
        sequence: u16,
        focus: Window,
        revert_to: u8,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[1] = revert_to;
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data
        buffer[8..12].copy_from_slice(&self.write_u32(focus.id().0));

        buffer
    }

    /// Encode AllocColor reply
    pub fn encode_alloc_color_reply(
        &self,
        sequence: u16,
        pixel: u32,
        red: u16,
        green: u16,
        blue: u16,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data
        buffer[8..10].copy_from_slice(&self.write_u16(red));
        buffer[10..12].copy_from_slice(&self.write_u16(green));
        buffer[12..14].copy_from_slice(&self.write_u16(blue));
        // bytes 14-15 unused
        buffer[16..20].copy_from_slice(&self.write_u32(pixel));

        buffer
    }

    /// Encode AllocNamedColor reply
    #[allow(clippy::too_many_arguments)]
    pub fn encode_alloc_named_color_reply(
        &self,
        sequence: u16,
        pixel: u32,
        exact_red: u16,
        exact_green: u16,
        exact_blue: u16,
        visual_red: u16,
        visual_green: u16,
        visual_blue: u16,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data
        buffer[8..12].copy_from_slice(&self.write_u32(pixel));
        buffer[12..14].copy_from_slice(&self.write_u16(exact_red));
        buffer[14..16].copy_from_slice(&self.write_u16(exact_green));
        buffer[16..18].copy_from_slice(&self.write_u16(exact_blue));
        buffer[18..20].copy_from_slice(&self.write_u16(visual_red));
        buffer[20..22].copy_from_slice(&self.write_u16(visual_green));
        buffer[22..24].copy_from_slice(&self.write_u16(visual_blue));

        buffer
    }

    /// Encode QueryFont reply
    pub fn encode_query_font_reply(
        &self,
        sequence: u16,
        font_ascent: i16,
        font_descent: i16,
        char_width: i16,
        _char_height: i16,
    ) -> Vec<u8> {
        // X11 QueryFont reply structure (60 bytes for n=0 properties, m=0 char infos):
        // bytes 0-7:   reply header (type, unused, sequence, length)
        // bytes 8-19:  min_bounds CHARINFO (12 bytes)
        // bytes 20-23: unused (4 bytes)
        // bytes 24-35: max_bounds CHARINFO (12 bytes)
        // bytes 36-39: unused (4 bytes)
        // bytes 40-59: font info (20 bytes)
        // reply_length = (60 - 32) / 4 = 7
        let mut buffer = vec![0u8; 60];

        // CHARINFO for min_bounds (all zeros for simplicity)
        let min_bounds = [0u8; 12];

        // CHARINFO for max_bounds
        let mut max_bounds = [0u8; 12];
        max_bounds[0..2].copy_from_slice(&self.write_i16(0)); // left_side_bearing
        max_bounds[2..4].copy_from_slice(&self.write_i16(char_width)); // right_side_bearing
        max_bounds[4..6].copy_from_slice(&self.write_i16(char_width)); // character_width
        max_bounds[6..8].copy_from_slice(&self.write_i16(font_ascent)); // ascent
        max_bounds[8..10].copy_from_slice(&self.write_i16(font_descent)); // descent
        max_bounds[10..12].copy_from_slice(&self.write_u16(0)); // attributes

        // Reply header
        buffer[0] = 1; // Reply
        // buffer[1] = unused (already 0)
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(7)); // reply_length = 7 for n=0, m=0

        // min_bounds at bytes 8-19
        buffer[8..20].copy_from_slice(&min_bounds);
        // bytes 20-23 unused (already 0)

        // max_bounds at bytes 24-35
        buffer[24..36].copy_from_slice(&max_bounds);
        // bytes 36-39 unused (already 0)

        // Font info at bytes 40-59
        buffer[40..42].copy_from_slice(&self.write_u16(0)); // min_char_or_byte2
        buffer[42..44].copy_from_slice(&self.write_u16(255)); // max_char_or_byte2
        buffer[44..46].copy_from_slice(&self.write_u16(0)); // default_char
        buffer[46..48].copy_from_slice(&self.write_u16(0)); // n_font_props
        buffer[48] = 0; // draw_direction (LeftToRight)
        buffer[49] = 0; // min_byte1
        buffer[50] = 0; // max_byte1
        buffer[51] = 1; // all_chars_exist
        buffer[52..54].copy_from_slice(&self.write_i16(font_ascent)); // font_ascent
        buffer[54..56].copy_from_slice(&self.write_i16(font_descent)); // font_descent
        buffer[56..60].copy_from_slice(&self.write_u32(0)); // n_char_infos (m)

        buffer
    }

    /// Encode a simple success reply (no data)
    pub fn encode_void_reply(&self, sequence: u16) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];
        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer
    }

    /// Encode ListFonts reply
    pub fn encode_list_fonts_reply(&self, sequence: u16, font_names: &[String]) -> Vec<u8> {
        // Build the LISTofSTR data (each is: 1 byte length + string bytes)
        let mut str_data = Vec::new();
        for name in font_names {
            let name_bytes = name.as_bytes();
            let len = name_bytes.len().min(255) as u8;
            str_data.push(len);
            str_data.extend_from_slice(&name_bytes[..len as usize]);
        }

        // Pad to 4-byte boundary
        let padding = (4 - (str_data.len() % 4)) % 4;
        str_data.extend(vec![0u8; padding]);

        // Reply length in 4-byte units (not including the header 32 bytes)
        let reply_length = str_data.len() / 4;

        let mut buffer = vec![0u8; 32];
        buffer[0] = 1; // Reply
        buffer[1] = 0; // unused
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(reply_length as u32));
        buffer[8..10].copy_from_slice(&self.write_u16(font_names.len() as u16)); // number of STRs
                                                                                 // bytes 10-31 are unused (22 bytes)

        // Append the string data
        buffer.extend_from_slice(&str_data);

        buffer
    }

    /// Encode ListExtensions reply
    pub fn encode_list_extensions_reply(
        &self,
        sequence: u16,
        extension_names: &[String],
    ) -> Vec<u8> {
        // Build the LISTofSTR data (each is: 1 byte length + string bytes)
        let mut str_data = Vec::new();
        for name in extension_names {
            let name_bytes = name.as_bytes();
            let len = name_bytes.len().min(255) as u8;
            str_data.push(len);
            str_data.extend_from_slice(&name_bytes[..len as usize]);
        }

        // Pad to 4-byte boundary
        let padding = (4 - (str_data.len() % 4)) % 4;
        str_data.extend(vec![0u8; padding]);

        // Reply length in 4-byte units (not including the header 32 bytes)
        let reply_length = str_data.len() / 4;

        let mut buffer = vec![0u8; 32];
        buffer[0] = 1; // Reply
        buffer[1] = extension_names.len() as u8; // number of STRs in data byte
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(reply_length as u32));
        // bytes 8-31 are unused (24 bytes)

        // Append the string data
        buffer.extend_from_slice(&str_data);

        buffer
    }

    /// Encode QueryPointer reply
    #[allow(clippy::too_many_arguments)]
    pub fn encode_query_pointer_reply(
        &self,
        sequence: u16,
        same_screen: bool,
        root: Window,
        child: Window,
        root_x: i16,
        root_y: i16,
        win_x: i16,
        win_y: i16,
        mask: u16,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[1] = if same_screen { 1 } else { 0 };
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data
        buffer[8..12].copy_from_slice(&self.write_u32(root.id().0));
        buffer[12..16].copy_from_slice(&self.write_u32(child.id().0));
        buffer[16..18].copy_from_slice(&self.write_i16(root_x));
        buffer[18..20].copy_from_slice(&self.write_i16(root_y));
        buffer[20..22].copy_from_slice(&self.write_i16(win_x));
        buffer[22..24].copy_from_slice(&self.write_i16(win_y));
        buffer[24..26].copy_from_slice(&self.write_u16(mask));

        buffer
    }

    /// Encode TranslateCoordinates reply
    pub fn encode_translate_coordinates_reply(
        &self,
        sequence: u16,
        same_screen: bool,
        child: Window,
        dst_x: i16,
        dst_y: i16,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[1] = if same_screen { 1 } else { 0 };
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data
        buffer[8..12].copy_from_slice(&self.write_u32(child.id().0));
        buffer[12..14].copy_from_slice(&self.write_i16(dst_x));
        buffer[14..16].copy_from_slice(&self.write_i16(dst_y));

        buffer
    }

    /// Encode QueryKeymap reply
    pub fn encode_query_keymap_reply(&self, sequence: u16, keys: &[u8; 32]) -> Vec<u8> {
        let mut buffer = vec![0u8; 40]; // 32 header + 8 more for total 40 bytes (reply length = 2)

        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(2)); // Length in 4-byte units (8 bytes / 4)
        buffer[8..40].copy_from_slice(keys);

        buffer
    }

    /// Encode GrabPointer reply
    /// status: 0=Success, 1=AlreadyGrabbed, 2=InvalidTime, 3=NotViewable, 4=Frozen
    pub fn encode_grab_pointer_reply(&self, sequence: u16, status: u8) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[1] = status;
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data

        buffer
    }

    /// Encode GrabKeyboard reply
    /// status: 0=Success, 1=AlreadyGrabbed, 2=InvalidTime, 3=NotViewable, 4=Frozen
    pub fn encode_grab_keyboard_reply(&self, sequence: u16, status: u8) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[1] = status;
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data

        buffer
    }

    /// Encode GetScreenSaver reply
    pub fn encode_get_screen_saver_reply(
        &self,
        sequence: u16,
        timeout: u16,
        interval: u16,
        prefer_blanking: u8,
        allow_exposures: u8,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(0)); // No additional data
        buffer[8..10].copy_from_slice(&self.write_u16(timeout));
        buffer[10..12].copy_from_slice(&self.write_u16(interval));
        buffer[12] = prefer_blanking;
        buffer[13] = allow_exposures;

        buffer
    }

    // ========== Event Encoders ==========

    /// Encode Expose event (event code 12)
    /// Sent when a window region needs to be redrawn
    #[allow(clippy::too_many_arguments)]
    pub fn encode_expose_event(
        &self,
        sequence: u16,
        window: Window,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        count: u16,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 12; // Expose event code
                        // buffer[1] unused
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(window.id().get()));
        buffer[8..10].copy_from_slice(&self.write_u16(x));
        buffer[10..12].copy_from_slice(&self.write_u16(y));
        buffer[12..14].copy_from_slice(&self.write_u16(width));
        buffer[14..16].copy_from_slice(&self.write_u16(height));
        buffer[16..18].copy_from_slice(&self.write_u16(count));
        // bytes 18-31 unused

        buffer
    }

    /// Encode MapNotify event (event code 19)
    /// Sent when a window is mapped
    pub fn encode_map_notify_event(
        &self,
        sequence: u16,
        event: Window,
        window: Window,
        override_redirect: bool,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];

        buffer[0] = 19; // MapNotify event code
                        // buffer[1] unused
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(event.id().get()));
        buffer[8..12].copy_from_slice(&self.write_u32(window.id().get()));
        buffer[12] = if override_redirect { 1 } else { 0 };
        // bytes 13-31 unused

        buffer
    }
}
