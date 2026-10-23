//! Select a font covering an entire paragraph before measuring or removing text.
use anyhow::{Context, Result, ensure};

struct Face {
    data: Vec<u8>,
    metrics: mupdf::Font,
}
impl Face {
    fn new(data: Vec<u8>) -> Result<Self> {
        let metrics = mupdf::Font::from_bytes("translation", &data)?;
        Ok(Self { data, metrics })
    }
    fn missing(&self, text: &str) -> Result<Option<char>> {
        for c in text.chars().filter(|c| !c.is_whitespace()) {
            if self.metrics.encode_character(c as i32)? == 0 {
                return Ok(Some(c));
            }
        }
        Ok(None)
    }
}

pub struct OutputFonts {
    primary: Face,
    fallback: Option<Face>,
}
impl OutputFonts {
    pub fn load() -> Result<Self> {
        Ok(Self {
            primary: Face::new(crate::resources::font_bytes()?)?,
            fallback: None,
        })
    }
    pub fn for_text(&mut self, text: &str) -> Result<&[u8]> {
        // Formula placeholders are drawn as original PDF forms, not font glyphs.
        let visible = crate::typeset::parse(text, text.matches('⟦').count())?
            .into_iter()
            .filter_map(|c| match c {
                crate::typeset::Content::Text(t) => Some(t),
                _ => None,
            })
            .collect::<String>();
        if self.primary.missing(&visible)?.is_none() {
            return Ok(&self.primary.data);
        }
        if self.fallback.is_none() {
            let path = crate::resources::cache().join("fonts/GoNotoKurrent-Regular.ttf");
            self.fallback =
                Some(Face::new(std::fs::read(&path).with_context(|| {
                    format!("reading fallback font {}", path.display())
                })?)?);
        }
        let fallback = self.fallback.as_ref().unwrap();
        let missing = fallback.missing(&visible)?;
        ensure!(
            missing.is_none(),
            "output fonts lack character U+{:04X}",
            missing.unwrap_or('\0') as u32
        );
        Ok(&fallback.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_bullet_uses_real_fallback_and_unsupported_text_still_errors() -> Result<()> {
        let mut fonts = OutputFonts::load()?;
        assert!(fonts.primary.missing("‣")?.is_some());
        assert!(fonts.primary.missing("中文译文")?.is_none());
        let primary = fonts.for_text("中文译文")?.len();
        let selected = fonts.for_text("‣ 中文译文 ⟦0⟧")?;
        assert_ne!(selected.len(), primary);
        let font = mupdf::Font::from_bytes("check", selected)?;
        assert_ne!(font.encode_character('‣' as i32)?, 0);
        assert!(fonts.for_text("\u{10ffff}").is_err());
        Ok(())
    }
}
