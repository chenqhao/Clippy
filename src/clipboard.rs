//! Clipboard abstraction layer.
//!
//! This module defines the traits and types for reading and writing
//! clipboard content, independent of the operating system.

/// The content that can live on a clipboard.
///
/// We use an enum (not just a plain `String`) because in the future
/// we will add variants like `Png(Vec<u8>)` for image data.
/// Starting with an enum now means we won't have to rewrite
/// everything when we add images later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipContent {
    /// UTF-8 text content.
    Text(String),
}

// `#[derive(Error)]` instead of `#[derive(thiserror::Error)]`.
use thiserror::Error;

/// Errors that can occur when reading or writing the clipboard.
///
/// Each variant carries a human-readable message describing what
/// went wrong. The `#[error("...")]` attribute tells `thiserror`
/// what text to produce when this error is printed.
#[derive(Debug, Error)]
pub enum ClipboardError {
    /// The OS denied access to the clipboard, or it was locked.
    #[error("failed to access clipboard: {0}")]
    Access(String),
}

/// An abstraction over a clipboard (OS clipboard, in-memory buffer, etc.).
///
/// In Java, this would be an `interface`. Any type that implements this
/// trait can be used to get and set clipboard content.
pub trait ClipboardBackend {
    /// Read the current contents of the clipboard.
    ///
    /// Returns:
    /// - `Ok(Some(content))` if the clipboard has content.
    /// - `Ok(None)` if the clipboard is empty or has non-text data.
    /// - `Err(ClipboardError)` if reading the clipboard failed.
    fn get(&mut self) -> Result<Option<ClipContent>, ClipboardError>;

    /// Set the contents of the clipboard.
    ///
    /// We borrow `content` (`&ClipContent`) because the caller still owns
    /// the data and might want to keep using it.
    fn set(&mut self, content: &ClipContent) -> Result<(), ClipboardError>;
}

/// An in-memory fake clipboard for testing.
///
/// Instead of calling the operating system, it simply stores the content
/// in an internal `Option<ClipContent>`.
#[derive(Debug, Default, Clone)]
pub struct MemoryClipboard {
    content: Option<ClipContent>,
}

impl MemoryClipboard {
    /// Create a new, empty in-memory clipboard.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ClipboardBackend for MemoryClipboard {
    fn get(&mut self) -> Result<Option<ClipContent>, ClipboardError> {
        // We clone our stored content so the caller gets their own copy,
        // while the clipboard keeps its copy intact.
        Ok(self.content.clone())
    }

    fn set(&mut self, content: &ClipContent) -> Result<(), ClipboardError> {
        // Clone the borrowed `&ClipContent` to store an owned copy inside `self`.
        self.content = Some(content.clone());
        Ok(())
    }
}

/// A clipboard backend that talks to the real operating system clipboard
/// via the `arboard` crate.
pub struct SystemClipboard {
    // `arboard::Clipboard` is the handle to the OS clipboard.
    // We store it as a field so we don't have to re-open the
    // clipboard connection on every get/set call.
    inner: arboard::Clipboard,
}

impl SystemClipboard {
    /// Open a connection to the OS clipboard.
    ///
    /// Returns `Err` if the OS clipboard cannot be accessed
    /// (e.g., no display server on headless Linux).
    pub fn new() -> Result<Self, ClipboardError> {
        let inner = arboard::Clipboard::new()
            // `.map_err(...)` converts arboard's error into our ClipboardError.
            // The `|e|` is a closure (anonymous function) — like a Java lambda.
            .map_err(|e| ClipboardError::Access(e.to_string()))?;
        Ok(Self { inner })
    }
}

impl ClipboardBackend for SystemClipboard {
    fn get(&mut self) -> Result<Option<ClipContent>, ClipboardError> {
        match self.inner.get_text() {
            // The OS returned text — wrap it in our ClipContent enum.
            Ok(text) => Ok(Some(ClipContent::Text(text))),

            // The clipboard is empty, or contains non-text data (e.g., an image).
            // This is NOT an error — it just means "nothing to paste".
            // We map it to Ok(None) instead of returning Err.
            Err(arboard::Error::ContentNotAvailable) => Ok(None),

            // Any other error (clipboard locked, no display server, etc.)
            // is a real failure — convert it to our error type.
            Err(other) => Err(ClipboardError::Access(other.to_string())),
        }
    }

