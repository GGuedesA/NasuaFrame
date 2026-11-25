use image::{GrayImage, RgbaImage, imageops};

const NAMES_TOP: f32 = 0.37;
const NAMES_BOTTOM: f32 = 0.44;
const CARD_WIDTH: f32 = 0.2235;

pub fn names_strip(img: &RgbaImage) -> RgbaImage {
    let (w, h) = (img.width() as f32, img.height() as f32);
    let half = (2.0 * CARD_WIDTH * h).min(w / 2.0);
    let x = (w / 2.0 - half) as u32;
    let y = (NAMES_TOP * h) as u32;
    let width = (2.0 * half) as u32;
    let height = ((NAMES_BOTTOM - NAMES_TOP) * h) as u32;
    imageops::crop_imm(img, x, y, width, height).to_image()
}

pub fn prepare(img: &RgbaImage) -> GrayImage {
    let mut gray = imageops::grayscale(img);
    imageops::invert(&mut gray);
    imageops::resize(
        &gray,
        gray.width() * 2,
        gray.height() * 2,
        imageops::FilterType::Triangle,
    )
}

#[cfg(windows)]
pub fn capture_primary() -> Result<RgbaImage, xcap::XCapError> {
    let monitors = xcap::Monitor::all()?;
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or(monitors.first())
        .ok_or_else(|| xcap::XCapError::new("no monitor found"))?;
    monitor.capture_image()
}

#[cfg(windows)]
pub mod ocr {
    use image::GrayImage;
    use nasua_core::reward::Word;
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    use windows::core::{HSTRING, Result};

    pub fn languages() -> Result<Vec<String>> {
        OcrEngine::AvailableRecognizerLanguages()?
            .into_iter()
            .map(|l| l.LanguageTag().map(|t| t.to_string()))
            .collect()
    }

    /// none uses the windows language
    pub fn read(img: &GrayImage, language: Option<&str>) -> Result<Vec<Word>> {
        let engine = match language {
            Some(tag) => {
                OcrEngine::TryCreateFromLanguage(&Language::CreateLanguage(&HSTRING::from(tag))?)?
            }
            None => OcrEngine::TryCreateFromUserProfileLanguages()?,
        };

        let bgra: Vec<u8> = img
            .pixels()
            .flat_map(|p| [p.0[0], p.0[0], p.0[0], 255])
            .collect();
        let writer = DataWriter::new()?;
        writer.WriteBytes(&bgra)?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &writer.DetachBuffer()?,
            BitmapPixelFormat::Bgra8,
            img.width() as i32,
            img.height() as i32,
        )?;

        let result = engine.RecognizeAsync(&bitmap)?.join()?;
        let mut words = Vec::new();
        for (line, ocr_line) in result.Lines()?.into_iter().enumerate() {
            for word in ocr_line.Words()? {
                let rect = word.BoundingRect()?;
                words.push(Word {
                    text: word.Text()?.to_string(),
                    line,
                    x: rect.X,
                    width: rect.Width,
                    height: rect.Height,
                });
            }
        }
        Ok(words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn names_strip_is_centered_and_scales_with_height() {
        let img = RgbaImage::new(1360, 716);
        let strip = names_strip(&img);
        assert_eq!(strip.width(), 640);
        assert_eq!(strip.height(), 50);

        let wide = names_strip(&RgbaImage::new(2720, 716));
        assert_eq!(wide.width(), 640);
    }

    #[test]
    fn prepare_turns_light_text_dark() {
        let img = RgbaImage::from_fn(4, 4, |x, _| {
            if x == 0 {
                Rgba([240, 240, 240, 255])
            } else {
                Rgba([10, 10, 10, 255])
            }
        });
        let out = prepare(&img);
        assert_eq!(out.dimensions(), (8, 8));
        assert!(out.get_pixel(0, 0).0[0] < 30);
        assert!(out.get_pixel(7, 7).0[0] > 230);
    }
}
