# Visual Tests

This directory contains visual regression tests for X11Anywhere that verify rendering correctness across different backends (Windows, macOS, Linux).

## Overview

The visual test suite:
1. Connects to an X11Anywhere server using the X11 protocol
2. Renders various test patterns (colors, shapes, lines, arcs, text)
3. Captures a screenshot of the rendered output
4. Compares it against reference images to detect rendering issues

## Running Visual Tests

### Prerequisites

**Linux:**
- X server or Xvfb
- ImageMagick (`convert` command)
- xwd (usually pre-installed)

```bash
sudo apt-get install xvfb imagemagick x11-apps
```

**macOS:**
- screencapture (built-in)

**Windows:**
- No additional dependencies needed

### Running Tests Locally

1. Start the X11Anywhere server:
```bash
cargo run --release
```

2. In another terminal, run the visual test:
```bash
# Set DISPLAY if needed
export DISPLAY=:0

# Run the test
cargo test --test visual_test -- --nocapture
```

The test will:
- Connect to the X11 server
- Draw test patterns
- Capture a screenshot
- Save it as `visual_test_actual.png`
- Compare with `visual_test_reference.png` if it exists

### Generating Reference Images

To create or update reference images:

1. Run the visual test and verify the output looks correct:
```bash
cargo test --test visual_test -- --nocapture
```

2. Inspect the generated `visual_test_actual.png`

3. If it looks correct, copy it as the reference:
```bash
cp visual_test_actual.png visual_test_reference.png
```

4. Commit the reference image:
```bash
git add visual_test_reference.png
git commit -m "Update visual test reference image"
```

## Test Patterns

The visual test renders the following patterns:

### 1. Colored Rectangles
Six solid-color rectangles in the top row:
- Red (0xFF0000)
- Green (0x00FF00)
- Blue (0x0000FF)
- Yellow (0xFFFF00)
- Magenta (0xFF00FF)
- Cyan (0x00FFFF)

### 2. Lines and Shapes
Middle section with various shapes:
- Horizontal and vertical lines
- Diagonal lines (X pattern)
- Rectangle outlines
- Filled rectangles
- Filled polygons (triangles)

### 3. Arcs and Circles
Bottom left section:
- Circle outline
- Filled circle
- Quarter arc
- Ellipse

### 4. Text Rendering
Bottom right section with text:
- "X11Anywhere Visual Test"
- "Rendering: OK"
- "Colors: RGB"

## GitHub Actions Integration

Visual tests run automatically on CI for all platforms:
- Linux (using Xvfb)
- macOS (when display available)
- Windows

Screenshots are uploaded as artifacts for inspection.

See `.github/workflows/visual_test.yml` for CI configuration.

## Comparison Tolerance

The test allows for minor differences between actual and reference images:
- Per-pixel tolerance: 1% (color differences up to ~2.55 per channel)
- Total difference tolerance: 5% (up to 5% of pixels can differ)

This accounts for:
- Anti-aliasing differences
- Font rendering variations
- Minor platform-specific rendering differences

## Troubleshooting

### "Failed to capture screenshot"

**Linux:** Ensure you have `xwd` and `convert` (ImageMagick) installed:
```bash
which xwd convert
```

**macOS:** Ensure screencapture has permissions:
```bash
screencapture --help
```

**Windows:** The test uses GDI APIs which should always be available.

### "No reference image found"

This is normal on first run. Generate a reference image as described above.

### "Screenshot differs too much from reference"

This indicates a rendering regression. Compare the actual vs reference images to identify the issue:
```bash
# View side by side
open visual_test_actual.png visual_test_reference.png

# Generate diff image
compare visual_test_reference.png visual_test_actual.png diff.png
```

## Files

- `visual_test.rs` - Main test program that draws patterns and captures screenshots
- `screenshot.rs` - Platform-specific screenshot capture utilities
- `visual_test_reference.png` - Reference image (to be created)
- `visual_test_actual.png` - Generated during test runs (gitignored)
