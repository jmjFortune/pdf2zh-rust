use crate::types::{Bounds, Glyph, PageText, TextBlock, TextLine};
use anyhow::{Context, Result, bail, ensure};
use mupdf::pdf::{
    PdfDocument, PdfPage, PdfRedactImageMethod, PdfRedactLineArtMethod, PdfRedactOptions,
};
use mupdf::shape::{FinishOptions, PdfColor, Shape, TextOptions, TextboxOptions};
use mupdf::{Colorspace, ImageFormat, Matrix, Point, Rect, Size, TextPageFlags};
use std::path::Path;

pub fn open(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    let mut doc = PdfDocument::open(path.to_str().context("non UTF-8 PDF path")?)
        .with_context(|| format!("opening {}", path.display()))?;
    if doc.needs_password()? {
        let password = password.context("PDF is encrypted; provide its password")?;
        ensure!(doc.authenticate(password)?, "incorrect PDF password");
    }
    Ok(doc)
}

pub fn save(doc: &PdfDocument, path: &Path) -> Result<()> {
    let mut options = mupdf::pdf::PdfWriteOptions::default();
    options
        .set_garbage_level(4)
        .set_compress(true)
        .set_compress_fonts(true)
        .set_compress_images(true);
    doc.save_with_options(path.to_str().context("invalid PDF output path")?, options)?;
    Ok(())
}

pub fn page_text(doc: &PdfDocument, index: usize) -> Result<PageText> {
    let page = doc.load_pdf_page(index.try_into()?)?;
    let text = page.to_text_page(TextPageFlags::PRESERVE_WHITESPACE)?;
    let mut blocks = Vec::new();
    for b in text.blocks() {
        let mut lines = Vec::new();
        for l in b.lines() {
            let glyphs = l
                .chars()
                .filter_map(|c| {
                    c.char().map(|ch| Glyph {
                        text: ch,
                        bounds: Rect::from(c.quad()).into(),
                        origin: [c.origin().x, c.origin().y],
                        size: c.size(),
                        font: c.font().map(|f| f.name().to_owned()).unwrap_or_default(),
                        argb: c.argb(),
                    })
                })
                .collect::<Vec<_>>();
            if !glyphs.is_empty() {
                lines.push(TextLine {
                    bounds: l.bounds().into(),
                    glyphs,
                });
            }
        }
        if !lines.is_empty() {
            blocks.push(TextBlock {
                bounds: b.bounds().into(),
                lines,
            });
        }
    }
    Ok(PageText {
        index,
        bounds: page.bounds()?.into(),
        rotation: page.rotation()?,
        blocks,
    })
}

pub fn render_rgb(page: &PdfPage, scale: f32) -> Result<image::RgbImage> {
    ensure!(
        scale.is_finite() && scale > 0.0 && scale <= 4.0,
        "render scale must be in (0, 4]"
    );
    let bounds = page.bounds()?;
    ensure!(
        bounds.width() * bounds.height() * scale * scale <= 32_000_000.0,
        "page raster exceeds 32 megapixel limit"
    );
    let pix = page.to_pixmap(
        &Matrix::new_scale(scale, scale),
        &Colorspace::device_rgb(),
        false,
        false,
    )?;
    let width = pix.width();
    let height = pix.height();
    let stride = pix.stride() as usize;
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);
    for row in pix.samples().chunks(stride).take(height as usize) {
        pixels.extend_from_slice(&row[..width as usize * 3]);
    }
    image::RgbImage::from_raw(width, height, pixels).context("invalid RGB raster")
}

pub fn render(input: &Path, output: &Path, index: usize, dpi: f32) -> Result<()> {
    let doc = open(input, None)?;
    let page = doc.load_pdf_page(index.try_into()?)?;
    let pix = page.to_pixmap(
        &Matrix::new_scale(dpi / 72., dpi / 72.),
        &Colorspace::device_rgb(),
        false,
        true,
    )?;
    let mut out = std::fs::File::create(output)?;
    pix.write_to(&mut out, ImageFormat::PNG)?;
    Ok(())
}

