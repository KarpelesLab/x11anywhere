//! X11 protocol events
//!
//! Events are sent from the server to clients to notify them of state changes,
//! user input, and other interesting occurrences.

use super::types::*;

/// Event type codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EventType {
    KeyPress = 2,
    KeyRelease = 3,
    ButtonPress = 4,
    ButtonRelease = 5,
    MotionNotify = 6,
    EnterNotify = 7,
    LeaveNotify = 8,
    FocusIn = 9,
    FocusOut = 10,
    KeymapNotify = 11,
    Expose = 12,
    GraphicsExposure = 13,
    NoExposure = 14,
    VisibilityNotify = 15,
    CreateNotify = 16,
    DestroyNotify = 17,
    UnmapNotify = 18,
    MapNotify = 19,
    MapRequest = 20,
    ReparentNotify = 21,
    ConfigureNotify = 22,
    ConfigureRequest = 23,
    GravityNotify = 24,
    ResizeRequest = 25,
    CirculateNotify = 26,
    CirculateRequest = 27,
    PropertyNotify = 28,
    SelectionClear = 29,
    SelectionRequest = 30,
    SelectionNotify = 31,
    ColormapNotify = 32,
    ClientMessage = 33,
    MappingNotify = 34,
}

/// Base event structure
#[derive(Debug, Clone)]
pub enum Event {
    KeyPress(KeyPressEvent),
    KeyRelease(KeyReleaseEvent),
    ButtonPress(ButtonPressEvent),
    ButtonRelease(ButtonReleaseEvent),
    MotionNotify(MotionNotifyEvent),
    EnterNotify(EnterNotifyEvent),
    LeaveNotify(LeaveNotifyEvent),
    FocusIn(FocusInEvent),
    FocusOut(FocusOutEvent),
    Expose(ExposeEvent),
    GraphicsExposure(GraphicsExposureEvent),
    NoExposure(NoExposureEvent),
    CreateNotify(CreateNotifyEvent),
    DestroyNotify(DestroyNotifyEvent),
    UnmapNotify(UnmapNotifyEvent),
    MapNotify(MapNotifyEvent),
    MapRequest(MapRequestEvent),
    ReparentNotify(ReparentNotifyEvent),
    ConfigureNotify(ConfigureNotifyEvent),
    ConfigureRequest(ConfigureRequestEvent),
    PropertyNotify(PropertyNotifyEvent),
    SelectionClear(SelectionClearEvent),
    SelectionRequest(SelectionRequestEvent),
    SelectionNotify(SelectionNotifyEvent),
    ClientMessage(ClientMessageEvent),
}

impl Event {
    /// Get the event type code
    pub fn event_type(&self) -> EventType {
        match self {
            Event::KeyPress(_) => EventType::KeyPress,
            Event::KeyRelease(_) => EventType::KeyRelease,
            Event::ButtonPress(_) => EventType::ButtonPress,
            Event::ButtonRelease(_) => EventType::ButtonRelease,
            Event::MotionNotify(_) => EventType::MotionNotify,
            Event::EnterNotify(_) => EventType::EnterNotify,
            Event::LeaveNotify(_) => EventType::LeaveNotify,
            Event::FocusIn(_) => EventType::FocusIn,
            Event::FocusOut(_) => EventType::FocusOut,
            Event::Expose(_) => EventType::Expose,
            Event::GraphicsExposure(_) => EventType::GraphicsExposure,
            Event::NoExposure(_) => EventType::NoExposure,
            Event::CreateNotify(_) => EventType::CreateNotify,
            Event::DestroyNotify(_) => EventType::DestroyNotify,
            Event::UnmapNotify(_) => EventType::UnmapNotify,
            Event::MapNotify(_) => EventType::MapNotify,
            Event::MapRequest(_) => EventType::MapRequest,
            Event::ReparentNotify(_) => EventType::ReparentNotify,
            Event::ConfigureNotify(_) => EventType::ConfigureNotify,
            Event::ConfigureRequest(_) => EventType::ConfigureRequest,
            Event::PropertyNotify(_) => EventType::PropertyNotify,
            Event::SelectionClear(_) => EventType::SelectionClear,
            Event::SelectionRequest(_) => EventType::SelectionRequest,
            Event::SelectionNotify(_) => EventType::SelectionNotify,
            Event::ClientMessage(_) => EventType::ClientMessage,
        }
    }

