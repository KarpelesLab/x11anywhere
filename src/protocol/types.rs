//! Core X11 protocol types
//!
//! These types represent the fundamental data types used in the X11 protocol.
//! They are kept minimal and close to the wire protocol for efficiency.

use std::fmt;

/// X11 resource ID - used for windows, pixmaps, graphics contexts, etc.
/// In X11, all objects are identified by 29-bit IDs.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct XID(pub u32);

impl XID {
    pub const NONE: XID = XID(0);

    pub fn new(id: u32) -> Self {
        XID(id)
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for XID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08x}", self.0)
    }
}

/// Window ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window(pub XID);

impl Window {
    pub const NONE: Window = Window(XID::NONE);

    pub fn new(id: u32) -> Self {
        Window(XID::new(id))
    }

    pub fn id(&self) -> XID {
        self.0
    }
}

/// Pixmap ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pixmap(pub XID);

impl Pixmap {
    pub const NONE: Pixmap = Pixmap(XID::NONE);

    pub fn new(id: u32) -> Self {
        Pixmap(XID::new(id))
    }

    pub fn id(&self) -> XID {
        self.0
    }
}

/// Drawable - can be either a Window or Pixmap
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Drawable {
    Window(Window),
    Pixmap(Pixmap),
}

impl Drawable {
    pub fn id(&self) -> XID {
        match self {
            Drawable::Window(w) => w.id(),
            Drawable::Pixmap(p) => p.id(),
        }
    }

    pub fn from_id(id: u32) -> Self {
        // We'll need context to determine if it's a window or pixmap
        // For now, this is a placeholder
        Drawable::Window(Window::new(id))
    }
}

/// Graphics Context ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GContext(pub XID);

impl GContext {
    pub fn new(id: u32) -> Self {
        GContext(XID::new(id))
    }

    pub fn id(&self) -> XID {
        self.0
    }
}

/// Colormap ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Colormap(pub XID);

impl Colormap {
    pub const NONE: Colormap = Colormap(XID::NONE);

    pub fn new(id: u32) -> Self {
        Colormap(XID::new(id))
    }

    pub fn id(&self) -> XID {
        self.0
    }
}

/// Cursor ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cursor(pub XID);

impl Cursor {
    pub const NONE: Cursor = Cursor(XID::NONE);

    pub fn new(id: u32) -> Self {
        Cursor(XID::new(id))
    }

    pub fn id(&self) -> XID {
        self.0
    }
}

/// Font ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Font(pub XID);

impl Font {
    pub const NONE: Font = Font(XID::NONE);

    pub fn new(id: u32) -> Self {
        Font(XID::new(id))
    }

    pub fn id(&self) -> XID {
        self.0
    }
}

/// Atom - interned string identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Atom(pub u32);

