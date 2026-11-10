//! Manual probe: what does `get_foreground_window` report, versus the windows the OS lists?
//!
//! `cargo run -p tap-platform --example foreground_probe`

use tap_platform::{get_foreground_window, list_windows};

fn main() {
    println!("foreground: {:?}", get_foreground_window());
    println!("on-screen windows (front to back):");
    for w in list_windows().iter().take(10) {
        println!(
            "  pid {:>6}  {:<22} {:>5}x{:<5} title={:?}",
            w.pid, w.process_name, w.rect.width, w.rect.height, w.title
        );
    }
}