    /// Encode event to wire format (32 bytes)
    pub fn encode(&self, buffer: &mut [u8]) {
        assert!(buffer.len() >= 32, "Event buffer must be at least 32 bytes");
        buffer.fill(0);

        match self {
            Event::KeyPress(e) => e.encode(buffer),
            Event::KeyRelease(e) => e.encode(buffer),
            Event::ButtonPress(e) => e.encode(buffer),
            Event::ButtonRelease(e) => e.encode(buffer),
            Event::MotionNotify(e) => e.encode(buffer),
            Event::EnterNotify(e) => e.encode(buffer),
            Event::LeaveNotify(e) => e.encode(buffer),
            Event::FocusIn(e) => e.encode(buffer),
            Event::FocusOut(e) => e.encode(buffer),
            Event::Expose(e) => e.encode(buffer),
            Event::GraphicsExposure(e) => e.encode(buffer),
            Event::NoExposure(e) => e.encode(buffer),
            Event::CreateNotify(e) => e.encode(buffer),
            Event::DestroyNotify(e) => e.encode(buffer),
            Event::UnmapNotify(e) => e.encode(buffer),
            Event::MapNotify(e) => e.encode(buffer),
            Event::MapRequest(e) => e.encode(buffer),
            Event::ReparentNotify(e) => e.encode(buffer),
            Event::ConfigureNotify(e) => e.encode(buffer),
            Event::ConfigureRequest(e) => e.encode(buffer),
            Event::PropertyNotify(e) => e.encode(buffer),
            Event::SelectionClear(e) => e.encode(buffer),
            Event::SelectionRequest(e) => e.encode(buffer),
            Event::SelectionNotify(e) => e.encode(buffer),
            Event::ClientMessage(e) => e.encode(buffer),
        }
    }
}

// Key and button events share a common structure
macro_rules! define_key_button_event {
    ($name:ident, $code:expr) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            pub detail: u8,  // Keycode or button
            pub sequence: u16,
            pub time: Timestamp,
            pub root: Window,
            pub event: Window,
            pub child: Window,
            pub root_x: i16,
            pub root_y: i16,
            pub event_x: i16,
            pub event_y: i16,
            pub state: u16,  // Modifier mask
            pub same_screen: bool,
        }

        impl $name {
            pub fn encode(&self, buffer: &mut [u8]) {
                buffer[0] = $code;
                buffer[1] = self.detail;
                buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
                buffer[4..8].copy_from_slice(&self.time.get().to_ne_bytes());
                buffer[8..12].copy_from_slice(&self.root.id().get().to_ne_bytes());
                buffer[12..16].copy_from_slice(&self.event.id().get().to_ne_bytes());
                buffer[16..20].copy_from_slice(&self.child.id().get().to_ne_bytes());
                buffer[20..22].copy_from_slice(&self.root_x.to_ne_bytes());
                buffer[22..24].copy_from_slice(&self.root_y.to_ne_bytes());
                buffer[24..26].copy_from_slice(&self.event_x.to_ne_bytes());
                buffer[26..28].copy_from_slice(&self.event_y.to_ne_bytes());
                buffer[28..30].copy_from_slice(&self.state.to_ne_bytes());
                buffer[30] = if self.same_screen { 1 } else { 0 };
            }
        }
    };
}

define_key_button_event!(KeyPressEvent, 2);
define_key_button_event!(KeyReleaseEvent, 3);
define_key_button_event!(ButtonPressEvent, 4);
define_key_button_event!(ButtonReleaseEvent, 5);

