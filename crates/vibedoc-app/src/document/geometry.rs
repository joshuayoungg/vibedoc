//! Where the pages sit on screen, and converting between screen points and
//! layout coordinates (points inside a page's content area).

use eframe::egui::{Pos2, Rect, Vec2};
use vibedoc_core::{Layout, PageSetup};

/// Gap around and between pages, in screen points.
const PAGE_GAP: f32 = 32.0;

pub struct PageGeometry {
    pub total_rect: Rect,
    pub zoom: f32,
    left: f32,
    top: f32,
    setup: PageSetup,
}

impl PageGeometry {
    pub fn new(origin: Pos2, available_width: f32, layout: &Layout, zoom: f32) -> Self {
        let setup = layout.page_setup;
        let page_width = setup.width * zoom;
        let page_stride = setup.height * zoom + PAGE_GAP;
        let width = (page_width + 2.0 * PAGE_GAP).max(available_width);
        let height = PAGE_GAP / 2.0 + layout.page_count as f32 * page_stride;
        let total_rect = Rect::from_min_size(origin, Vec2::new(width, height));
        PageGeometry {
            total_rect,
            left: total_rect.center().x - page_width / 2.0,
            top: total_rect.top() + PAGE_GAP / 2.0,
            zoom,
            setup,
        }
    }

    fn page_stride(&self) -> f32 {
        self.setup.height * self.zoom + PAGE_GAP
    }

    pub fn page_rect(&self, page: usize) -> Rect {
        let min = Pos2::new(self.left, self.top + page as f32 * self.page_stride());
        Rect::from_min_size(min, Vec2::new(self.setup.width, self.setup.height) * self.zoom)
    }

    pub fn to_screen(&self, page: usize, x: f32, y: f32) -> Pos2 {
        let corner = self.page_rect(page).min;
        corner + Vec2::new(x + self.setup.margin, y + self.setup.margin) * self.zoom
    }

    pub fn to_layout(&self, point: Pos2) -> (usize, f32, f32) {
        let page = ((point.y - self.top) / self.page_stride()).floor().max(0.0) as usize;
        let corner = self.page_rect(page).min;
        let x = (point.x - corner.x) / self.zoom - self.setup.margin;
        let y = (point.y - corner.y) / self.zoom - self.setup.margin;
        (page, x, y)
    }
}