/// Remove only selected text. Preserve image pixels and vector paths explicitly:
/// MuPDF's default redaction would otherwise modify overlapping images.
pub fn remove_text(page: &mut PdfPage, glyphs: &[Glyph]) -> Result<()> {
    for g in glyphs {
        if g.text.is_whitespace() {
            continue;
        }
        let b = g.bounds;
        let cx = (b.x0 + b.x1) * 0.5;
        let cy = (b.y0 + b.y1) * 0.5;
        // A narrow interior rectangle selects this glyph without touching its neighbors.
        let dx = (b.width() * 0.05).clamp(0.01, 0.25);
        let dy = (b.height() * 0.05).clamp(0.01, 0.25);
        page.add_redact_annotation(Rect::new(cx - dx, cy - dy, cx + dx, cy + dy))?;
    }
    page.apply_redactions_with_options(PdfRedactOptions {
        image_method: PdfRedactImageMethod::None,
        line_art: PdfRedactLineArtMethod::None,
        ..PdfRedactOptions::mupdf_default()
    })?;
    Ok(())
}

/// Fit all text atomically. The Shape API writes nothing on overflow; error
/// instead of emitting a clipped or silently incomplete translated paragraph.
pub fn write_box(
    doc: &mut PdfDocument,
    page: &mut PdfPage,
    bounds: Bounds,
    text: &str,
    font: &[u8],
    initial_size: f32,
    min_size: f32,
    color: u32,
) -> Result<f32> {
    begin_actual_text(doc, page, text)?;
    let mut shape = Shape::new(page)?;
    let rgb = PdfColor::rgb(
        ((color >> 16) & 255) as f32 / 255.,
        ((color >> 8) & 255) as f32 / 255.,
        (color & 255) as f32 / 255.,
    );
    let mut size = initial_size.max(min_size);
    loop {
        let options = TextboxOptions {
            fontsize: size,
            fontname: "translation".into(),
            fontfile: Some(font),
            simple: false,
            lineheight: 1.1,
            color: Some(rgb.clone()),
            ..Default::default()
        };
        if shape.insert_textbox(bounds.into(), text, &options)? >= 0.0 {
            shape.commit(doc, true)?;
            drop(shape);
            page.insert_contents(doc, b"EMC\n", true)?;
            return Ok(size);
        }
        if size <= min_size {
            break;
        }
        size = (size - 0.5).max(min_size);
    }
    bail!(
        "translated text overflows box ({:.1},{:.1},{:.1},{:.1}) even at {:.1}pt",
        bounds.x0,
        bounds.y0,
        bounds.x1,
        bounds.y1,
        min_size
    )
}

fn begin_actual_text(doc: &mut PdfDocument, page: &mut PdfPage, text: &str) -> Result<()> {
    // CJK fonts can map compatibility ideographs to the same glyph. Preserve
    // the exact translated Unicode for search and copy through ActualText.
    let utf16 = text
        .encode_utf16()
        .map(|c| format!("{c:04X}"))
        .collect::<String>();
    page.insert_contents(
        doc,
        format!("/Span << /ActualText <FEFF{utf16}> >> BDC\n").as_bytes(),
        true,
    )?;
    Ok(())
}

pub fn write_at(
    doc: &mut PdfDocument,
    page: &mut PdfPage,
    point: Point,
    text: &str,
    font: &[u8],
    size: f32,
    color: u32,
) -> Result<()> {
    begin_actual_text(doc, page, text)?;
    let mut shape = Shape::new(page)?;
    shape.insert_text(
        point,
        text,
        &TextOptions {
            fontsize: size,
            fontname: "translation".into(),
            fontfile: Some(font),
            simple: false,
            color: Some(PdfColor::rgb(
                ((color >> 16) & 255) as f32 / 255.,
                ((color >> 8) & 255) as f32 / 255.,
                (color & 255) as f32 / 255.,
            )),
            ..Default::default()
        },
    )?;
    shape.commit(doc, true)?;
    drop(shape);
    page.insert_contents(doc, b"EMC\n", true)?;
    Ok(())
}