#[derive(Debug, Clone)]
pub struct MotionNotifyEvent {
    pub detail: u8,  // Normal or hint
    pub sequence: u16,
    pub time: Timestamp,
    pub root: Window,
    pub event: Window,
    pub child: Window,
    pub root_x: i16,
    pub root_y: i16,
    pub event_x: i16,
    pub event_y: i16,
    pub state: u16,
    pub same_screen: bool,
}

impl MotionNotifyEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 6;
        buffer[1] = self.detail;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.time.get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.root.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&self.event.id().get().to_ne_bytes());
        buffer[16..20].copy_from_slice(&self.child.id().get().to_ne_bytes());
        buffer[20..22].copy_from_slice(&self.root_x.to_ne_bytes());
        buffer[22..24].copy_from_slice(&self.root_y.to_ne_bytes());
        buffer[24..26].copy_from_slice(&self.event_x.to_ne_bytes());
        buffer[26..28].copy_from_slice(&self.event_y.to_ne_bytes());
        buffer[28..30].copy_from_slice(&self.state.to_ne_bytes());
        buffer[30] = if self.same_screen { 1 } else { 0 };
    }
}

macro_rules! define_enter_leave_event {
    ($name:ident, $code:expr) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            pub detail: u8,
            pub sequence: u16,
            pub time: Timestamp,
            pub root: Window,
            pub event: Window,
            pub child: Window,
            pub root_x: i16,
            pub root_y: i16,
            pub event_x: i16,
            pub event_y: i16,
            pub state: u16,
            pub mode: u8,
            pub same_screen_focus: u8,
        }

        impl $name {
            pub fn encode(&self, buffer: &mut [u8]) {
                buffer[0] = $code;
                buffer[1] = self.detail;
                buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
                buffer[4..8].copy_from_slice(&self.time.get().to_ne_bytes());
                buffer[8..12].copy_from_slice(&self.root.id().get().to_ne_bytes());
                buffer[12..16].copy_from_slice(&self.event.id().get().to_ne_bytes());
                buffer[16..20].copy_from_slice(&self.child.id().get().to_ne_bytes());
                buffer[20..22].copy_from_slice(&self.root_x.to_ne_bytes());
                buffer[22..24].copy_from_slice(&self.root_y.to_ne_bytes());
                buffer[24..26].copy_from_slice(&self.event_x.to_ne_bytes());
                buffer[26..28].copy_from_slice(&self.event_y.to_ne_bytes());
                buffer[28..30].copy_from_slice(&self.state.to_ne_bytes());
                buffer[30] = self.mode;
                buffer[31] = self.same_screen_focus;
            }
        }
    };
}

define_enter_leave_event!(EnterNotifyEvent, 7);
define_enter_leave_event!(LeaveNotifyEvent, 8);

macro_rules! define_focus_event {
    ($name:ident, $code:expr) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            pub detail: u8,
            pub sequence: u16,
            pub event: Window,
            pub mode: u8,
        }

        impl $name {
            pub fn encode(&self, buffer: &mut [u8]) {
                buffer[0] = $code;
                buffer[1] = self.detail;
                buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
                buffer[4..8].copy_from_slice(&self.event.id().get().to_ne_bytes());
                buffer[8] = self.mode;
            }
        }
    };
}

define_focus_event!(FocusInEvent, 9);
define_focus_event!(FocusOutEvent, 10);

#[derive(Debug, Clone)]
pub struct ExposeEvent {
    pub sequence: u16,
    pub window: Window,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub count: u16,  // Number of following expose events
}