    fn set(&mut self, content: &ClipContent) -> Result<(), ClipboardError> {
        // Destructure the enum to extract the inner string.
        // `match` forces us to handle every variant — if we add
        // `Png(Vec<u8>)` later, the compiler will remind us here.
        match content {
            ClipContent::Text(text) => self
                .inner
                .set_text(text)
                .map_err(|e| ClipboardError::Access(e.to_string())),
        }
    }
}

use std::fs;
use std::io;
use std::path::PathBuf;

pub struct FileClipboard {
    path: PathBuf,
}

impl FileClipboard {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ClipboardBackend for FileClipboard {
    fn get(&mut self) -> Result<Option<ClipContent>, ClipboardError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(Some(ClipContent::Text(text))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(other) => Err(ClipboardError::Access(other.to_string())),
        }
    }

    fn set(&mut self, content: &ClipContent) -> Result<(), ClipboardError> {
        match content {
            ClipContent::Text(text) => {
                fs::write(&self.path, text).map_err(|e| ClipboardError::Access(e.to_string()))
            }
        }
    }
}

/// Create a clipboard backend from a specification string.
///
/// Supported specs:
/// - `None` or `"system"`: [`SystemClipboard`] (real OS clipboard).
/// - `"memory"`: [`MemoryClipboard`] (in-memory, for testing).
/// - `"file:<path>"`: [`FileClipboard`] (file-backed at `<path>`).
pub fn create_backend_from_spec(
    spec: Option<&str>,
) -> Result<Box<dyn ClipboardBackend>, ClipboardError> {
    match spec {
        None | Some("system") => Ok(Box::new(SystemClipboard::new()?)),
        Some("memory") => Ok(Box::new(MemoryClipboard::new())),
        Some(s) if s.starts_with("file:") => {
            let path = PathBuf::from(&s["file:".len()..]);
            Ok(Box::new(FileClipboard::new(path)))
        }
        Some(unknown) => Err(ClipboardError::Access(format!(
            "unknown clipboard backend: {unknown} (expected 'system', 'memory', or 'file:<path>')"
        ))),
    }
}

/// Create a clipboard backend based on the `UCLIP_BACKEND` environment variable.
///
/// Defaults to [`SystemClipboard`] if the environment variable is not set.
pub fn create_backend() -> Result<Box<dyn ClipboardBackend>, ClipboardError> {
    let spec = std::env::var("UCLIP_BACKEND").ok();
    create_backend_from_spec(spec.as_deref())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_clipboard_starts_empty_and_stores_content() {
        let mut clipboard = MemoryClipboard::new();

        // 1. Starts empty (Ok(None))
        assert_eq!(clipboard.get().unwrap(), None);

        // 2. Set some text
        let clip = ClipContent::Text("hello world".to_string());
        clipboard.set(&clip).unwrap();

        // 3. Read it back (Ok(Some(ClipContent::Text("hello world"))))
        assert_eq!(clipboard.get().unwrap(), Some(clip));
    }

    #[test]
    fn file_clipboard_empty_and_stores_content() {
        let dir = tempfile::tempdir().unwrap();
        let test_path = dir.path().join("clipboard.txt");
        let mut clipboard = FileClipboard::new(test_path);

        assert_eq!(clipboard.get().unwrap(), None);

        let clip = ClipContent::Text("hello world".to_string());
        clipboard.set(&clip).unwrap();

        assert_eq!(clipboard.get().unwrap(), Some(clip));
    }

    #[test]
    fn create_backend_from_spec_handles_all_variants() {
        // None and "system" create a SystemClipboard
        for spec in [None, Some("system")] {
            match create_backend_from_spec(spec) {
                Ok(_) => {}
                Err(ClipboardError::Access(msg)) => {
                    assert!(!msg.contains("unknown clipboard backend"));
                }
            }
        }

        // "memory" creates a MemoryClipboard
        let mut mem = create_backend_from_spec(Some("memory")).unwrap();
        assert_eq!(mem.get().unwrap(), None);

        // "file:<path>" creates a FileClipboard
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.txt");
        let spec = format!("file:{}", path.display());
        let mut file_cb = create_backend_from_spec(Some(&spec)).unwrap();
        assert_eq!(file_cb.get().unwrap(), None);

        // Unknown spec returns an error
        assert!(matches!(
            create_backend_from_spec(Some("bogus")),
            Err(ClipboardError::Access(_))
        ));
    }
}
