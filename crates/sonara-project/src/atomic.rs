//! Crash-safe file primitives (`sonara.persistence.atomic`).
//!
//! A write goes to a temporary file next to its target (`.<name>.<random>.tmp`), is flushed to
//! disk, and is renamed over the target, so a reader sees either the old or the new content.
//! [`tempfile`] provides the temporary file and the replacing rename (QG-13): it creates the file
//! with mode `0600` like Python's `mkstemp`, removes it when the write fails, and its `persist`
//! replaces an existing target on every platform.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

/// Read size for hashing files.
const HASH_CHUNK: usize = 1 << 20;

/// Persists a directory entry change, such as a rename into `path`.
///
/// Errors are ignored, as in the prototype: not every file system can sync a directory. A no-op
/// on Windows.
pub fn fsync_dir(path: &Path) {
    if cfg!(windows) {
        return;
    }
    if let Ok(dir) = File::open(path) {
        // ERR-06: best effort by design (see above); the data itself was already synced.
        let _ = dir.sync_all();
    }
}

/// A temporary file in `path`'s directory, named `.<file name>.<random>.tmp`.
fn temp_beside(path: &Path) -> io::Result<NamedTempFile> {
    let parent = parent_dir(path);
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    tempfile::Builder::new()
        .prefix(&format!(".{name}."))
        .suffix(".tmp")
        .tempfile_in(parent)
}

/// The directory that holds `path` (`.` for a bare file name).
fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Writes `data` to `path` so that readers see either the old or the new content, creating
/// the parent directories.
///
/// # Errors
///
/// The I/O error of creating, writing, syncing or renaming the temporary file; the temporary
/// file is removed then.
pub fn atomic_write_bytes(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut tmp = temp_beside(path)?;
    tmp.write_all(data)?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|err| err.error)?;
    fsync_dir(parent_dir(path));
    Ok(())
}

/// Copies `src` to `dst` through a temporary file and a rename. Only the content is copied,
/// like Python's `shutil.copyfile`: the copy keeps the temporary file's mode.
///
/// # Errors
///
/// The I/O error of reading `src` or of writing, syncing or renaming the copy.
pub fn atomic_copy(src: &Path, dst: &Path) -> io::Result<()> {
    let mut tmp = temp_beside(dst)?;
    let mut source = File::open(src)?;
    io::copy(&mut source, tmp.as_file_mut())?;
    tmp.as_file().sync_all()?;
    tmp.persist(dst).map_err(|err| err.error)?;
    Ok(())
}

/// Hard-links `src` to `dst` when possible (same volume, no extra space), else copies it
/// atomically. Does nothing when `dst` exists. Media files are immutable once written, so
/// sharing an inode is safe.
///
/// # Errors
///
/// The I/O error of creating `dst`'s directory or of the fallback copy.
pub fn link_or_copy(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(parent_dir(dst))?;
    if dst.exists() {
        return Ok(());
    }
    if fs::hard_link(src, dst).is_err() {
        atomic_copy(src, dst)?;
    }
    Ok(())
}

/// Lowercase hex SHA-256 of the file at `path`.
///
/// # Errors
///
/// The I/O error of opening or reading the file.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_CHUNK];
    loop {
        let read = file.read(&mut buffer)?;
        let Some(chunk) = buffer.get(..read).filter(|chunk| !chunk.is_empty()) else {
            break;
        };
        hasher.update(chunk);
    }
    Ok(hex(&hasher.finalize()))
}

/// Lowercase hex SHA-256 of `data`.
#[must_use]
pub fn sha256_bytes(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// Lowercase hex digits of `bytes`.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{atomic_copy, atomic_write_bytes, link_or_copy, sha256_bytes, sha256_file};

    #[test]
    fn sha256_matches_known_values() {
        assert_eq!(
            sha256_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("big.bin");
        // Larger than one hash chunk, so the loop runs more than once.
        let data: Vec<u8> = (0..(3_u32 << 20) + 17)
            .map(|i| u8::try_from(i % 251).expect("below 251"))
            .collect();
        fs::write(&path, &data).expect("write");
        assert_eq!(sha256_file(&path).expect("hash"), sha256_bytes(&data));
    }

    #[test]
    fn atomic_write_replaces_and_leaves_no_temporary_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("sub").join("file.json");
        atomic_write_bytes(&path, b"old").expect("first write");
        atomic_write_bytes(&path, b"new").expect("second write");
        assert_eq!(fs::read(&path).expect("read"), b"new");
        let names: Vec<String> = fs::read_dir(path.parent().expect("parent"))
            .expect("list")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["file.json"]);
    }

    #[test]
    fn link_or_copy_keeps_an_existing_target() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src = dir.path().join("a.wav");
        fs::write(&src, b"RIFF").expect("write");
        let dst = dir.path().join("media").join("a.wav");
        link_or_copy(&src, &dst).expect("link");
        assert_eq!(fs::read(&dst).expect("read"), b"RIFF");
        fs::write(dir.path().join("b.wav"), b"other").expect("write");
        link_or_copy(&dir.path().join("b.wav"), &dst).expect("no-op");
        assert_eq!(fs::read(&dst).expect("read"), b"RIFF");
        let copy = dir.path().join("copy").join("a.wav");
        atomic_copy(&src, &copy).expect("copy");
        assert_eq!(fs::read(&copy).expect("read"), b"RIFF");
        assert!(atomic_copy(&dir.path().join("missing.wav"), &copy).is_err());
    }
}