impl ExposeEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 12;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.window.id().get().to_ne_bytes());
        buffer[8..10].copy_from_slice(&self.x.to_ne_bytes());
        buffer[10..12].copy_from_slice(&self.y.to_ne_bytes());
        buffer[12..14].copy_from_slice(&self.width.to_ne_bytes());
        buffer[14..16].copy_from_slice(&self.height.to_ne_bytes());
        buffer[16..18].copy_from_slice(&self.count.to_ne_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct GraphicsExposureEvent {
    pub sequence: u16,
    pub drawable: Drawable,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub minor_opcode: u16,
    pub count: u16,
    pub major_opcode: u8,
}

impl GraphicsExposureEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 13;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.drawable.id().get().to_ne_bytes());
        buffer[8..10].copy_from_slice(&self.x.to_ne_bytes());
        buffer[10..12].copy_from_slice(&self.y.to_ne_bytes());
        buffer[12..14].copy_from_slice(&self.width.to_ne_bytes());
        buffer[14..16].copy_from_slice(&self.height.to_ne_bytes());
        buffer[16..18].copy_from_slice(&self.minor_opcode.to_ne_bytes());
        buffer[18..20].copy_from_slice(&self.count.to_ne_bytes());
        buffer[20] = self.major_opcode;
    }
}

#[derive(Debug, Clone)]
pub struct NoExposureEvent {
    pub sequence: u16,
    pub drawable: Drawable,
    pub minor_opcode: u16,
    pub major_opcode: u8,
}

impl NoExposureEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 14;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.drawable.id().get().to_ne_bytes());
        buffer[8..10].copy_from_slice(&self.minor_opcode.to_ne_bytes());
        buffer[10] = self.major_opcode;
    }
}

#[derive(Debug, Clone)]
pub struct CreateNotifyEvent {
    pub sequence: u16,
    pub parent: Window,
    pub window: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub override_redirect: bool,
}

impl CreateNotifyEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 16;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.parent.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.window.id().get().to_ne_bytes());
        buffer[12..14].copy_from_slice(&self.x.to_ne_bytes());
        buffer[14..16].copy_from_slice(&self.y.to_ne_bytes());
        buffer[16..18].copy_from_slice(&self.width.to_ne_bytes());
        buffer[18..20].copy_from_slice(&self.height.to_ne_bytes());
        buffer[20..22].copy_from_slice(&self.border_width.to_ne_bytes());
        buffer[22] = if self.override_redirect { 1 } else { 0 };
    }
}

#[derive(Debug, Clone)]
pub struct DestroyNotifyEvent {
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
}

impl DestroyNotifyEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 17;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.event.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.window.id().get().to_ne_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct UnmapNotifyEvent {
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub from_configure: bool,
}

impl UnmapNotifyEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 18;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.event.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.window.id().get().to_ne_bytes());
        buffer[12] = if self.from_configure { 1 } else { 0 };
    }
}

#[derive(Debug, Clone)]
pub struct MapNotifyEvent {
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub override_redirect: bool,
}

impl MapNotifyEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 19;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.event.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.window.id().get().to_ne_bytes());
        buffer[12] = if self.override_redirect { 1 } else { 0 };
    }
}

#[derive(Debug, Clone)]
pub struct MapRequestEvent {
    pub sequence: u16,
    pub parent: Window,
    pub window: Window,
}

impl MapRequestEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 20;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.parent.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.window.id().get().to_ne_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct ReparentNotifyEvent {
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub parent: Window,
    pub x: i16,
    pub y: i16,
    pub override_redirect: bool,
}

impl ReparentNotifyEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 21;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.event.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.window.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&self.parent.id().get().to_ne_bytes());
        buffer[16..18].copy_from_slice(&self.x.to_ne_bytes());
        buffer[18..20].copy_from_slice(&self.y.to_ne_bytes());
        buffer[20] = if self.override_redirect { 1 } else { 0 };
    }
}

#[derive(Debug, Clone)]
pub struct ConfigureNotifyEvent {
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub above_sibling: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub override_redirect: bool,
}

impl ConfigureNotifyEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 22;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.event.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.window.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&self.above_sibling.id().get().to_ne_bytes());
        buffer[16..18].copy_from_slice(&self.x.to_ne_bytes());
        buffer[18..20].copy_from_slice(&self.y.to_ne_bytes());
        buffer[20..22].copy_from_slice(&self.width.to_ne_bytes());
        buffer[22..24].copy_from_slice(&self.height.to_ne_bytes());
        buffer[24..26].copy_from_slice(&self.border_width.to_ne_bytes());
        buffer[26] = if self.override_redirect { 1 } else { 0 };
    }
}

