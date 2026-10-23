use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Bounds {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Bounds {
    pub fn width(self) -> f32 {
        self.x1 - self.x0
    }
    pub fn height(self) -> f32 {
        self.y1 - self.y0
    }
    pub fn contains_center(self, other: Self) -> bool {
        let x = (other.x0 + other.x1) * 0.5;
        let y = (other.y0 + other.y1) * 0.5;
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }
    pub fn union(self, rhs: Self) -> Self {
        Self {
            x0: self.x0.min(rhs.x0),
            y0: self.y0.min(rhs.y0),
            x1: self.x1.max(rhs.x1),
            y1: self.y1.max(rhs.y1),
        }
    }
    pub fn intersects(self, rhs: Self) -> bool {
        self.x0 < rhs.x1 && rhs.x0 < self.x1 && self.y0 < rhs.y1 && rhs.y0 < self.y1
    }
}

impl From<mupdf::Rect> for Bounds {
    fn from(r: mupdf::Rect) -> Self {
        Self {
            x0: r.x0,
            y0: r.y0,
            x1: r.x1,
            y1: r.y1,
        }
    }
}
impl From<Bounds> for mupdf::Rect {
    fn from(r: Bounds) -> Self {
        Self::new(r.x0, r.y0, r.x1, r.y1)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Glyph {
    pub text: char,
    pub bounds: Bounds,
    pub origin: [f32; 2],
    pub size: f32,
    pub font: String,
    pub argb: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextLine {
    pub bounds: Bounds,
    pub glyphs: Vec<Glyph>,
}
impl TextLine {
    pub fn text(&self) -> String {
        self.glyphs.iter().map(|g| g.text).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextBlock {
    pub bounds: Bounds,
    pub lines: Vec<TextLine>,
}
impl TextBlock {
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(TextLine::text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageText {
    pub index: usize,
    pub bounds: Bounds,
    pub rotation: i32,
    pub blocks: Vec<TextBlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutRegion {
    pub bounds: Bounds,
    pub class_id: usize,
    pub label: String,
    pub confidence: f32,
}
