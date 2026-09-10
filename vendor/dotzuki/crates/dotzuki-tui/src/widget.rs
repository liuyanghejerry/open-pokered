use dotzuki_renderer::FbSurface;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;

/// Renders any [`FbSurface`] framebuffer as half-block characters (▀) in the terminal.
///
/// Each terminal cell represents 2 vertical pixels:
/// - Top pixel → foreground color of `▀`
/// - Bottom pixel → background color of the cell
///
/// This achieves 2:1 vertical pixel density per cell row.
///
/// # Example
/// ```ignore
/// let widget = HalfblockImage {
///     fb: &framebuffer,
///     scale: 3,
///     cell_ratio: 0.8,
/// };
/// frame.render_widget(widget, area);
/// ```
pub struct HalfblockImage<'a, F: FbSurface> {
    /// The framebuffer to render.
    pub fb: &'a F,
    /// Integer scale factor: how many frame pixels map to one terminal cell width.
    pub scale: u32,
    /// Terminal cell width:height ratio (e.g., 0.5 means cells are half as wide as tall).
    /// Adjust if the image looks stretched or squashed.
    pub cell_ratio: f64,
}

impl<F: FbSurface> Widget for HalfblockImage<'_, F> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let cols_per_px = 1.0 / self.cell_ratio;
        let img_cols = (self.fb.width() as f64 * self.scale as f64 * cols_per_px).ceil() as u16;
        let img_rows = ((self.fb.height() * self.scale) / 2) as u16;

        let offset_x = (area.width.saturating_sub(img_cols)) / 2;
        let offset_y = (area.height.saturating_sub(img_rows)) / 2;

        // Clear the entire area to black (letterbox / pillarbox)
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                    cell.set_char(' ').set_fg(Color::Reset).set_bg(Color::Black);
                }
            }
        }

        let draw_cols = img_cols.min(area.width.saturating_sub(offset_x));
        let draw_rows = img_rows.min(area.height.saturating_sub(offset_y));

        let fb_width = self.fb.width();
        let fb_height = self.fb.height();

        for cy in 0..draw_rows as u32 {
            for cx in 0..draw_cols as u32 {
                // Reverse-map terminal column back to source pixel x
                let src_x = ((cx as f64 * self.cell_ratio) / self.scale as f64).floor() as u32;
                let src_x = src_x.min(fb_width - 1);
                let src_top_y = ((cy * 2) / self.scale).min(fb_height - 1);
                let src_bot_y = ((cy * 2 + 1) / self.scale).min(fb_height - 1);

                let top = self.fb.pixel_rgba(src_x, src_top_y);
                let bot = self.fb.pixel_rgba(src_x, src_bot_y);

                let px = area.x + offset_x + cx as u16;
                let py = area.y + offset_y + cy as u16;
                if let Some(cell) = buf.cell_mut((px, py)) {
                    cell.set_char('▀')
                        .set_fg(Color::Rgb(top.r, top.g, top.b))
                        .set_bg(Color::Rgb(bot.r, bot.g, bot.b));
                }
            }
        }
    }
}

/// Compute the largest integer scale that fits within `cols × rows` terminal cells,
/// accounting for the terminal cell aspect ratio.
///
/// Each terminal cell represents `scale` frame pixels horizontally and
/// `scale / 2` frame pixels vertically (because of halfblock `▀` rendering).
pub fn auto_scale(cols: u16, rows: u16, cell_ratio: f64, fb_width: u32, fb_height: u32) -> u32 {
    let cols = cols as f64;
    let rows = rows as f64;
    let w = fb_width as f64;
    // halfblock: 2 vertical pixels per cell row
    let h = fb_height as f64 / 2.0;
    let max_sx = (cols * cell_ratio) / w;
    let max_sy = rows / h;
    (max_sx.min(max_sy).floor() as u32).max(1)
}