pub fn fixture(path: &Path) -> Result<()> {
    let mut doc = PdfDocument::new();
    let mut page = doc.new_page(Size::A4)?;
    let mut shape = Shape::new(&mut page)?;
    shape.insert_text(
        Point::new(72., 90.),
        "Hello world. This paragraph is translated while the formula stays intact.",
        &TextOptions {
            fontsize: 12.,
            ..Default::default()
        },
    )?;
    shape.insert_text(
        Point::new(72., 160.),
        "E = mc2",
        &TextOptions {
            fontsize: 18.,
            fontname: "Times-Italic".into(),
            ..Default::default()
        },
    )?;
    shape
        .draw_line(Point::new(72., 200.), Point::new(400., 200.))?
        .finish(&FinishOptions {
            color: Some(PdfColor::rgb(0., 0., 1.)),
            width: 2.,
            ..Default::default()
        })?;
    shape.commit(&mut doc, true)?;
    doc.save(path.to_str().context("invalid output path")?)?;
    Ok(())
}

pub fn probe(input: &Path, output: &Path, replacement: &str, font: &[u8]) -> Result<()> {
    ensure!(input != output, "input and output paths must differ");
    let mut doc = open(input, None)?;
    let page_ir = page_text(&doc, 0)?;
    let block = page_ir.blocks.first().context("PDF has no text")?;
    let glyphs = block
        .lines
        .iter()
        .flat_map(|l| l.glyphs.clone())
        .collect::<Vec<_>>();
    let mut page = doc.load_pdf_page(0)?;
    remove_text(&mut page, &glyphs)?;
    let bounds = Bounds {
        y1: block.bounds.y1.max(block.bounds.y0 + 40.),
        ..block.bounds
    };
    write_box(&mut doc, &mut page, bounds, replacement, font, 12., 7., 0)?;
    doc.save(output.to_str().context("invalid output path")?)?;
    Ok(())
}

pub fn dual_output(
    input: &Path,
    password: Option<&str>,
    translated: &PdfDocument,
    output: &Path,
    side_by_side: bool,
) -> Result<()> {
    use mupdf::pdf::{InsertPdfOptions, PageSelection};
    let original = open(input, password)?;
    let mut result = PdfDocument::new();
    for index in 0..translated.page_count()? {
        if !side_by_side {
            for source in [&original, translated] {
                result.insert_pdf(
                    source,
                    InsertPdfOptions {
                        source_pages: PageSelection::Pages(vec![index as usize]),
                        ..Default::default()
                    },
                )?;
            }
        } else {
            let left = original.load_pdf_page(index)?;
            let right = translated.load_pdf_page(index)?;
            let lb = left.bounds()?;
            let rb = right.bounds()?;
            let height = lb.height().max(rb.height());
            let left_form = page_form(&mut result, &left)?;
            let right_form = page_form(&mut result, &right)?;
            let mut page = result.new_page((lb.width() + rb.width(), height))?;
            let mut xobjects = result.new_dict()?;
            xobjects.dict_put("Original", left_form)?;
            xobjects.dict_put("Translated", right_form)?;
            page.resources()?.dict_put("XObject", xobjects)?;
            let content = format!(
                "q 1 0 0 1 0 {} cm /Original Do Q\nq 1 0 0 1 {} {} cm /Translated Do Q\n",
                height - lb.height(),
                lb.width(),
                height - rb.height()
            );
            page.insert_contents(&mut result, content.as_bytes(), true)?;
        }
    }
    copy_dual_links(&mut result, &original, translated, side_by_side)?;
    let mut outlines = original.outlines()?;
    if !side_by_side {
        remap_outlines(&mut outlines);
    }
    result.set_outlines(&outlines)?;
    save(&result, output)?;
    Ok(())
}

