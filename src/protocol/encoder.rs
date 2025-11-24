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
        let mut buffer = vec![0u8; 32];

        // CHARINFO for min_bounds (all zeros for fixed-width font)
        let min_bounds = [0u8; 12];
        // CHARINFO for max_bounds
        let mut max_bounds = [0u8; 12];
        max_bounds[0..2].copy_from_slice(&self.write_i16(0)); // left_side_bearing
        max_bounds[2..4].copy_from_slice(&self.write_i16(0)); // right_side_bearing
        max_bounds[4..6].copy_from_slice(&self.write_i16(char_width)); // character_width
        max_bounds[6..8].copy_from_slice(&self.write_i16(font_ascent)); // ascent
        max_bounds[8..10].copy_from_slice(&self.write_i16(font_descent)); // descent
        max_bounds[10..12].copy_from_slice(&self.write_u16(0)); // attributes

        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer[4..8].copy_from_slice(&self.write_u32(7)); // Length (60 bytes / 4)
        buffer[8..20].copy_from_slice(&min_bounds);
        buffer[20..32].copy_from_slice(&max_bounds);

        // Additional 28 bytes to complete the FONTINFO structure
        buffer.extend_from_slice(&self.write_u16(0)); // min_char_or_byte2
        buffer.extend_from_slice(&self.write_u16(255)); // max_char_or_byte2
        buffer.extend_from_slice(&self.write_u16(0)); // default_char
        buffer.extend_from_slice(&self.write_u16(0)); // n_font_props
        buffer.push(0); // draw_direction (LeftToRight)
        buffer.push(0); // min_byte1
        buffer.push(0); // max_byte1
        buffer.push(1); // all_chars_exist
        buffer.extend_from_slice(&self.write_i16(font_ascent)); // font_ascent
        buffer.extend_from_slice(&self.write_i16(font_descent)); // font_descent
        buffer.extend_from_slice(&self.write_u32(0)); // n_char_infos

        buffer
    }

    /// Encode a simple success reply (no data)
    pub fn encode_void_reply(&self, sequence: u16) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];
        buffer[0] = 1; // Reply
        buffer[2..4].copy_from_slice(&self.write_u16(sequence));
        buffer
    }
}
