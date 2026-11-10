//! Manual probe for the macOS pixel sampler: how does ScreenCaptureKit compare with the
//! `CGWindowListCreateImage` implementation `tap-platform` uses today?
//!
//! `cargo run --release -p tap-platform --example pixel_probe`
//!
//! Needs the Screen Recording permission for the terminal that runs it. It prints, for each
//! sampler, the single-point latency distribution (what a `wait_until` poll pays, against the
//! 100 ms default interval) and then how often the two disagree on the same points.
//!
//! Spike code: nothing here ships. The conclusions are in
//! `specs/proposals/editor_and_engine_hardening.md` (WS6c).

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("pixel_probe only exists for macOS");
}

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use block2::RcBlock;
    use core_graphics::display::CGDisplay;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{CGDataProvider, CGImage};
    use objc2_foundation::NSError;
    use objc2_screen_capture_kit::SCScreenshotManager;
    use tap_platform::{get_pixel_color, Color};

    const SAMPLES: usize = 300;
    /// Samples taken at the `wait_until` cadence (one per poll interval).
    const POLLED: usize = 100;
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const WARMUP: usize = 5;
    const TIMEOUT: Duration = Duration::from_secs(3);

    /// What a failed sample says about why. The payload is only read through `Debug`.
    #[allow(dead_code)]
    #[derive(Debug)]
    enum SckError {
        NoImage(String),
        Timeout,
        TooSmall,
    }

    /// One pixel through `SCScreenshotManager.captureImageInRect:` (macOS 15.2+), blocking until
    /// its completion handler has run.
    fn sck_pixel(x: i32, y: i32) -> Result<Color, SckError> {
        let (tx, rx) = mpsc::channel::<Result<Color, SckError>>();
        let handler = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
            let result = if image.is_null() {
                let why = unsafe { error.as_ref() }
                    .map(|e| e.localizedDescription().to_string())
                    .unwrap_or_else(|| "no image and no error".into());
                Err(SckError::NoImage(why))
            } else {
                // The image belongs to the framework for the duration of the callback.
                read_top_left(unsafe { &*image })
            };
            let _ = tx.send(result);
        });

        let rect = CGRect::new(
            CGPoint::new(f64::from(x), f64::from(y)),
            CGSize::new(1.0, 1.0),
        );
        unsafe { SCScreenshotManager::captureImageInRect_completionHandler(rect, Some(&handler)) };
        rx.recv_timeout(TIMEOUT).unwrap_or(Err(SckError::Timeout))
    }

    fn read_top_left(image: &CGImage) -> Result<Color, SckError> {
        if CGImage::width(Some(image)) == 0
            || CGImage::height(Some(image)) == 0
            || CGImage::bits_per_pixel(Some(image)) < 32
        {
            return Err(SckError::TooSmall);
        }
        let provider = CGImage::data_provider(Some(image)).ok_or(SckError::TooSmall)?;
        let data = CGDataProvider::data(Some(&provider)).ok_or(SckError::TooSmall)?;
        // SAFETY: `data` is alive for the whole read and nothing mutates it.
        let bytes = unsafe { data.as_bytes_unchecked() };
        if bytes.len() < 4 {
            return Err(SckError::TooSmall);
        }
        // BGRA, like the Quartz path.
        Ok(Color::new(bytes[2], bytes[1], bytes[0]))
    }

    /// Evenly spread points inside the main display, away from its edges.
    fn sample_points() -> Vec<(i32, i32)> {
        let bounds = CGDisplay::main().bounds();
        let (w, h) = (bounds.size.width as i32, bounds.size.height as i32);
        let cols = 20;
        let rows = SAMPLES.div_ceil(cols) as i32;
        (0..SAMPLES as i32)
            .map(|i| {
                let (c, r) = (i % cols as i32, i / cols as i32);
                (
                    bounds.origin.x as i32 + 40 + c * (w - 80) / cols as i32,
                    bounds.origin.y as i32 + 40 + r * (h - 80) / rows,
                )
            })
            .collect()
    }

    struct Latencies(Vec<Duration>);

    impl Latencies {
        fn percentile(&self, p: f64) -> Duration {
            let mut sorted = self.0.clone();
            sorted.sort();
            sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
        }

        fn report(&self, name: &str) {
            let ms = |d: Duration| d.as_secs_f64() * 1000.0;
            println!(
                "{name:<22} n={:<4} p50={:>7.2} ms  p95={:>7.2} ms  p99={:>7.2} ms  max={:>7.2} ms",
                self.0.len(),
                ms(self.percentile(0.50)),
                ms(self.percentile(0.95)),
                ms(self.percentile(0.99)),
                ms(self.percentile(1.0)),
            );
        }
    }

    pub fn run() {
        let points = sample_points();
        println!(
            "{} points on the main display ({:?})",
            points.len(),
            CGDisplay::main().bounds()
        );

        // Quartz: what the app does today.
        for &(x, y) in points.iter().take(WARMUP) {
            let _ = get_pixel_color(x, y);
        }
        let mut quartz = Vec::new();
        let mut quartz_colors = Vec::new();
        for &(x, y) in &points {
            let began = Instant::now();
            let color = get_pixel_color(x, y);
            quartz.push(began.elapsed());
            quartz_colors.push(color);
        }
        let blank = quartz_colors.iter().filter(|c| c.is_none()).count();
        Latencies(quartz).report("CGWindowListCreateImage");
        println!("  unreadable samples: {blank}");

        // ScreenCaptureKit. The first call loads the framework and may prompt for permission,
        // so it is timed on its own and left out of the distribution.
        let first = Instant::now();
        let first_result = sck_pixel(points[0].0, points[0].1);
        println!(
            "SCK first call: {:.1} ms -> {first_result:?}",
            first.elapsed().as_secs_f64() * 1000.0
        );
        for &(x, y) in points.iter().take(WARMUP) {
            let _ = sck_pixel(x, y);
        }
        let mut sck = Vec::new();
        let mut sck_colors = Vec::new();
        let mut failures = Vec::new();
        for &(x, y) in &points {
            let began = Instant::now();
            let color = sck_pixel(x, y);
            sck.push(began.elapsed());
            match color {
                Ok(c) => sck_colors.push(Some(c)),
                Err(e) => {
                    failures.push(format!("({x}, {y}): {e:?}"));
                    sck_colors.push(None);
                }
            }
        }
        Latencies(sck).report("SCScreenshotManager");
        println!("  failed samples: {}", failures.len());
        for failure in failures.iter().take(3) {
            println!("    {failure}");
        }

        // Do they read the same thing? (The screen may change between the two passes: a few
        // differences at a clock or a cursor are normal, a systematic offset is not.)
        let mut compared = 0;
        let mut identical = 0;
        let mut worst = 0u32;
        let mut shown = 0;
        for (i, (q, s)) in quartz_colors.iter().zip(&sck_colors).enumerate() {
            if let (Some(q), Some(s)) = (q, s) {
                compared += 1;
                let diff = q.difference(s);
                worst = worst.max(diff);
                if diff == 0 {
                    identical += 1;
                } else if shown < 5 {
                    shown += 1;
                    println!(
                        "  differs at {:?}: quartz {q} vs sck {s} (sum of channel differences {diff})",
                        points[i]
                    );
                }
            }
        }
        println!(
            "compared {compared}: {identical} identical, worst channel-sum difference {worst}"
        );

        // Is that disagreement the samplers', or just the screen changing between passes? Read
        // the same points with Quartz once more and compare it with its own first pass.
        let again: Vec<Option<Color>> =
            points.iter().map(|&(x, y)| get_pixel_color(x, y)).collect();
        let stable = quartz_colors
            .iter()
            .zip(&again)
            .filter(|(a, b)| a == b)
            .count();
        println!(
            "quartz against itself: {stable} of {} identical",
            points.len()
        );

        // What a `wait_until` actually pays: one sample, then ~100 ms of idling, repeatedly. A
        // capture pipeline that sleeps between uses can be slower on the first sample of each.
        let (px, py) = points[points.len() / 2];
        let polled = |sample: &dyn Fn() -> bool| -> Latencies {
            let mut taken = Vec::new();
            for _ in 0..POLLED {
                let began = Instant::now();
                let _ok = sample();
                taken.push(began.elapsed());
                std::thread::sleep(POLL_INTERVAL);
            }
            Latencies(taken)
        };
        println!("-- polled every {} ms --", POLL_INTERVAL.as_millis());
        polled(&|| get_pixel_color(px, py).is_some()).report("CGWindowListCreateImage");
        polled(&|| sck_pixel(px, py).is_ok()).report("SCScreenshotManager");
    }
}
