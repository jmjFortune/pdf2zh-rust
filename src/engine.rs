use crate::{
    pdf, resources,
    translate::{TranslationSettings, Translator},
    types::{Bounds, Glyph, LayoutRegion, PageText},
    typeset::{self, Content, MathFragment},
};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct JobConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    pub translation: TranslationSettings,
    pub pages: String,
    pub layout: bool,
    pub min_font_size: f32,
    pub password: Option<String>,
    pub format: OutputFormat,
    pub data_dir: PathBuf,
    pub cache_path: Option<PathBuf>,
}
impl Default for JobConfig {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: PathBuf::new(),
            translation: TranslationSettings::default(),
            pages: String::new(),
            layout: true,
            min_font_size: 6.,
            password: None,
            format: OutputFormat::Mono,
            data_dir: "data".into(),
            cache_path: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, clap::ValueEnum, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Mono,
    Interleaved,
    SideBySide,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progress {
    pub stage: String,
    pub page: usize,
    pub total: usize,
    pub completed_blocks: usize,
    pub message: String,
}

pub async fn run(config: &JobConfig, mut emit: impl FnMut(Progress)) -> Result<()> {
    ensure!(config.input.is_file(), "input PDF does not exist");
    ensure!(
        config.min_font_size.is_finite() && config.min_font_size >= 4.,
        "minimum font size must be at least 4pt"
    );
    if config.output.exists() {
        ensure!(
            std::fs::canonicalize(&config.input)? != std::fs::canonicalize(&config.output)?,
            "output cannot overwrite input"
        );
        bail!("output already exists: {}", config.output.display());
    }
    std::fs::create_dir_all(&config.data_dir)?;
    let mut doc = pdf::open(&config.input, config.password.as_deref())?;
    let original = pdf::open(&config.input, config.password.as_deref())?;
    let total = doc.page_count()? as usize;
    ensure!(total > 0, "input PDF has no pages");
    let selected = page_selection(&config.pages, total)?;
    let font = resources::font_bytes()?;
    let mut translator = Translator::new(
        config.translation.clone(),
        &config
            .cache_path
            .clone()
            .unwrap_or_else(|| config.data_dir.join("cache.sqlite3")),
    )?;
    let layouts = if config.layout {
        crate::analysis::run_process(config, &mut emit).await?
    } else {
        std::collections::BTreeMap::new()
    };
    let mut completed = 0usize;
    for index in 0..total {
        if !selected.contains(&index) {
            continue;
        }
        check_cancel(&config.data_dir)?;
        emit(Progress {
            stage: "extract".into(),
            page: index + 1,
            total,
            completed_blocks: completed,
            message: String::new(),
        });
        let mut page = doc.load_pdf_page(index as i32)?;
        let rotation = page.rotation()?;
        if rotation != 0 {
            page.set_rotation(0)?;
            page = doc.reload_page(index)?;
        }
        let info = pdf::page_text(&doc, index)?;
        let regions = layouts.get(&index).map(Vec::as_slice).unwrap_or(&[]);
        if info.blocks.is_empty() {
            // A blank page can pass through. An image-only page needs OCR and
            // must not be reported as successfully translated.
            ensure!(
                page.images()?.is_empty(),
                "page {} contains images but no text layer; OCR is not yet implemented",
                index + 1
            );
        }
        let units = translation_units(&info, regions);
        for unit in units {
            check_cancel(&config.data_dir)?;
            emit(Progress {
                stage: "translate".into(),
                page: index + 1,
                total,
                completed_blocks: completed,
                message: String::new(),
            });
            let translated = translator.translate(&unit.text).await?;
            if translated.trim() == unit.text.trim() {
                continue;
            }
            check_cancel(&config.data_dir)?;
            if !unit.math.is_empty() {
                let output_font = mupdf::Font::from_bytes("translation", &font)?;
                let plan = typeset::fit(
                    &translated,
                    &unit.math,
                    unit.bounds,
                    &output_font,
                    unit.size,
                    config.min_font_size,
                )?;
                let mut forms = Vec::new();
                for formula in &unit.math {
                    forms.push(pdf::formula_form(
                        &mut doc,
                        &original,
                        index,
                        formula.bounds,
                    )?);
                }
                pdf::remove_text(&mut page, &unit.glyphs)?;
                let mut ordered: Vec<typeset::Placement> = Vec::new();
                for item in &plan.items {
                    if let Content::Text(text) = &item.content {
                        if let Some(previous) = ordered.last_mut()
                            && (previous.baseline - item.baseline).abs() < 0.01
                            && let Content::Text(existing) = &mut previous.content
                        {
                            existing.push_str(text);
                            continue;
                        }
                    }
                    ordered.push(item.clone());
                }
                let scale = plan.size / unit.size;
                for item in ordered {
                    if let Content::Text(text) = item.content {
                        pdf::write_at(
                            &mut doc,
                            &mut page,
                            mupdf::Point::new(item.x, item.baseline),
                            &text,
                            &font,
                            plan.size,
                            unit.color,
                        )?;
                    } else if let Content::Math(i) = item.content {
                        let f = &unit.math[i];
                        let bottom = item.baseline + (f.bounds.height() - f.baseline) * scale;
                        pdf::draw_form(
                            &mut doc,
                            &mut page,
                            forms[i].clone(),
                            item.x,
                            bottom,
                            scale,
                        )?;
                    }
                }
                completed += 1;
                continue;
            }
            // Fit before changing source text. If fit fails the job produces no
            // final PDF, rather than a partly translated result.
            let fitted = fit_translation(
                &mut doc,
                &mut page,
                &unit,
                &translated,
                &font,
                config.min_font_size,
            )?;
            pdf::remove_text(&mut page, &unit.glyphs)?;
            pdf::write_box(
                &mut doc,
                &mut page,
                unit.bounds,
                &translated,
                &font,
                fitted,
                fitted,
                unit.color,
            )?;
            completed += 1;
        }
        if rotation != 0 {
            page.set_rotation(rotation)?;
        }
    }
    check_cancel(&config.data_dir)?;
    emit(Progress {
        stage: "save".into(),
        page: total,
        total,
        completed_blocks: completed,
        message: String::new(),
    });
    let parent = config
        .output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}.partial.pdf", uuid::Uuid::new_v4()));
    let save_result = (|| -> Result<()> {
        match config.format {
            OutputFormat::Mono => pdf::save(&doc, &tmp)?,
            _ => pdf::dual_output(
                &config.input,
                config.password.as_deref(),
                &doc,
                &tmp,
                config.format == OutputFormat::SideBySide,
            )?,
        }
        let check = pdf::open(&tmp, None)?;
        let expected = if config.format == OutputFormat::Interleaved {
            total * 2
        } else {
            total
        };
        ensure!(
            check.page_count()? as usize == expected,
            "output page count mismatch"
        );
        drop(check);
        std::fs::rename(&tmp, &config.output)?;
        Ok(())
    })();
    if save_result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    save_result?;
    emit(Progress {
        stage: "done".into(),
        page: total,
        total,
        completed_blocks: completed,
        message: String::new(),
    });
    Ok(())
}