#[derive(Debug, Clone)]
pub struct ConfigureRequestEvent {
    pub sequence: u16,
    pub stack_mode: u8,
    pub parent: Window,
    pub window: Window,
    pub sibling: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub value_mask: u16,
}

impl ConfigureRequestEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 23;
        buffer[1] = self.stack_mode;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.parent.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.window.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&self.sibling.id().get().to_ne_bytes());
        buffer[16..18].copy_from_slice(&self.x.to_ne_bytes());
        buffer[18..20].copy_from_slice(&self.y.to_ne_bytes());
        buffer[20..22].copy_from_slice(&self.width.to_ne_bytes());
        buffer[22..24].copy_from_slice(&self.height.to_ne_bytes());
        buffer[24..26].copy_from_slice(&self.border_width.to_ne_bytes());
        buffer[26..28].copy_from_slice(&self.value_mask.to_ne_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct PropertyNotifyEvent {
    pub sequence: u16,
    pub window: Window,
    pub atom: Atom,
    pub time: Timestamp,
    pub state: u8,  // NewValue or Deleted
}

impl PropertyNotifyEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 28;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.window.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.atom.get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&self.time.get().to_ne_bytes());
        buffer[16] = self.state;
    }
}

#[derive(Debug, Clone)]
pub struct SelectionClearEvent {
    pub sequence: u16,
    pub time: Timestamp,
    pub owner: Window,
    pub selection: Atom,
}

impl SelectionClearEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 29;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.time.get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.owner.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&self.selection.get().to_ne_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct SelectionRequestEvent {
    pub sequence: u16,
    pub time: Timestamp,
    pub owner: Window,
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub property: Atom,
}

impl SelectionRequestEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 30;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.time.get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.owner.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&self.requestor.id().get().to_ne_bytes());
        buffer[16..20].copy_from_slice(&self.selection.get().to_ne_bytes());
        buffer[20..24].copy_from_slice(&self.target.get().to_ne_bytes());
        buffer[24..28].copy_from_slice(&self.property.get().to_ne_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct SelectionNotifyEvent {
    pub sequence: u16,
    pub time: Timestamp,
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub property: Atom,
}

impl SelectionNotifyEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 31;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.time.get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.requestor.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&self.selection.get().to_ne_bytes());
        buffer[16..20].copy_from_slice(&self.target.get().to_ne_bytes());
        buffer[20..24].copy_from_slice(&self.property.get().to_ne_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct ClientMessageEvent {
    pub sequence: u16,
    pub format: u8,  // 8, 16, or 32
    pub window: Window,
    pub type_: Atom,
    pub data: ClientMessageData,
}

#[derive(Debug, Clone)]
pub enum ClientMessageData {
    Data8([u8; 20]),
    Data16([u16; 10]),
    Data32([u32; 5]),
}

impl ClientMessageEvent {
    pub fn encode(&self, buffer: &mut [u8]) {
        buffer[0] = 33;
        buffer[1] = self.format;
        buffer[2..4].copy_from_slice(&self.sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&self.window.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&self.type_.get().to_ne_bytes());

        match &self.data {
            ClientMessageData::Data8(data) => {
                buffer[12..32].copy_from_slice(data);
            }
            ClientMessageData::Data16(data) => {
                for (i, &val) in data.iter().enumerate() {
                    let offset = 12 + i * 2;
                    buffer[offset..offset + 2].copy_from_slice(&val.to_ne_bytes());
                }
            }
            ClientMessageData::Data32(data) => {
                for (i, &val) in data.iter().enumerate() {
                    let offset = 12 + i * 4;
                    buffer[offset..offset + 4].copy_from_slice(&val.to_ne_bytes());
                }
            }
        }
    }
}
