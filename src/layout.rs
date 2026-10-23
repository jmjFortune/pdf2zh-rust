use crate::types::{Bounds, LayoutRegion};
use anyhow::{Context, Result, ensure};
use image::{RgbImage, imageops::FilterType};
use ort::{environment::Environment, session::Session, value::Tensor};
use std::{collections::BTreeMap, path::Path};

pub struct LayoutModel {
    session: Session,
    _environment: Environment,
    labels: BTreeMap<usize, String>,
}

impl LayoutModel {
    pub fn load(path: &Path) -> Result<Self> {
        ensure!(path.is_file(), "layout model missing: {}", path.display());
        let builder = match crate::resources::runtime_path() {
            Some(p) => ort::init_from(p).context("loading native ONNX Runtime")?,
            None => ort::init(),
        };
        let environment = builder.with_telemetry(false).build()?;
        let session = Session::builder(&environment)?
            // Release page-operation scratch allocations while the document
            // worker waits for remote translation responses.
            .with_execution_providers([ort::ep::CPU::default().with_arena_allocator(false).build()])
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .with_intra_threads(2)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .with_inter_threads(1)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .commit_from_file(path)?;
        let names = session
            .metadata()?
            .custom("names")
            .context("model does not contain class names")?;
        // This pinned model serializes its names as a Python dict of integer keys
        // and quoted strings, not executable expressions. Parse only that grammar.
        let mut labels = BTreeMap::new();
        for item in names.trim_matches(|c| c == '{' || c == '}').split(',') {
            let (k, v) = item
                .split_once(':')
                .context("invalid model class name metadata")?;
            labels.insert(
                k.trim().parse()?,
                v.trim().trim_matches(|c| c == '\'' || c == '"').to_string(),
            );
        }
        ensure!(!labels.is_empty(), "model has no labels");
        Ok(Self {
            session,
            _environment: environment,
            labels,
        })
    }

    pub fn predict(&mut self, image: &RgbImage, page: Bounds) -> Result<Vec<LayoutRegion>> {
        let (tensor, gain, pad_x, pad_y) = letterbox(image)?;
        let tensor = Tensor::from_array(([1, 3, 1024, 1024], tensor))?;
        let outputs = self.session.run(ort::inputs!["images" => tensor])?;
        let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        ensure!(
            shape.last() == Some(&6),
            "unsupported layout model output: {shape:?}; expected xyxy/confidence/class"
        );
        let sx = page.width() / image.width() as f32;
        let sy = page.height() / image.height() as f32;
        let mut regions = Vec::new();
        for row in data.chunks_exact(6) {
            if row.iter().any(|x| !x.is_finite()) || row[4] < 0.25 {
                continue;
            }
            let class_id = row[5] as usize;
            let label = self
                .labels
                .get(&class_id)
                .context("unknown layout class")?
                .clone();
            let bounds = Bounds {
                x0: page.x0 + ((row[0] - pad_x) / gain).clamp(0., image.width() as f32) * sx,
                y0: page.y0 + ((row[1] - pad_y) / gain).clamp(0., image.height() as f32) * sy,
                x1: page.x0 + ((row[2] - pad_x) / gain).clamp(0., image.width() as f32) * sx,
                y1: page.y0 + ((row[3] - pad_y) / gain).clamp(0., image.height() as f32) * sy,
            };
            if bounds.width() > 0. && bounds.height() > 0. {
                regions.push(LayoutRegion {
                    bounds,
                    class_id,
                    label,
                    confidence: row[4],
                });
            }
        }
        Ok(regions)
    }
}

fn letterbox(image: &RgbImage) -> Result<(Vec<f32>, f32, f32, f32)> {
    ensure!(
        image.width() > 0 && image.height() > 0,
        "empty layout image"
    );
    let size = 1024usize;
    let gain = (size as f32 / image.width() as f32).min(size as f32 / image.height() as f32);
    let w = (image.width() as f32 * gain).round() as u32;
    let h = (image.height() as f32 * gain).round() as u32;
    let resized = image::imageops::resize(image, w, h, FilterType::Triangle);
    let px = (size - w as usize) / 2;
    let py = (size - h as usize) / 2;
    let mut data = vec![114.0 / 255.; 3 * size * size];
    for (x, y, p) in resized.enumerate_pixels() {
        for c in 0..3 {
            // BabelDOC feeds OpenCV BGR pixels to this model. Preserve that order.
            data[c * size * size + (y as usize + py) * size + x as usize + px] =
                p[2 - c] as f32 / 255.;
        }
    }
    Ok((data, gain, px as f32, py as f32))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn letterbox_uses_bgr_and_centered_padding() {
        let image = RgbImage::from_pixel(2, 1, image::Rgb([255, 0, 0]));
        let (x, gain, px, py) = letterbox(&image).unwrap();
        assert_eq!((gain, px, py), (512., 0., 256.));
        let i = 300 * 1024;
        assert_eq!(x[i], 0.);
        assert_eq!(x[2 * 1024 * 1024 + i], 1.);
        assert_eq!(x[0], 114. / 255.);
    }
}