fn check_cancel(data: &Path) -> Result<()> {
    ensure!(!data.join("cancel").exists(), "translation cancelled");
    Ok(())
}

// Probe a Shape without committing it. Its overflow operation is atomic. This
// separates layout validation from deletion of original glyphs.
fn fit_translation(
    _doc: &mut mupdf::pdf::PdfDocument,
    page: &mut mupdf::pdf::PdfPage,
    u: &Unit,
    text: &str,
    font: &[u8],
    min: f32,
) -> Result<f32> {
    let mut shape = mupdf::shape::Shape::new(page)?;
    let mut size = u.size.max(min);
    loop {
        let opts = mupdf::shape::TextboxOptions {
            fontsize: size,
            fontname: "translation".into(),
            fontfile: Some(font),
            simple: false,
            lineheight: 1.1,
            ..Default::default()
        };
        if shape.insert_textbox(u.bounds.into(), text, &opts)? >= 0. {
            return Ok(size);
        }
        if size <= min {
            bail!(
                "page text overflows at ({:.1},{:.1}); minimum font size {}pt",
                u.bounds.x0,
                u.bounds.y0,
                min
            );
        }
        size = (size - 0.5).max(min);
    }
}

struct Unit {
    text: String,
    bounds: Bounds,
    glyphs: Vec<Glyph>,
    size: f32,
    color: u32,
    math: Vec<MathFragment>,
}

