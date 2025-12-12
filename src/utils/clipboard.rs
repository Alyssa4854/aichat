use anyhow::Context;

#[cfg(not(any(target_os = "android", target_os = "emscripten")))]
mod internal {
    use arboard::Clipboard;
    use std::sync::{LazyLock, Mutex};

    static CLIPBOARD: LazyLock<Mutex<Option<Clipboard>>> =
        LazyLock::new(|| Mutex::new(Clipboard::new().ok()));

    pub fn set_text(text: &str) -> anyhow::Result<()> {
        let mut clipboard = CLIPBOARD.lock().unwrap();
        match clipboard.as_mut() {
            Some(clipboard) => {
                match clipboard.set_text(text) {
                    Ok(()) => {
                        #[cfg(target_os = "linux")]
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        Ok(())
                    }
                    Err(_) => set_text_osc52(text),
                }
            }
            None => set_text_osc52(text),
        }
    }

    /// Attempts to set text to clipboard using OSC52.
    /// First tries the `osc` command with $OSC_TTY device, then falls back to /dev/tty.
    fn set_text_osc52(text: &str) -> anyhow::Result<()> {
        use std::process::{Command, Stdio};

        // Try using `osc copy` with OSC_TTY env var (if set)
        if let Ok(tty_device) = std::env::var("OSC_TTY") {
            let result = Command::new("osc")
                .args(["copy", "--device", &tty_device])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(stdin) = child.stdin.as_mut() {
                        stdin.write_all(text.as_bytes())?;
                    }
                    child.wait()
                });

            if result.is_ok() {
                return Ok(());
            }
        }

        // Fallback: write OSC52 directly to /dev/tty (or stdout)
        use std::fs::OpenOptions;
        use std::io::Write;

        let tty = OpenOptions::new().write(true).open("/dev/tty");
        let mut output: Box<dyn Write> = match tty {
            Ok(f) => Box::new(f),
            Err(_) => Box::new(std::io::stdout()),
        };

        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let encoded = STANDARD.encode(text);
        let seq = format!("\x1b]52;c;{encoded}\x07");
        output.write_all(seq.as_bytes())?;
        output.flush()?;
        Ok(())
    }
}

#[cfg(any(target_os = "android", target_os = "emscripten"))]
mod internal {
    pub fn set_text(_text: &str) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("No clipboard available"))
    }
}

pub fn set_text(text: &str) -> anyhow::Result<()> {
    internal::set_text(text).context("Failed to copy")
}