fn remap_outlines(outlines: &mut [mupdf::Outline]) {
    for outline in outlines {
        if let Some(dest) = outline.dest.as_mut() {
            dest.loc.page_number *= 2;
            dest.loc.page_in_chapter *= 2;
        }
        remap_outlines(&mut outline.down);
    }
}

fn copy_dual_links(
    result: &mut PdfDocument,
    original: &PdfDocument,
    translated: &PdfDocument,
    side_by_side: bool,
) -> Result<()> {
    use mupdf::pdf::{LinkAction, PdfAction, PdfDestination};
    for index in 0..original.page_count()? {
        for (side, source) in [original, translated].iter().enumerate() {
            let source_page = source.load_pdf_page(index)?;
            let mut links = source_page
                .resolved_links()?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for link in &mut links {
                if side_by_side && side == 1 {
                    let width = original.load_pdf_page(index)?.bounds()?.width();
                    link.bounds.x0 += width;
                    link.bounds.x1 += width;
                }
                let dest = match &mut link.action {
                    LinkAction::Dest(d) | LinkAction::Action(PdfAction::GoTo(d)) => Some(d),
                    _ => None,
                };
                if let Some(PdfDestination::Page { page, kind }) = dest {
                    if side_by_side {
                        if side == 1 {
                            *kind = kind.transform(&Matrix::new_translate(
                                original.load_pdf_page(*page as i32)?.bounds()?.width(),
                                0.,
                            ));
                        }
                    } else {
                        *page = *page * 2 + side as u32;
                    }
                }
            }
            let target = if side_by_side {
                index
            } else {
                index * 2 + side as i32
            };
            let mut page = result.load_pdf_page(target)?;
            if !side_by_side || side == 0 {
                if let Some(mut annotations) = page.object().get_dict("Annots")? {
                    for i in (0..annotations.len()?).rev() {
                        if let Some(a) = annotations.get_array(i as i32)? {
                            if a.get_dict("Subtype")?
                                .is_some_and(|s| s.as_name().is_ok_and(|v| v == b"Link"))
                            {
                                annotations.array_delete(i as i32)?;
                            }
                        }
                    }
                }
            }
            page.add_links(result, &links)?;
        }
    }
    Ok(())
}

fn page_form(dest: &mut PdfDocument, page: &PdfPage) -> Result<mupdf::pdf::PdfObject> {
    let mut bytes = Vec::new();
    if let Some(contents) = page.contents()? {
        if contents.is_array()? {
            for object in contents.array_iter()? {
                bytes.extend(object?.read_stream()?);
                bytes.push(b'\n');
            }
        } else {
            bytes = contents.read_stream()?;
        }
    }
    let mut dict = dest.new_dict()?;
    dict.dict_put("Type", dest.new_name("XObject")?)?;
    dict.dict_put("Subtype", dest.new_name("Form")?)?;
    let object = page.object();
    let bbox = object
        .get_dict_inheritable("CropBox")?
        .or(object.get_dict_inheritable("MediaBox")?)
        .context("page has no media box")?;
    dict.dict_put("BBox", dest.graft_object(&bbox)?)?;
    dict.dict_put("Resources", dest.graft_object(&page.resources()?)?)?;
    if let Some(group) = object.get_dict("Group")? {
        dict.dict_put("Group", dest.graft_object(&group)?)?;
    }
    let c = page.ctm()?;
    let bounds = page.bounds()?;
    // Source PDF coordinates -> displayed page coordinates -> PDF bottom-up
    // form coordinates. This also preserves rotated/cropped page appearance.
    let matrix = format!(
        "[{} {} {} {} {} {}]",
        c.a,
        -c.b,
        c.c,
        -c.d,
        c.e - bounds.x0,
        bounds.y1 - c.f
    );
    dict.dict_put("Matrix", dest.new_object_from_str(&matrix)?)?;
    Ok(dest.add_stream(&mupdf::Buffer::from_bytes(&bytes)?, Some(&dict), false)?)
}

