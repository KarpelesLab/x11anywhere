# X11 Protocol Analysis: xcalc Session

## Capture Overview
- **File**: `/tmp/xcalc_protocol_capture.log`
- **Size**: 3.1 MB (27,214 lines)
- **Application**: xcalc (X11 calculator)
- **Purpose**: Understanding real-world X11 protocol usage for implementation

## Initial Handshake Sequence

### Request #0: Query BIG-REQUESTS Extension
```
Opcode: 0x62 (98) = QueryExtension
Length: 20 bytes
Data: "BIG-REQUESTS"
```
**Server Response**: Extension available at major opcode 0x85 (133)

### Request #1: Enable BIG-REQUESTS
```
Opcode: 0x85 (133) = Extension request
Length: 4 bytes
```
**Server Response**: Maximum request length 0x3FFFFF (4,194,303 bytes)

### Request #2: Create Graphics Context
```
Opcode: 0x37 (55) = CreateGC
Length: 44 bytes
Window: 0x05200000 (root window from setup)
```
**Purpose**: Create GC for drawing operations

### Request #3-4: Query XKEYBOARD Extension
```
Opcode: 0x62 (98) = QueryExtension for "XKEYBOARD"
Opcode: 0x87 (135) = XKEYBOARD extension request
```
**Purpose**: Initialize keyboard input handling

### Request #5-7: Intern Atoms
```
Opcode: 0x10 (16) = InternAtom
Atoms requested:
  - "Custom Init" → atom 0x027C
  - "Custom Data" → atom 0x027D
  - "SCREEN_RESOURCES" → atom 0 (doesn't exist)
```
**Purpose**: Register atom names for properties and messages

### Request #8: Query RENDER Extension
```
Opcode: 0x62 (98) = QueryExtension
Combined with: CreatePixmap (0x35/53)
Extension: "ESRENDER"
```
**Server Response**: Major opcode 0x8A (138), first event 0x8C (140)

## Common Request Types Observed

### Core Protocol Requests
- **0x10 (16)**: InternAtom - Register/lookup atom names
- **0x35 (53)**: CreatePixmap - Allocate offscreen drawable
- **0x37 (55)**: CreateGC - Create graphics context
- **0x62 (98)**: QueryExtension - Check for X extensions

### Extension Requests (Opcodes 128+)
- **0x85 (133)**: BIG-REQUESTS extension
- **0x87 (135)**: XKEYBOARD extension
- **0x8A (138)**: RENDER extension

## Request Structure Pattern

All X11 requests follow this format:
```
Byte 0:    Opcode (0-127 core, 128+ extensions)
Byte 1:    Request-specific data or padding
Bytes 2-3: Length in 4-byte units (little-endian)
Bytes 4+:  Request-specific data
```

### Example: InternAtom Request (#5)
```
Hex: 10 00 05 00 0b 00 00 00 43 75 73 74 6f 6d 20 49 6e 69 74 00

Fields:
  10          - Opcode 16 (InternAtom)
  00          - only-if-exists flag (false)
  05 00       - Length: 5 * 4 = 20 bytes
  0b 00       - Name length: 11 bytes
  00 00       - Padding
  "Custom Init" - Atom name (11 bytes)
  00          - Padding to 4-byte boundary
```

## Response Structure Pattern

All replies follow this format:
```
Byte 0:    Reply type (1 = Reply, 0 = Error)
Byte 1:    Reply-specific data
Bytes 2-3: Sequence number (little-endian)
Bytes 4-7: Additional data length in 4-byte units
Bytes 8+:  Reply-specific data
```

### Example: InternAtom Reply
```
Hex: 01 00 07 00 00 00 00 00 7c 02 00 00 ...

Fields:
  01          - Reply type
  00          - Unused
  07 00       - Sequence number 7
  00 00 00 00 - No additional data
  7c 02 00 00 - Atom value: 0x027C (636)
```

## Key Observations

### 1. Extension Usage
xcalc heavily relies on extensions:
- **BIG-REQUESTS**: Enables requests larger than 256KB
- **XKEYBOARD**: Advanced keyboard input handling
- **RENDER**: Modern anti-aliased rendering

### 2. Atom Management
Applications intern atoms for:
- Property names (window manager hints)
- Message types (client messages)
- Selection types (clipboard operations)

### 3. Resource Allocation
Initial phase creates resources:
- Graphics contexts for drawing
- Pixmaps for double-buffering
- Fonts for text rendering

## Next Steps for Implementation

### Priority 1: Core Requests
1. **InternAtom (16)** - Essential for property handling
2. **CreateWindow (1)** - Window creation
3. **MapWindow (8)** - Make windows visible
4. **CreateGC (55)** - Graphics contexts
5. **CreatePixmap (53)** - Offscreen drawing

### Priority 2: Extension Support
1. **QueryExtension (98)** - Extension discovery
2. **ListExtensions (99)** - Available extensions
3. Extension-specific requests (138+)

### Priority 3: Properties & Events
1. **ChangeProperty (18)** - Set window properties
2. **GetProperty (20)** - Read properties
3. Event handling for user input

## Testing Strategy

1. **Unit Tests**: Test each opcode parser individually
2. **Integration Tests**: Use captured xcalc traffic as test data
3. **Compliance**: Verify against X11 protocol specification
4. **Real-world**: Test with actual X11 clients

## Reference Materials

- **X11 Protocol Spec**: https://www.x.org/releases/current/doc/xproto/x11protocol.html
- **Xlib Manual**: Reference for client-side behavior
- **Extension Specs**: Individual RFCs for RENDER, XKB, etc.
- **This Capture**: Real-world protocol examples from xcalc

## Capture File Location

Full protocol capture with all hex dumps:
```
/tmp/xcalc_protocol_capture.log
```

Use this file to:
- Extract example packets for unit tests
- Understand request/response sequences
- Debug protocol encoding/decoding
- Validate implementation correctness
