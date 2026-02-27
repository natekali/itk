//! Clipboard watcher daemon for ITK.
//!
//! Monitors the clipboard for changes and automatically cleans/frames
//! developer content (stack traces, JSON, YAML, etc.).
//!
//! Usage: `itk watch` -- runs until Ctrl+C.

use crate::cleaners;
use crate::clipboard;
use crate::db;
use crate::detect;
use crate::frame;
use crate::style;
use crate::undo;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Minimum content length to consider for optimization.
const MIN_CONTENT_LEN: usize = 200;

/// Polling interval in milliseconds.
const POLL_INTERVAL_MS: u64 = 250;

/// Run the clipboard watcher loop.
pub fn run() {
    eprintln!("{} {} press {} to stop",
        style::dim("itk:"),
        style::header("watching clipboard..."),
        style::info("Ctrl+C")
    );
    eprintln!();

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    setup_ctrlc(running.clone());

    let mut last_hash: u64 = 0;
    let mut runs: u32 = 0;
    let mut total_saved: i64 = 0;

    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));

        // Read clipboard (skip errors silently -- clipboard may be locked)
        let content = match clipboard::read() {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Hash to detect changes
        let hash = hash_string(&content);
        if hash == last_hash {
            continue;
        }
        last_hash = hash;

        // Skip short content
        if content.len() < MIN_CONTENT_LEN {
            continue;
        }

        // Detect content type
        let ct = detect::detect(&content, false, None);

        // Skip plain text -- nothing to optimize
        if ct == detect::ContentType::PlainText {
            eprintln!("  {} {} {}", style::dim("itk:"), style::info("[text]"), style::dim("skipped (plain text)"));
            continue;
        }

        // Save for undo before modifying
        undo::save(&content);

        // Clean
        let opts = cleaners::CleanOptions {
            aggressive: false,
            _diff_mode: false,
            content_type: ct.clone(),
        };
        let cleaned = cleaners::clean(&content, &opts);

        // Frame
        let fc = frame::build_frame(&cleaned, &ct);
        let framed = frame::render_framed(&cleaned, &fc, None);

        // Estimate tokens (simple word count for watch mode -- fast)
        let original_words = content.split_whitespace().count() as u64;
        let cleaned_words = cleaned.split_whitespace().count() as u64;
        let saved = original_words as i64 - cleaned_words as i64;

        let type_label = ct.label();

        // Record in database (best-effort)
        if let Ok(mut conn) = db::open() {
            let _ = db::record_run(&mut conn, &type_label, original_words, cleaned_words);
        }

        // If no savings (or content grew), skip modification
        if saved <= 0 {
            eprintln!("  {} {} {}",
                style::dim("itk:"),
                style::info(&format!("[{}]", type_label)),
                style::dim("no savings, clipboard unchanged")
            );
            continue;
        }

        // Write cleaned content to clipboard
        match clipboard::write(&framed) {
            Ok(()) => {
                runs += 1;
                total_saved += saved;

                let pct = if original_words > 0 {
                    saved * 100 / original_words as i64
                } else {
                    0
                };

                let savings_str = format!("-{}%", pct);
                eprintln!("  {} {} {} -> {} tokens ({}) {}",
                    style::dim("itk:"),
                    style::info(&format!("[{}]", type_label)),
                    original_words,
                    cleaned_words,
                    style::savings_colored(&savings_str, true),
                    style::success("v")
                );

                // Update hash to the new (cleaned) content so we don't re-process
                last_hash = hash_string(&framed);
            }
            Err(_) => {
                eprintln!("  {} {} {}",
                    style::dim("itk:"),
                    style::info(&format!("[{}]", type_label)),
                    style::dim("clipboard write failed")
                );
            }
        }
    }

    // Session summary on exit
    eprintln!();
    if runs > 0 {
        eprintln!("{} {} {} runs, ~{} tokens saved",
            style::dim("itk:"),
            style::header("session summary:"),
            runs,
            style::savings_colored(&format!("{}", total_saved), true)
        );
    } else {
        eprintln!("{} {} no content was optimized",
            style::dim("itk:"),
            style::header("session summary:")
        );
    }
}

fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Set up Ctrl+C handling using platform-native APIs.
fn setup_ctrlc(running: Arc<AtomicBool>) {
    #[cfg(windows)]
    {
        // Windows: SetConsoleCtrlHandler
        use std::sync::atomic::AtomicBool;
        static STOP: AtomicBool = AtomicBool::new(false);

        // Store in static for the handler
        // We use a separate static since we can't capture in extern fn
        let r = running;
        thread::spawn(move || {
            while !STOP.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));
            }
            r.store(false, Ordering::Relaxed);
        });

        unsafe {
            extern "system" {
                fn SetConsoleCtrlHandler(
                    handler: Option<unsafe extern "system" fn(u32) -> i32>,
                    add: i32,
                ) -> i32;
            }

            unsafe extern "system" fn handler(_ctrl_type: u32) -> i32 {
                STOP.store(true, Ordering::Relaxed);
                1 // Handled
            }

            SetConsoleCtrlHandler(Some(handler), 1);
        }
    }

    #[cfg(unix)]
    {
        // Unix: simple approach -- spawn a thread that checks for termination
        // We use the process signal handler via std
        let r = running;
        thread::spawn(move || {
            // Use a pipe-based signal handler approach
            // For simplicity, just sleep and check -- the OS will deliver SIGINT
            // and the default handler will set a flag
            loop {
                thread::sleep(Duration::from_millis(100));
                if !r.load(Ordering::Relaxed) {
                    break;
                }
            }
        });

        // Install a simple SIGINT handler using unsafe libc
        unsafe {
            // Use function pointer for SIGINT
            static mut RUNNING_FLAG: *const AtomicBool = std::ptr::null();
            RUNNING_FLAG = Arc::into_raw(running);

            extern "C" fn sigint_handler(_: i32) {
                unsafe {
                    if !RUNNING_FLAG.is_null() {
                        (*RUNNING_FLAG).store(false, Ordering::Relaxed);
                    }
                }
            }

            // libc::signal equivalent using raw syscall
            // signal(SIGINT, handler)
            type SigHandler = extern "C" fn(i32);
            extern "C" {
                fn signal(signum: i32, handler: SigHandler) -> usize;
            }
            signal(2, sigint_handler); // SIGINT = 2
        }
    }
}
