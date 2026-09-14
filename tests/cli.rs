use std::{
    fs,
    io::Cursor,
    path::Path,
    process::{Command, Output},
};

use exif::{Field, In, Tag, Value};

const DATE: &str = "2026:06:01 12:34:56";
const FORMAT: &str = "%Y%m%dT%H%M%S%z";
const TARGET: &str = "20260601T123456+0000.tiff";

fn image(path: &Path, tags: &[(Tag, &str)]) -> Vec<u8> {
    let fields: Vec<_> = tags
        .iter()
        .map(|(tag, value)| Field {
            tag: *tag,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![value.as_bytes().to_vec()]),
        })
        .collect();
    let mut writer = exif::experimental::Writer::new();
    for field in &fields {
        writer.push_field(field);
    }
    let mut buffer = Cursor::new(Vec::new());
    writer.write(&mut buffer, false).unwrap();
    let bytes = buffer.into_inner();
    fs::write(path, &bytes).unwrap();
    bytes
}

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_namexif"));
    command
        .env_remove("NAMEXIF_FORMAT")
        .env_remove("NAMEXIF_TIMEZONE")
        .env_remove("NAMEXIF_LOG_LEVEL")
        .env("TZ", "UTC")
        .env("LS_COLORS", "")
        .args(["--assume-yes", "--format", FORMAT]);
    command
}

fn assert_exit(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn renames_a_photo_and_preserves_its_contents() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("input.tif");
    let bytes = image(&source, &[(Tag::DateTimeOriginal, DATE)]);
    let output = command()
        .args(["--timezone", "UTC"])
        .arg(&source)
        .output()
        .unwrap();
    assert_exit(&output, 0);
    assert!(!source.exists());
    assert_eq!(fs::read(dir.path().join(TARGET)).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn source_symlink_cannot_replace_the_photo_it_points_to() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join(TARGET);
    let bytes = image(&target, &[(Tag::DateTimeOriginal, DATE)]);
    let source = dir.path().join("input.tif");
    std::os::unix::fs::symlink(TARGET, &source).unwrap();
    let output = command()
        .args(["--timezone", "UTC"])
        .arg(&source)
        .output()
        .unwrap();
    assert_exit(&output, 2);
    assert_eq!(fs::read_link(&source).unwrap(), Path::new(TARGET));
    assert!(!target.is_symlink());
    assert_eq!(fs::read(target).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn dangling_target_symlink_is_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("input.tif");
    let bytes = image(&source, &[(Tag::DateTimeOriginal, DATE)]);
    let target = dir.path().join(TARGET);
    std::os::unix::fs::symlink("missing.tiff", &target).unwrap();
    let output = command()
        .args(["--timezone", "UTC"])
        .arg(&source)
        .output()
        .unwrap();
    assert_exit(&output, 2);
    assert_eq!(fs::read_link(target).unwrap(), Path::new("missing.tiff"));
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn existing_hard_link_is_reported_as_a_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("input.tif");
    let bytes = image(&source, &[(Tag::DateTimeOriginal, DATE)]);
    let target = dir.path().join(TARGET);
    fs::hard_link(&source, &target).unwrap();
    let output = command()
        .args(["--timezone", "UTC"])
        .arg(&source)
        .output()
        .unwrap();
    assert_exit(&output, 2);
    assert_eq!(fs::read(source).unwrap(), bytes);
    assert_eq!(fs::read(target).unwrap(), bytes);
}