fn protected(g: &Glyph, regions: &[LayoutRegion]) -> bool {
    if g.text.is_whitespace() {
        return false;
    }
    let name = g.font.to_lowercase();
    let math_font = [
        "cmmi", "cmex", "cmsy", "symbol", "math", "msam", "msbm", "mtmi", "mtsy",
    ]
    .iter()
    .any(|s| name.contains(s));
    math_font
        || regions.iter().any(|r| {
            ["isolate_formula", "figure", "table", "abandon"].contains(&r.label.as_str())
                && r.bounds.contains_center(g.bounds)
        })
}

fn translation_units(page: &PageText, regions: &[LayoutRegion]) -> Vec<Unit> {
    let mut units = Vec::new();
    for block in &page.blocks {
        let all = block
            .lines
            .iter()
            .flat_map(|l| l.glyphs.iter())
            .collect::<Vec<_>>();
        if all.is_empty() {
            continue;
        }
        let non_text = all
            .iter()
            .filter(|g| {
                regions.iter().any(|r| {
                    ["figure", "table", "abandon", "isolate_formula"].contains(&r.label.as_str())
                        && r.bounds.contains_center(g.bounds)
                })
            })
            .count();
        if non_text * 2 > all.len() || formula_line(&block.text()) {
            continue;
        }
        {
            let mut text = String::new();
            let mut math = Vec::new();
            for line in &block.lines {
                let ranges = math_ranges(&line.glyphs, regions);
                let mut index = 0;
                for (start, end) in ranges {
                    text.extend(line.glyphs[index..start].iter().map(|g| g.text));
                    let glyphs = &line.glyphs[start..end];
                    let bounds = glyphs
                        .iter()
                        .map(|g| g.bounds)
                        .reduce(Bounds::union)
                        .unwrap();
                    let baseline = glyphs
                        .iter()
                        .max_by(|a, b| a.size.total_cmp(&b.size))
                        .unwrap()
                        .origin[1]
                        - bounds.y0;
                    text.push_str(&format!("⟦{}⟧", math.len()));
                    math.push(MathFragment { bounds, baseline });
                    index = end;
                }
                text.extend(line.glyphs[index..].iter().map(|g| g.text));
                text.push(' ');
            }
            let text = text.replace("- ", "");
            if !translatable(&text) {
                continue;
            }
            let glyphs = all.into_iter().cloned().collect::<Vec<_>>();
            let size = median_size(&glyphs);
            let mut bounds = block.bounds;
            let below = page
                .blocks
                .iter()
                .filter(|b| {
                    b.bounds.y0 >= bounds.y1 && b.bounds.x0 < bounds.x1 && b.bounds.x1 > bounds.x0
                })
                .map(|b| b.bounds.y0)
                .fold(page.bounds.y1 - 12., f32::min);
            bounds.y1 = (bounds.y1 + size * 1.5).min(below - 2.).max(bounds.y1);
            units.push(Unit {
                text,
                bounds,
                color: glyphs[0].argb,
                glyphs,
                size,
                math,
            });
        }
    }
    units
}

