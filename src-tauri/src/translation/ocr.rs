use crate::translation::types::{OcrError, Region};

/// Recognise English text in an RGBA8 frame using Apple Vision Framework.
///
/// # Arguments
/// * `rgba`   – raw RGBA8 pixel data (width × height × 4 bytes, row-major)
/// * `width`  – image width in pixels
/// * `height` – image height in pixels
/// * `region` – optional crop rectangle; when `None` the full frame is used
///
/// # Returns
/// The recognised text with candidates joined by newlines, or an error.
#[cfg(target_os = "macos")]
pub fn recognize_english(
    rgba: &[u8],
    width: u32,
    height: u32,
    region: Option<Region>,
) -> Result<String, OcrError> {
    use image::{GenericImageView, ImageBuffer, RgbaImage};
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_core_graphics::{
        CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage,
        CGImageAlphaInfo,
    };
    use objc2_foundation::{NSArray, NSDictionary, NSString};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequestTextRecognitionLevel,
    };

    // --- 1. Optionally crop to the requested region ----------------------
    let cropped_buf: Vec<u8>;
    let (img_width, img_height, img_rgba): (u32, u32, &[u8]) = match region {
        None => (width, height, rgba),
        Some(r) => {
            // Validate the region fits inside the frame
            if r.x + r.width > width || r.y + r.height > height || r.width == 0 || r.height == 0 {
                return Err(OcrError::InvalidImage(format!(
                    "region {:?} is out of bounds for {}×{} image",
                    r, width, height
                )));
            }
            // Use image crate to sub-image (zero-copy view where possible)
            let full: RgbaImage = ImageBuffer::from_raw(width, height, rgba.to_vec())
                .ok_or_else(|| OcrError::InvalidImage("buffer size mismatch".into()))?;
            let view = full.view(r.x, r.y, r.width, r.height);
            cropped_buf = view.to_image().into_raw();
            (r.width, r.height, cropped_buf.as_slice())
        }
    };

    // --- 2. Build a CGImage from the RGBA bytes --------------------------
    let color_space = CGColorSpace::new_device_rgb()
        .ok_or_else(|| OcrError::VisionFramework("CGColorSpaceCreateDeviceRGB failed".into()))?;

    // SAFETY: `img_rgba` outlives `provider` and `cg_image` within this function.
    let provider = unsafe {
        CGDataProvider::with_data(
            std::ptr::null_mut(),
            img_rgba.as_ptr() as *const std::ffi::c_void,
            img_rgba.len(),
            None, // no release callback; the Rust slice owns the data
        )
    }
    .ok_or_else(|| OcrError::VisionFramework("CGDataProviderCreateWithData failed".into()))?;

    // RGBA8 = 8 bits per component, 32 bits per pixel, last byte is alpha (non-premultiplied)
    let bitmap_info = CGBitmapInfo(CGImageAlphaInfo::Last.0);

    let cg_image = unsafe {
        CGImage::new(
            img_width as usize,
            img_height as usize,
            8,                        // bits_per_component
            32,                       // bits_per_pixel
            (img_width as usize) * 4, // bytes_per_row
            Some(&color_space),
            bitmap_info,
            Some(&provider),
            std::ptr::null(), // decode array (null = no remapping)
            false,            // should_interpolate
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
    .ok_or_else(|| OcrError::VisionFramework("CGImageCreate failed".into()))?;

    // --- 3. Build and configure VNRecognizeTextRequest -------------------
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);

    let lang = NSString::from_str("en-US");
    let langs = NSArray::from_slice(&[lang.as_ref()]);
    request.setRecognitionLanguages(&langs);

    request.setUsesLanguageCorrection(true);
    request.setMinimumTextHeight(0.0);

    // --- 4. Execute via VNImageRequestHandler ----------------------------
    let options: Retained<NSDictionary<_, _>> = NSDictionary::new();
    let handler = unsafe {
        VNImageRequestHandler::initWithCGImage_options(
            <VNImageRequestHandler as AnyThread>::alloc(),
            &cg_image,
            &options,
        )
    };

    // VNRequest is the base class; we need to upcast for the array
    use objc2_vision::VNRequest;
    let request_array: Retained<NSArray<VNRequest>> = {
        // SAFETY: VNRecognizeTextRequest is a subclass of VNRequest.
        let req_ref: &VNRequest = &request;
        NSArray::from_slice(&[req_ref])
    };

    handler
        .performRequests_error(&request_array)
        .map_err(|e| OcrError::VisionFramework(e.localizedDescription().to_string()))?;

    // --- 5. Collect results ----------------------------------------------
    let observations = request.results().unwrap_or_default();

    let mut lines: Vec<String> = Vec::new();
    for obs in observations.iter() {
        let candidates = obs.topCandidates(1);
        if let Some(candidate) = candidates.firstObject() {
            if candidate.confidence() >= 0.4 {
                lines.push(candidate.string().to_string());
            }
        }
    }

    let text = lines.join("\n").trim().to_string();
    if text.is_empty() {
        Err(OcrError::NoTextFound)
    } else {
        Ok(text)
    }
}

/// Stub for non-macOS targets so the crate still compiles on other platforms.
#[cfg(not(target_os = "macos"))]
pub fn recognize_english(
    _rgba: &[u8],
    _width: u32,
    _height: u32,
    _region: Option<Region>,
) -> Result<String, OcrError> {
    Err(OcrError::UnsupportedPlatform)
}