/// Capture only the formula's visible region. Text outside the region is
/// removed in a temporary document before it is made into an XObject: a clip
/// alone could leave whole source paragraphs duplicated in searchable text.
pub fn formula_form(
    dest: &mut PdfDocument,
    source: &PdfDocument,
    index: usize,
    bounds: Bounds,
) -> Result<mupdf::pdf::PdfObject> {
    use mupdf::pdf::{InsertPdfOptions, PageSelection};
    let mut temp = PdfDocument::new();
    temp.insert_pdf(
        source,
        InsertPdfOptions {
            source_pages: PageSelection::Pages(vec![index]),
            ..Default::default()
        },
    )?;
    let mut page = temp.load_pdf_page(0)?;
    page.set_rotation(0)?;
    let full = page.bounds()?;
    let pad = 0.05;
    let b = Bounds {
        x0: bounds.x0 - pad,
        y0: bounds.y0 - pad,
        x1: bounds.x1 + pad,
        y1: bounds.y1 + pad,
    };
    for r in [
        Rect::new(full.x0, full.y0, full.x1, b.y0),
        Rect::new(full.x0, b.y1, full.x1, full.y1),
        Rect::new(full.x0, b.y0, b.x0, b.y1),
        Rect::new(b.x1, b.y0, full.x1, b.y1),
    ] {
        if r.width() > 0. && r.height() > 0. {
            page.add_redact_annotation(r)?;
        }
    }
    page.apply_redactions_with_options(PdfRedactOptions {
        image_method: PdfRedactImageMethod::None,
        line_art: PdfRedactLineArtMethod::None,
        ..PdfRedactOptions::mupdf_default()
    })?;
    let original = page_form(dest, &page)?;
    let mut xobjects = dest.new_dict()?;
    xobjects.dict_put("Source", original)?;
    let mut resources = dest.new_dict()?;
    resources.dict_put("XObject", xobjects)?;
    let mut dict = dest.new_dict()?;
    dict.dict_put("Type", dest.new_name("XObject")?)?;
    dict.dict_put("Subtype", dest.new_name("Form")?)?;
    dict.dict_put(
        "BBox",
        dest.new_object_from_str(&format!("[0 0 {} {}]", bounds.width(), bounds.height()))?,
    )?;
    dict.dict_put("Resources", resources)?;
    let stream = format!(
        "q 1 0 0 1 {} {} cm /Source Do Q",
        -bounds.x0,
        -(full.height() - bounds.y1)
    );
    Ok(dest.add_stream(
        &mupdf::Buffer::from_bytes(stream.as_bytes())?,
        Some(&dict),
        false,
    )?)
}

pub fn draw_form(
    doc: &mut PdfDocument,
    page: &mut PdfPage,
    form: mupdf::pdf::PdfObject,
    x: f32,
    bottom: f32,
    scale: f32,
) -> Result<()> {
    let name = format!("Math{}", uuid::Uuid::new_v4().simple());
    let mut resources = page.resources()?;
    let mut xobjects = resources.get_dict("XObject")?.unwrap_or(doc.new_dict()?);
    xobjects.dict_put(name.as_str(), form)?;
    resources.dict_put("XObject", xobjects)?;
    let inv = page.ctm()?.invert().context("non-invertible page matrix")?;
    let stream = format!(
        "q {} {} {} {} {} {} cm /{} Do Q\n",
        inv.a * scale,
        inv.b * scale,
        -inv.c * scale,
        -inv.d * scale,
        inv.a * x + inv.c * bottom + inv.e,
        inv.b * x + inv.d * bottom + inv.f,
        name
    );
    page.insert_contents(doc, stream.as_bytes(), true)?;
    Ok(())
}