fn math_ranges(glyphs: &[Glyph], regions: &[LayoutRegion]) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for i in 0..glyphs.len() {
        if !protected(&glyphs[i], regions) {
            continue;
        }
        let mut start = i;
        let mut end = i + 1;
        while start > 0 && extend_math(glyphs, start - 1, -1, regions) {
            start -= 1;
        }
        while end < glyphs.len() && extend_math(glyphs, end, 1, regions) {
            end += 1;
        }
        if let Some(last) = ranges.last_mut()
            && (start <= last.1 || glyphs[last.1..start].iter().all(|g| g.text.is_whitespace()))
        {
            last.1 = last.1.max(end);
            continue;
        }
        ranges.push((start, end));
    }
    ranges
}
fn extend_math(g: &[Glyph], i: usize, direction: isize, regions: &[LayoutRegion]) -> bool {
    let c = g[i].text;
    if protected(&g[i], regions) || c.is_ascii_alphanumeric() || "=+-*/()[]{}^_<>|√∂∑∫".contains(c)
    {
        return true;
    }
    if c.is_whitespace() {
        let mut next = i as isize + direction;
        while next >= 0 && (next as usize) < g.len() && g[next as usize].text.is_whitespace() {
            next += direction;
        }
        if next < 0 || next as usize >= g.len() {
            return false;
        }
        let n = &g[next as usize];
        return protected(n, regions) || n.text.is_numeric() || "=+-*/()[]{}^_<>|".contains(n.text);
    }
    false
}
fn median_size(glyphs: &[Glyph]) -> f32 {
    let mut sizes = glyphs
        .iter()
        .map(|g| g.size)
        .filter(|x| x.is_finite() && *x > 0.)
        .collect::<Vec<_>>();
    sizes.sort_by(f32::total_cmp);
    sizes.get(sizes.len() / 2).copied().unwrap_or(10.)
}
fn translatable(s: &str) -> bool {
    s.chars().filter(|c| c.is_alphabetic()).count() >= 3
        && !s.contains("http://")
        && !s.contains("https://")
}
fn formula_line(s: &str) -> bool {
    let words = s
        .split_whitespace()
        .filter(|s| s.chars().filter(|c| c.is_alphabetic()).count() >= 3)
        .count();
    words < 3 && s.chars().any(|c| "=∑∫√∂∏≈≠≤≥".contains(c))
}

pub fn page_selection(spec: &str, total: usize) -> Result<BTreeSet<usize>> {
    if spec.trim().is_empty() {
        return Ok((0..total).collect());
    }
    let mut pages = BTreeSet::new();
    for part in spec.split(',') {
        let part = part.trim();
        let (start, end) = if let Some((a, b)) = part.split_once('-') {
            (a.parse::<usize>()?, b.parse::<usize>()?)
        } else {
            let n = part.parse::<usize>()?;
            (n, n)
        };
        ensure!(
            start >= 1 && end >= start && end <= total,
            "invalid page range {part}; document has {total} pages"
        );
        pages.extend(start - 1..end);
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn math_font_spaces_do_not_capture_adjacent_prose() {
        let text = "stack of N = 6 identical";
        let glyphs = text
            .chars()
            .enumerate()
            .map(|(i, c)| Glyph {
                text: c,
                bounds: Bounds {
                    x0: i as f32,
                    y0: 0.,
                    x1: i as f32 + 1.,
                    y1: 10.,
                },
                origin: [i as f32, 8.],
                size: 10.,
                font: if c == 'N' || c == ' ' {
                    "CMMI10"
                } else {
                    "NimbusRoman"
                }
                .into(),
                argb: 0,
            })
            .collect::<Vec<_>>();
        assert_eq!(math_ranges(&glyphs, &[]), vec![(9, 14)]);
    }
    #[test]
    fn page_ranges_validate_before_work() {
        assert_eq!(
            page_selection("1,3-4", 5).unwrap(),
            BTreeSet::from([0, 2, 3])
        );
        for bad in ["0", "3-2", "1-6", "x", ""] {
            if bad.is_empty() {
                continue;
            }
            assert!(page_selection(bad, 5).is_err());
        }
    }
    #[test]
    fn formula_detection_does_not_protect_normal_prose() {
        assert!(formula_line("E = mc2"));
        assert!(!formula_line(
            "We let x = y and use the following approach."
        ));
    }
}