impl Atom {
    pub const NONE: Atom = Atom(0);
    pub const PRIMARY: Atom = Atom(1);
    pub const SECONDARY: Atom = Atom(2);
    pub const ARC: Atom = Atom(3);
    pub const ATOM: Atom = Atom(4);
    pub const BITMAP: Atom = Atom(5);
    pub const CARDINAL: Atom = Atom(6);
    pub const COLORMAP: Atom = Atom(7);
    pub const CURSOR: Atom = Atom(8);
    pub const CUT_BUFFER0: Atom = Atom(9);
    pub const CUT_BUFFER1: Atom = Atom(10);
    pub const CUT_BUFFER2: Atom = Atom(11);
    pub const CUT_BUFFER3: Atom = Atom(12);
    pub const CUT_BUFFER4: Atom = Atom(13);
    pub const CUT_BUFFER5: Atom = Atom(14);
    pub const CUT_BUFFER6: Atom = Atom(15);
    pub const CUT_BUFFER7: Atom = Atom(16);
    pub const DRAWABLE: Atom = Atom(17);
    pub const FONT: Atom = Atom(18);
    pub const INTEGER: Atom = Atom(19);
    pub const PIXMAP: Atom = Atom(20);
    pub const POINT: Atom = Atom(21);
    pub const RECTANGLE: Atom = Atom(22);
    pub const RESOURCE_MANAGER: Atom = Atom(23);
    pub const RGB_COLOR_MAP: Atom = Atom(24);
    pub const RGB_BEST_MAP: Atom = Atom(25);
    pub const RGB_BLUE_MAP: Atom = Atom(26);
    pub const RGB_DEFAULT_MAP: Atom = Atom(27);
    pub const RGB_GRAY_MAP: Atom = Atom(28);
    pub const RGB_GREEN_MAP: Atom = Atom(29);
    pub const RGB_RED_MAP: Atom = Atom(30);
    pub const STRING: Atom = Atom(31);
    pub const VISUALID: Atom = Atom(32);
    pub const WINDOW: Atom = Atom(33);
    pub const WM_COMMAND: Atom = Atom(34);
    pub const WM_HINTS: Atom = Atom(35);
    pub const WM_CLIENT_MACHINE: Atom = Atom(36);
    pub const WM_ICON_NAME: Atom = Atom(37);
    pub const WM_ICON_SIZE: Atom = Atom(38);
    pub const WM_NAME: Atom = Atom(39);
    pub const WM_NORMAL_HINTS: Atom = Atom(40);
    pub const WM_SIZE_HINTS: Atom = Atom(41);
    pub const WM_ZOOM_HINTS: Atom = Atom(42);
    pub const MIN_SPACE: Atom = Atom(43);
    pub const NORM_SPACE: Atom = Atom(44);
    pub const MAX_SPACE: Atom = Atom(45);
    pub const END_SPACE: Atom = Atom(46);
    pub const SUPERSCRIPT_X: Atom = Atom(47);
    pub const SUPERSCRIPT_Y: Atom = Atom(48);
    pub const SUBSCRIPT_X: Atom = Atom(49);
    pub const SUBSCRIPT_Y: Atom = Atom(50);
    pub const UNDERLINE_POSITION: Atom = Atom(51);
    pub const UNDERLINE_THICKNESS: Atom = Atom(52);
    pub const STRIKEOUT_ASCENT: Atom = Atom(53);
    pub const STRIKEOUT_DESCENT: Atom = Atom(54);
    pub const ITALIC_ANGLE: Atom = Atom(55);
    pub const X_HEIGHT: Atom = Atom(56);
    pub const QUAD_WIDTH: Atom = Atom(57);
    pub const WEIGHT: Atom = Atom(58);
    pub const POINT_SIZE: Atom = Atom(59);
    pub const RESOLUTION: Atom = Atom(60);
    pub const COPYRIGHT: Atom = Atom(61);
    pub const NOTICE: Atom = Atom(62);
    pub const FONT_NAME: Atom = Atom(63);
    pub const FAMILY_NAME: Atom = Atom(64);
    pub const FULL_NAME: Atom = Atom(65);
    pub const CAP_HEIGHT: Atom = Atom(66);
    pub const WM_CLASS: Atom = Atom(67);
    pub const WM_TRANSIENT_FOR: Atom = Atom(68);

    /// First user-defined atom ID
    pub const FIRST_USER_ATOM: u32 = 69;

    pub fn new(id: u32) -> Self {
        Atom(id)
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}

/// Visual ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisualID(pub u32);

impl VisualID {
    pub fn new(id: u32) -> Self {
        VisualID(id)
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}

/// Timestamp (milliseconds)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub u32);

impl Timestamp {
    pub const CURRENT_TIME: Timestamp = Timestamp(0);

    pub fn new(ms: u32) -> Self {
        Timestamp(ms)
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}

/// Keycode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Keycode(pub u8);

impl Keycode {
    pub fn new(code: u8) -> Self {
        Keycode(code)
    }

    pub fn get(&self) -> u8 {
        self.0
    }
}

/// Button (mouse button)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Button(pub u8);

impl Button {
    pub const BUTTON1: Button = Button(1);
    pub const BUTTON2: Button = Button(2);
    pub const BUTTON3: Button = Button(3);
    pub const BUTTON4: Button = Button(4);
    pub const BUTTON5: Button = Button(5);

    pub fn new(button: u8) -> Self {
        Button(button)
    }

    pub fn get(&self) -> u8 {
        self.0
    }
}

/// Point (x, y coordinate)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i16,
    pub y: i16,
}

impl Point {
    pub fn new(x: i16, y: i16) -> Self {
        Point { x, y }
    }
}

/// Rectangle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Rectangle {
    pub fn new(x: i16, y: i16, width: u16, height: u16) -> Self {
        Rectangle {
            x,
            y,
            width,
            height,
        }
    }
}

/// Segment (for drawing line segments)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub x1: i16,
    pub y1: i16,
    pub x2: i16,
    pub y2: i16,
}

/// Arc (for drawing arcs and ellipses)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arc {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub angle1: i16, // Start angle in 1/64 degrees
    pub angle2: i16, // Arc angle in 1/64 degrees
}

/// Window class
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowClass {
    CopyFromParent = 0,
    InputOutput = 1,
    InputOnly = 2,
}

impl WindowClass {
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0 => Some(WindowClass::CopyFromParent),
            1 => Some(WindowClass::InputOutput),
            2 => Some(WindowClass::InputOnly),
            _ => None,
        }
    }
}

