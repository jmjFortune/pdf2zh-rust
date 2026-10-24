use crate::types::Bounds;
use anyhow::{Context, Result, bail, ensure};

#[derive(Clone, Debug)]
pub struct MathFragment {
    pub bounds: Bounds,
    pub baseline: f32,
    pub glyphs: Vec<crate::types::Glyph>,
}
#[derive(Clone, Debug)]
pub enum Content {
    Text(String),
    Math(usize),
}
#[derive(Clone, Debug)]
pub struct Placement {
    pub content: Content,
    pub x: f32,
    pub baseline: f32,
}
pub struct Plan {
    pub size: f32,
    pub items: Vec<Placement>,
}

pub fn parse(text: &str, count: usize) -> Result<Vec<Content>> {
    let mut result = Vec::new();
    let mut remaining = text;
    let mut seen = vec![0; count];
    while let Some(start) = remaining.find('⟦') {
        push_text(&mut result, &remaining[..start]);
        let tail = &remaining[start + '⟦'.len_utf8()..];
        let end = tail
            .find('⟧')
            .context("translated formula marker is not closed")?;
        let index = tail[..end]
            .parse::<usize>()
            .context("translated formula marker is invalid")?;
        ensure!(index < count, "translated formula marker is unknown");
        seen[index] += 1;
        result.push(Content::Math(index));
        remaining = &tail[end + '⟧'.len_utf8()..];
    }
    push_text(&mut result, remaining);
    ensure!(
        seen.iter().all(|n| *n == 1),
        "translation lost or duplicated a formula marker"
    );
    Ok(result)
}
fn push_text(items: &mut Vec<Content>, text: &str) {
    let mut word = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || "-_/@.".contains(c) {
            word.push(c);
        } else {
            if !word.is_empty() {
                items.push(Content::Text(std::mem::take(&mut word)));
            }
            items.push(Content::Text(c.to_string()));
        }
    }
    if !word.is_empty() {
        items.push(Content::Text(word));
    }
}

pub fn fit(
    text: &str,
    math: &[MathFragment],
    bounds: Bounds,
    font: &mupdf::Font,
    original_size: f32,
    min: f32,
) -> Result<Plan> {
    let content = parse(text, math.len())?;
    let mut size = original_size.max(min);
    loop {
        if let Some(items) = layout(&content, math, bounds, font, original_size, size)? {
            return Ok(Plan { size, items });
        }
        if size <= min {
            bail!(
                "translated paragraph with formulas overflows at ({:.1},{:.1})",
                bounds.x0,
                bounds.y0
            );
        }
        size = (size - 0.5).max(min);
    }
}
fn text_width(font: &mupdf::Font, text: &str, size: f32) -> Result<f32> {
    let mut width = 0.;
    for c in text.chars() {
        let glyph = font.encode_character(c as i32)?;
        ensure!(
            glyph != 0 || c.is_whitespace(),
            "output font lacks character U+{:04X}",
            c as u32
        );
        width += font.advance_glyph(glyph)? * size;
    }
    Ok(width)
}

fn layout(
    content: &[Content],
    math: &[MathFragment],
    bounds: Bounds,
    font: &mupdf::Font,
    original_size: f32,
    size: f32,
) -> Result<Option<Vec<Placement>>> {
    let scale = size / original_size;
    let mut tokens = Vec::new();
    for c in content {
        if let Content::Text(t) = c
            && text_width(font, t, size)? > bounds.width()
        {
            tokens.extend(t.chars().map(|c| Content::Text(c.to_string())));
            continue;
        }
        tokens.push(c.clone());
    }
    let font_ascent = font.ascender().max(0.8) * size;
    let font_descent = (-font.descender()).max(0.15) * size;
    let mut result = Vec::new();
    let mut line = Vec::new();
    let mut x = bounds.x0;
    let mut top = bounds.y0;
    let mut ascent = font_ascent;
    let mut descent = font_descent;
    for token in tokens {
        let (width, a, d) = match &token {
            Content::Text(t) => (text_width(font, t, size)?, font_ascent, font_descent),
            Content::Math(i) => {
                let f = &math[*i];
                (
                    f.bounds.width() * scale,
                    f.baseline * scale,
                    (f.bounds.height() - f.baseline).max(0.) * scale,
                )
            }
        };
        if width > bounds.width() {
            return Ok(None);
        }
        if x + width > bounds.x1 && !line.is_empty() {
            if top + ascent + descent > bounds.y1 {
                return Ok(None);
            }
            for (content, px) in line.drain(..) {
                result.push(Placement {
                    content,
                    x: px,
                    baseline: top + ascent,
                });
            }
            top += ascent + descent + size * 0.12;
            x = bounds.x0;
            ascent = font_ascent;
            descent = font_descent;
        }
        if line.is_empty() && matches!(&token,Content::Text(s) if s.trim().is_empty()) {
            continue;
        }
        ascent = ascent.max(a);
        descent = descent.max(d);
        line.push((token, x));
        x += width;
    }
    if top + ascent + descent > bounds.y1 {
        return Ok(None);
    }
    for (content, x) in line {
        result.push(Placement {
            content,
            x,
            baseline: top + ascent,
        });
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formula_markers_must_be_exactly_preserved() {
        assert!(parse("译文 ⟦0⟧ 与 ⟦1⟧。", 2).is_ok());
        for bad in ["译文 ⟦0⟧。", "⟦0⟧ ⟦0⟧", "⟦5⟧ ⟦1⟧", "⟦0"] {
            assert!(parse(bad, 2).is_err(), "{bad}");
        }
    }
}
