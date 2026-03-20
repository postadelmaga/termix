/// Renders the alacritty_terminal grid to an ARGB8888 pixel buffer.
///
/// Pipeline:
///   1. Lock Term, iterate over all visible cells
///   2. Resolve cell foreground/background colors (Catppuccin Mocha palette)
///   3. Rasterize each glyph with fontdue into a 1-channel bitmap
///   4. Composite glyph bitmap onto the ARGB buffer at (col*cw, row*ch)
///   5. Caller uploads the buffer to wl_shm
use alacritty_terminal::{
    grid::Dimensions,
    sync::FairMutex,
    term::Term,
    vte::ansi::{Color, NamedColor},
};
use anyhow::{Context, Result};
use fontdue::{Font, FontSettings};
use std::sync::Arc;

use crate::vte::TermEventHandler;

// ── Catppuccin Mocha palette ─────────────────────────────────────────────────

const PALETTE: [[u8; 3]; 16] = [
    [30, 30, 46],    // 0  Black       (base)
    [243, 139, 168], // 1  Red         (red)
    [166, 227, 161], // 2  Green       (green)
    [249, 226, 175], // 3  Yellow      (yellow)
    [137, 180, 250], // 4  Blue        (blue)
    [245, 194, 231], // 5  Magenta     (pink)
    [148, 226, 213], // 6  Cyan        (teal)
    [205, 214, 244], // 7  White       (text)
    [88, 91, 112],   // 8  BrightBlack (surface2)
    [243, 139, 168], // 9  BrightRed
    [166, 227, 161], // 10 BrightGreen
    [249, 226, 175], // 11 BrightYellow
    [137, 180, 250], // 12 BrightBlue
    [245, 194, 231], // 13 BrightMagenta
    [148, 226, 213], // 14 BrightCyan
    [205, 214, 244], // 15 BrightWhite
];

const BG_DEFAULT: [u8; 3] = [30, 30, 46];   // base
const FG_DEFAULT: [u8; 3] = [205, 214, 244]; // text

// ── Color resolution ─────────────────────────────────────────────────────────

fn resolve_color(color: &Color, is_fg: bool) -> [u8; 3] {
    match color {
        Color::Named(named) => named_to_rgb(*named, is_fg),
        Color::Spec(rgb) => [rgb.r, rgb.g, rgb.b],
        Color::Indexed(idx) => index_to_rgb(*idx),
    }
}

fn named_to_rgb(named: NamedColor, is_fg: bool) -> [u8; 3] {
    match named {
        NamedColor::Black         => PALETTE[0],
        NamedColor::Red           => PALETTE[1],
        NamedColor::Green         => PALETTE[2],
        NamedColor::Yellow        => PALETTE[3],
        NamedColor::Blue          => PALETTE[4],
        NamedColor::Magenta       => PALETTE[5],
        NamedColor::Cyan          => PALETTE[6],
        NamedColor::White         => PALETTE[7],
        NamedColor::BrightBlack   => PALETTE[8],
        NamedColor::BrightRed     => PALETTE[9],
        NamedColor::BrightGreen   => PALETTE[10],
        NamedColor::BrightYellow  => PALETTE[11],
        NamedColor::BrightBlue    => PALETTE[12],
        NamedColor::BrightMagenta => PALETTE[13],
        NamedColor::BrightCyan    => PALETTE[14],
        NamedColor::BrightWhite   => PALETTE[15],
        NamedColor::Foreground | NamedColor::BrightForeground
        | NamedColor::DimForeground => if is_fg { FG_DEFAULT } else { BG_DEFAULT },
        NamedColor::Background    => BG_DEFAULT,
        _                         => if is_fg { FG_DEFAULT } else { BG_DEFAULT },
    }
}

fn index_to_rgb(idx: u8) -> [u8; 3] {
    if (idx as usize) < PALETTE.len() {
        return PALETTE[idx as usize];
    }
    // 216-color cube (indices 16–231)
    if idx >= 16 && idx <= 231 {
        let i = idx - 16;
        let b = (i % 6) * 51;
        let g = ((i / 6) % 6) * 51;
        let r = (i / 36) * 51;
        return [r, g, b];
    }
    // Greyscale ramp (indices 232–255)
    if idx >= 232 {
        let v = 8 + (idx - 232) * 10;
        return [v, v, v];
    }
    FG_DEFAULT
}

