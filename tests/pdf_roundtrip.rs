use pdf2zh_rust::{pdf, resources};

#[test]
fn replacing_text_preserves_formula_and_vector_pixels() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let original = tmp.path().join("input.pdf");
    let output = tmp.path().join("output.pdf");
    pdf::fixture(&original)?;
    let before = pdf::open(&original, None)?;
    let before_image = pdf::render_rgb(&before.load_pdf_page(0)?, 1.)?;
    pdf::probe(
        &original,
        &output,
        "你好，世界。这是编码器的力量。",
        &resources::font_bytes()?,
    )?;
    let after = pdf::open(&output, None)?;
    let text = pdf::page_text(&after, 0)?
        .blocks
        .iter()
        .map(|b| b.text())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("你好"),
        "Chinese must remain searchable: {text}"
    );
    assert!(
        text.contains("编码器的力量"),
        "exact CJK Unicode must survive compatibility glyph mappings: {text}"
    );
    assert!(text.contains("E = mc2"), "formula was changed: {text}");
    assert!(
        !text.contains("Hello world"),
        "original paragraph still present"
    );
    let after_image = pdf::render_rgb(&after.load_pdf_page(0)?, 1.)?;
    for y in 135..220 {
        for x in 60..420 {
            assert_eq!(
                before_image.get_pixel(x, y),
                after_image.get_pixel(x, y),
                "protected formula/vector changed at {x},{y}"
            );
        }
    }
    Ok(())
}

#[test]
fn moving_formula_does_not_duplicate_clipped_source_paragraphs() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let input = tmp.path().join("input.pdf");
    pdf::fixture(&input)?;
    let original = pdf::open(&input, None)?;
    let info = pdf::page_text(&original, 0)?;
    let formula = info
        .blocks
        .iter()
        .find(|b| b.text().contains("mc2"))
        .unwrap();
    let mut result = mupdf::pdf::PdfDocument::new();
    let mut page = result.new_page(mupdf::Size::A4)?;
    let form = pdf::formula_form(&mut result, &original, 0, formula.bounds)?;
    pdf::draw_form(&mut result, &mut page, form, 72., 250., 1.)?;
    let path = tmp.path().join("moved.pdf");
    result.save(path.to_str().unwrap())?;
    let reopened = pdf::open(&path, None)?;
    let text = pdf::page_text(&reopened, 0)?
        .blocks
        .iter()
        .map(|b| b.text())
        .collect::<String>();
    assert!(text.contains("mc2"), "formula was lost: {text}");
    assert!(
        !text.contains("Hello"),
        "clipped source text leaked into output: {text}"
    );
    Ok(())
}

#[test]
fn side_by_side_preserves_rotated_and_cropped_page_rendering() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    for rotation in [0, 90, 180, 270] {
        let source = tmp.path().join(format!("input-{rotation}.pdf"));
        pdf::fixture(&source)?;
        let doc = pdf::open(&source, None)?;
        let mut page = doc.load_pdf_page(0)?;
        page.set_crop_box(mupdf::Rect::new(40., 40., 500., 700.))?;
        page.set_rotation(rotation)?;
        let input = tmp.path().join(format!("cropped-{rotation}.pdf"));
        doc.save(input.to_str().unwrap())?;
        let original = pdf::open(&input, None)?;
        let before = pdf::render_rgb(&original.load_pdf_page(0)?, 1.)?;
        let output = tmp.path().join(format!("dual-{rotation}.pdf"));
        pdf::dual_output(&input, None, &original, &output, true)?;
        let dual = pdf::open(&output, None)?;
        let after = pdf::render_rgb(&dual.load_pdf_page(0)?, 1.)?;
        for y in 0..before.height() {
            for x in 0..before.width() {
                assert_eq!(
                    before.get_pixel(x, y),
                    after.get_pixel(x, y),
                    "rotation {rotation}, pixel {x},{y}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn dual_outputs_remap_internal_links_and_outlines() -> anyhow::Result<()> {
    use mupdf::{
        DestinationKind,
        pdf::{LinkAction, PdfAction, PdfDestination, PdfDocument, PdfLink},
    };
    let tmp = tempfile::tempdir()?;
    let mut doc = PdfDocument::new();
    let mut page = doc.new_page(mupdf::Size::A4)?;
    doc.new_page(mupdf::Size::A4)?;
    page.add_links(
        &mut doc,
        &[PdfLink {
            bounds: mupdf::Rect::new(40., 40., 100., 60.),
            action: LinkAction::Action(PdfAction::GoTo(PdfDestination::Page {
                page: 1,
                kind: DestinationKind::Fit,
            })),
        }],
    )?;
    doc.set_outlines(&[mupdf::Outline {
        title: "Second page".into(),
        uri: None,
        dest: Some(mupdf::link::LinkDestination {
            loc: mupdf::document::Location {
                chapter: 0,
                page_in_chapter: 1,
                page_number: 1,
            },
            kind: DestinationKind::Fit,
        }),
        down: Vec::new(),
    }])?;
    let input = tmp.path().join("input.pdf");
    doc.save(input.to_str().unwrap())?;
    for side in [false, true] {
        let output = tmp.path().join(format!("dual-{side}.pdf"));
        pdf::dual_output(&input, None, &doc, &output, side)?;
        let dual = pdf::open(&output, None)?;
        let expected = if side { 1 } else { 2 };
        assert_eq!(dual.outlines()?[0].dest.unwrap().loc.page_number, expected);
        let links = dual
            .load_pdf_page(0)?
            .resolved_links()?
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(links.len(), if side { 2 } else { 1 });
        assert!(links.iter().all(|l|matches!(l.action.destination(),Some(PdfDestination::Page{page,..}) if *page==expected)));
    }
    Ok(())
}