/// Backing store hint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackingStore {
    NotUseful = 0,
    WhenMapped = 1,
    Always = 2,
}

/// Map state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapState {
    Unmapped = 0,
    Unviewable = 1,
    Viewable = 2,
}

/// Stack mode for ConfigureWindow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackMode {
    Above = 0,
    Below = 1,
    TopIf = 2,
    BottomIf = 3,
    Opposite = 4,
}

/// Event masks
pub mod event_mask {
    pub const NO_EVENT: u32 = 0;
    pub const KEY_PRESS: u32 = 1 << 0;
    pub const KEY_RELEASE: u32 = 1 << 1;
    pub const BUTTON_PRESS: u32 = 1 << 2;
    pub const BUTTON_RELEASE: u32 = 1 << 3;
    pub const ENTER_WINDOW: u32 = 1 << 4;
    pub const LEAVE_WINDOW: u32 = 1 << 5;
    pub const POINTER_MOTION: u32 = 1 << 6;
    pub const POINTER_MOTION_HINT: u32 = 1 << 7;
    pub const BUTTON1_MOTION: u32 = 1 << 8;
    pub const BUTTON2_MOTION: u32 = 1 << 9;
    pub const BUTTON3_MOTION: u32 = 1 << 10;
    pub const BUTTON4_MOTION: u32 = 1 << 11;
    pub const BUTTON5_MOTION: u32 = 1 << 12;
    pub const BUTTON_MOTION: u32 = 1 << 13;
    pub const KEYMAP_STATE: u32 = 1 << 14;
    pub const EXPOSURE: u32 = 1 << 15;
    pub const VISIBILITY_CHANGE: u32 = 1 << 16;
    pub const STRUCTURE_NOTIFY: u32 = 1 << 17;
    pub const RESIZE_REDIRECT: u32 = 1 << 18;
    pub const SUBSTRUCTURE_NOTIFY: u32 = 1 << 19;
    pub const SUBSTRUCTURE_REDIRECT: u32 = 1 << 20;
    pub const FOCUS_CHANGE: u32 = 1 << 21;
    pub const PROPERTY_CHANGE: u32 = 1 << 22;
    pub const COLORMAP_CHANGE: u32 = 1 << 23;
    pub const OWNER_GRAB_BUTTON: u32 = 1 << 24;
}

/// Keyboard/pointer modifier masks
pub mod modifier_mask {
    pub const SHIFT: u16 = 1 << 0;
    pub const LOCK: u16 = 1 << 1;
    pub const CONTROL: u16 = 1 << 2;
    pub const MOD1: u16 = 1 << 3;
    pub const MOD2: u16 = 1 << 4;
    pub const MOD3: u16 = 1 << 5;
    pub const MOD4: u16 = 1 << 6;
    pub const MOD5: u16 = 1 << 7;
    pub const BUTTON1: u16 = 1 << 8;
    pub const BUTTON2: u16 = 1 << 9;
    pub const BUTTON3: u16 = 1 << 10;
    pub const BUTTON4: u16 = 1 << 11;
    pub const BUTTON5: u16 = 1 << 12;
}

/// GC function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GCFunction {
    Clear = 0,
    And = 1,
    AndReverse = 2,
    Copy = 3,
    AndInverted = 4,
    NoOp = 5,
    Xor = 6,
    Or = 7,
    Nor = 8,
    Equiv = 9,
    Invert = 10,
    OrReverse = 11,
    CopyInverted = 12,
    OrInverted = 13,
    Nand = 14,
    Set = 15,
}

/// Line style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid = 0,
    OnOffDash = 1,
    DoubleDash = 2,
}

/// Cap style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapStyle {
    NotLast = 0,
    Butt = 1,
    Round = 2,
    Projecting = 3,
}

/// Join style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinStyle {
    Miter = 0,
    Round = 1,
    Bevel = 2,
}

/// Fill style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillStyle {
    Solid = 0,
    Tiled = 1,
    Stippled = 2,
    OpaqueStippled = 3,
}

/// Fill rule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    EvenOdd = 0,
    Winding = 1,
}

/// Arc mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcMode {
    Chord = 0,
    PieSlice = 1,
}

/// Image format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Bitmap = 0,
    XYPixmap = 1,
    ZPixmap = 2,
}

/// Byte order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    LSBFirst = 0,
    MSBFirst = 1,
}

impl ByteOrder {
    pub fn native() -> Self {
        if cfg!(target_endian = "little") {
            ByteOrder::LSBFirst
        } else {
            ByteOrder::MSBFirst
        }
    }
}