// ── Renderer ─────────────────────────────────────────────────────────────────

pub struct Renderer {
    font: Font,
    pub cell_width: usize,
    pub cell_height: usize,
}

impl Renderer {
    /// Load the first available monospace font from common system paths.
    pub fn new(font_size: f32) -> Result<Self> {
        let font_paths = [
            "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
            "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/liberation/LiberationMono-Regular.ttf",
        ];

        let font_data = font_paths.iter()
            .find_map(|p| std::fs::read(p).ok())
            .context("no suitable monospace font found; install noto-fonts or ttf-jetbrains-mono")?;

        let font = Font::from_bytes(font_data, FontSettings::default())
            .map_err(|e| anyhow::anyhow!("font parse error: {e}"))?;

        // Measure 'M' to derive cell dimensions
        let (metrics, _) = font.rasterize('M', font_size);
        let cell_width = metrics.advance_width.ceil() as usize;
        let cell_height = (font_size * 1.4).ceil() as usize; // line height ≈ 1.4×

        Ok(Self { font, cell_width, cell_height })
    }

    /// Render the full terminal grid into `buf` (ARGB8888, stride = width*4).
    pub fn render(
        &self,
        term: &Arc<FairMutex<Term<TermEventHandler>>>,
        buf: &mut [u8],
        surface_width: usize,
        font_size: f32,
    ) {
        let term = term.lock();
        let cols = term.columns();
        let rows = term.screen_lines();

        // Fill background
        let bg = BG_DEFAULT;
        for pixel in buf.chunks_exact_mut(4) {
            pixel[0] = bg[2]; // B
            pixel[1] = bg[1]; // G
            pixel[2] = bg[0]; // R
            pixel[3] = 255;   // A
        }

        let cw = self.cell_width;
        let ch = self.cell_height;

        for row in 0..rows {
            for col in 0..cols {
                let cell = &term.grid()[alacritty_terminal::index::Point {
                    line: alacritty_terminal::index::Line(row as i32),
                    column: alacritty_terminal::index::Column(col),
                }];

                let fg = resolve_color(&cell.fg, true);
                let bg_cell = resolve_color(&cell.bg, false);

                let px = col * cw;
                let py = row * ch;

                // Draw cell background
                for dy in 0..ch {
                    for dx in 0..cw {
                        let x = px + dx;
                        let y = py + dy;
                        if x >= surface_width { continue; }
                        let offset = (y * surface_width + x) * 4;
                        if offset + 3 >= buf.len() { continue; }
                        buf[offset]     = bg_cell[2];
                        buf[offset + 1] = bg_cell[1];
                        buf[offset + 2] = bg_cell[0];
                        buf[offset + 3] = 255;
                    }
                }

                // Skip empty/space cells
                let c = cell.c;
                if c == ' ' || c == '\0' { continue; }

                // Rasterize glyph
                let (metrics, bitmap) = self.font.rasterize(c, font_size);
                let baseline = ch.saturating_sub(ch / 5); // approximate baseline

                for (i, &alpha) in bitmap.iter().enumerate() {
                    if alpha == 0 { continue; }
                    let gx = i % metrics.width;
                    let gy = i / metrics.width;

                    let x = px + gx + metrics.xmin.max(0) as usize;
                    let y = py + baseline + gy;
                    let y = y.wrapping_sub(metrics.height.saturating_sub(metrics.ymin.max(0) as usize));

                    if x >= surface_width { continue; }
                    let offset = (y * surface_width + x) * 4;
                    if offset + 3 >= buf.len() { continue; }

                    let a = alpha as u32;
                    buf[offset]     = ((fg[2] as u32 * a + buf[offset]     as u32 * (255 - a)) / 255) as u8;
                    buf[offset + 1] = ((fg[1] as u32 * a + buf[offset + 1] as u32 * (255 - a)) / 255) as u8;
                    buf[offset + 2] = ((fg[0] as u32 * a + buf[offset + 2] as u32 * (255 - a)) / 255) as u8;
                    buf[offset + 3] = 255;
                }
            }
        }
    }
}
