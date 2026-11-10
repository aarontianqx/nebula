//! macOS implementation of pixel color detection.
//!
//! Captures a 1x1 region at the requested point with
//! `CGWindowListCreateImage` and reads the top-left pixel. The coordinates are
//! interpreted in the global *point* space (the same space used for injection
//! on macOS); on Retina displays the captured image is denser than one device
//! pixel, and we sample its top-left pixel.
//!
//! `CGWindowListCreateImage` is deprecated by Apple (obsoleted in the macOS 15 SDK headers). A
//! ScreenCaptureKit alternative was measured (`examples/pixel_probe.rs`, proposal WS6c) and found
//! comparable in latency but not pixel-identical, so the capture call stays. What changes is how
//! it is reached: the function is looked up by name when first needed, not linked as an import.
//! A linked import that the system no longer provides ends the whole process (dyld, at the first
//! call or at launch); a lookup that finds nothing is just "no color".
//!
//! Note: screen capture requires the Screen Recording permission. Without it
//! the returned image is empty/blank and this function yields `None` or a
//! black pixel.

use std::ffi::{c_void, CStr};
use std::sync::OnceLock;

use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::image::CGImage;
use core_graphics::window::{
    kCGNullWindowID, kCGWindowImageDefault, kCGWindowListOptionOnScreenOnly, CGWindowID,
    CGWindowImageOption, CGWindowListOption,
};
use foreign_types::ForeignType;
use tracing::warn;

use tap_core::Color;

/// `CGWindowListCreateImage(screenBounds, listOption, windowID, imageOption)`.
type CreateImageFn = unsafe extern "C" fn(
    CGRect,
    CGWindowListOption,
    CGWindowID,
    CGWindowImageOption,
) -> core_graphics::sys::CGImageRef;

const CORE_GRAPHICS: &CStr = c"/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics";
const CREATE_IMAGE_SYMBOL: &CStr = c"CGWindowListCreateImage";

/// Look `name` up among the functions of CoreGraphics.
///
/// The framework is opened by path (it is always loaded already, and system frameworks resolve
/// from the dyld shared cache even though no file is on disk); if that fails the process-wide
/// namespace is searched instead.
fn find_symbol(name: &CStr) -> Option<*mut c_void> {
    // SAFETY: both strings are NUL-terminated; the handle is only passed to `dlsym` and is never
    // closed, so the framework stays loaded for the life of the process.
    let found = unsafe {
        let handle = libc::dlopen(CORE_GRAPHICS.as_ptr(), libc::RTLD_LAZY);
        let handle = if handle.is_null() {
            libc::RTLD_DEFAULT
        } else {
            handle
        };
        libc::dlsym(handle, name.as_ptr())
    };
    (!found.is_null()).then_some(found)
}

/// The capture function, looked up once. `None` when this system no longer has it.
fn create_image_fn() -> Option<CreateImageFn> {
    static CREATE_IMAGE: OnceLock<Option<CreateImageFn>> = OnceLock::new();
    *CREATE_IMAGE.get_or_init(|| match find_symbol(CREATE_IMAGE_SYMBOL) {
        // SAFETY: the symbol's documented signature is `CreateImageFn` (the declaration
        // `core-graphics` links against, and the SDK header).
        Some(symbol) => Some(unsafe { std::mem::transmute::<*mut c_void, CreateImageFn>(symbol) }),
        None => {
            warn!("CGWindowListCreateImage is not available on this system; pixel colors cannot be read");
            None
        }
    })
}

/// Get the color of a pixel at the given screen coordinates (points).
pub fn get_pixel_color(x: i32, y: i32) -> Option<Color> {
    sample(create_image_fn()?, x, y)
}

