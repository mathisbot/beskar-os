# ascii-ui

ASCII UI toolkit.
Provides character-grid abstraction, theme support, and common UI primitives for retro-styled terminal interfaces.

## Usage

### Basic Canvas Setup

```rust
use ascii_ui::{AsciiCanvas, Theme, CharRect, BoxStyle};
use beskar_core::video::PixelComponents;

let theme = Theme::white_on_black();
let mut canvas = AsciiCanvas::new(screen_info, pixel_buffer, theme);

// Clear with background color
canvas.clear_with_theme();

// Write centered text at row 1
canvas.write_line_centered(1, "[ TITLE ]");

// Draw a panel
let panel = CharRect::new(2, 3, 40, 20);
canvas.draw_panel(panel);

// Write text inside the panel
canvas.write_line(panel.x + 1, panel.y + 1, "Content here");
```

### Theming

```rust
use ascii_ui::Theme;

// Or create a custom theme
let custom = Theme::new(
    PixelComponents::WHITE,
    PixelComponents::BLACK
);

// Swap colors
let inverted = theme.inverse();

canvas.set_theme(custom);
```

### Advanced Layout

```rust
// Draw horizontal rule
canvas.h_rule(5, '=');

// Draw vertical rule
canvas.v_rule(10, 5..15, '|');

// Fill a rectangle with a character
let rect = CharRect::new(2, 2, 20, 10);
canvas.fill_rect(rect, ' ');

// Inset a rectangle
let inner = rect.inset(1);

// Format text to fit
let formatted = canvas.format_line("Long text here");
```

### Text Formatting

```rust
use ascii_ui::TextFormatter;
use core::fmt::Write;

let mut formatter = TextFormatter::new();
write!(formatter, "Status: {}", value).unwrap();
canvas.write_line(0, 0, formatter.as_str());
```