fn sample(create_image: CreateImageFn, x: i32, y: i32) -> Option<Color> {
    let rect = CGRect::new(&CGPoint::new(x as f64, y as f64), &CGSize::new(1.0, 1.0));
    // SAFETY: plain call with by-value arguments. A non-null result follows the Create rule: it
    // is owned by the caller, which `CGImage` releases on drop.
    let image = unsafe {
        let raw = create_image(
            rect,
            kCGWindowListOptionOnScreenOnly,
            kCGNullWindowID,
            kCGWindowImageDefault,
        );
        if raw.is_null() {
            return None;
        }
        CGImage::from_ptr(raw)
    };
    top_left_pixel(&image)
}

/// The color of an image's top-left pixel.
fn top_left_pixel(image: &CGImage) -> Option<Color> {
    if image.width() == 0 || image.height() == 0 || image.bits_per_pixel() < 32 {
        return None;
    }

    let data = image.data();
    let bytes = data.bytes();
    if bytes.len() < 4 {
        return None;
    }

    // CGWindowListCreateImage yields 32-bit pixels in little-endian BGRA order,
    // so the first four bytes of the top-left pixel are B, G, R, A.
    Some(Color::new(bytes[2], bytes[1], bytes[0]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::base::{
        kCGImageAlphaNoneSkipFirst, kCGImageAlphaPremultipliedFirst, kCGRenderingIntentDefault,
    };
    use core_graphics::color_space::CGColorSpace;
    use core_graphics::data_provider::CGDataProvider;
    use std::sync::Arc;

    /// A 1x1 image of 32-bit BGRA pixels, as the capture function hands them over.
    fn bgra_image(bgra: [u8; 4]) -> CGImage {
        const BYTE_ORDER_32_LITTLE: u32 = 2 << 12;
        let provider = CGDataProvider::from_buffer(Arc::new(bgra.to_vec()));
        CGImage::new(
            1,
            1,
            8,
            32,
            4,
            &CGColorSpace::create_device_rgb(),
            kCGImageAlphaPremultipliedFirst | BYTE_ORDER_32_LITTLE,
            &provider,
            false,
            kCGRenderingIntentDefault,
        )
    }

    /// A 1x1 image with 16 bits per pixel (5 per component): too small to read a color from.
    fn rgb555_image() -> CGImage {
        let provider = CGDataProvider::from_buffer(Arc::new(vec![0u8, 0u8]));
        CGImage::new(
            1,
            1,
            5,
            16,
            2,
            &CGColorSpace::create_device_rgb(),
            kCGImageAlphaNoneSkipFirst,
            &provider,
            false,
            kCGRenderingIntentDefault,
        )
    }

    #[test]
    fn the_pixel_is_read_from_bgra_bytes() {
        let image = bgra_image([30, 20, 10, 255]);
        assert_eq!(top_left_pixel(&image), Some(Color::new(10, 20, 30)));
    }

    #[test]
    fn an_image_that_is_not_32_bit_is_not_read() {
        assert_eq!(top_left_pixel(&rgb555_image()), None);
    }

    /// A stand-in for a capture function that has nothing to return (no permission, no display).
    unsafe extern "C" fn captures_nothing(
        _: CGRect,
        _: CGWindowListOption,
        _: CGWindowID,
        _: CGWindowImageOption,
    ) -> core_graphics::sys::CGImageRef {
        std::ptr::null_mut()
    }

    #[test]
    fn a_capture_that_returns_no_image_is_no_color() {
        assert_eq!(sample(captures_nothing, 10, 10), None);
    }

    #[test]
    fn a_name_the_system_does_not_have_is_not_found() {
        assert!(find_symbol(c"TapNoSuchCoreGraphicsFunction").is_none());
    }

    #[test]
    fn a_function_the_system_does_have_is_found() {
        // Not the deprecated one, whose absence is exactly what the lookup is there to survive.
        assert!(find_symbol(c"CGWindowListCopyWindowInfo").is_some());
    }

    #[test]
    fn asking_for_a_color_never_takes_the_process_down() {
        // Whether the symbol exists, the screen is readable or not, this returns normally.
        let _ = get_pixel_color(0, 0);
    }
}
