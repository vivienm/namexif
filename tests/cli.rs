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
        .env("NAMEXIF_FORMAT", FORMAT)
        .env_remove("NAMEXIF_TIMEZONE")
        .env_remove("NAMEXIF_LOG_LEVEL")
        .env("TZ", "UTC")
        .env("LS_COLORS", "")
        .arg("--assume-yes");
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

#[test]
fn invalid_formats_report_errors_without_panicking_or_renaming() {
    for format in ["%", "%J"] {
        for dry_run in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("input.tif");
            let bytes = image(&source, &[(Tag::DateTimeOriginal, DATE)]);
            let mut cmd = command();
            cmd.args(["--timezone", "UTC", "--format", format])
                .arg(&source);
            if dry_run {
                cmd.arg("--dry-run");
            }
            let output = cmd.output().unwrap();
            assert_exit(&output, 1);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("Invalid filename format"), "{stderr}");
            assert!(!stderr.contains("panicked"), "{stderr}");
            assert_eq!(fs::read(source).unwrap(), bytes);
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }
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

#[test]
fn exif_offsets_are_converted_to_the_output_timezone() {
    for (date, offset, timezone, target) in [
        (DATE, "+02:00", "UTC", "20260601T103456+0000.tiff"),
        (DATE, "-03:30", "UTC", "20260601T160456+0000.tiff"),
        (DATE, "+02:00", "Europe/Paris", "20260601T123456+0200.tiff"),
        (DATE, "+00:00", "Europe/Paris", "20260601T143456+0200.tiff"),
        (
            "2026:06:01 00:30:00",
            "+02:00",
            "UTC",
            "20260531T223000+0000.tiff",
        ),
        // The offset disambiguates the repeated hour at the end of DST.
        (
            "2026:10:25 02:30:00",
            "+01:00",
            "Europe/Paris",
            "20261025T023000+0100.tiff",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("input.tif");
        let bytes = image(
            &source,
            &[
                (Tag::DateTimeOriginal, date),
                (Tag::OffsetTimeOriginal, offset),
            ],
        );
        let output = command()
            .args(["--timezone", timezone])
            .arg(&source)
            .output()
            .unwrap();
        assert_exit(&output, 0);
        assert!(!source.exists());
        assert_eq!(fs::read(dir.path().join(target)).unwrap(), bytes);
    }
}

#[cfg(unix)]
#[test]
fn exif_offset_is_respected_with_the_default_output_timezone() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("input.tif");
    image(
        &source,
        &[
            (Tag::DateTimeOriginal, DATE),
            (Tag::OffsetTimeOriginal, "+02:00"),
        ],
    );
    // command() supplies TZ=UTC without setting --timezone or NAMEXIF_TIMEZONE.
    let output = command().arg(&source).output().unwrap();
    assert_exit(&output, 0);
    assert!(dir.path().join("20260601T103456+0000.tiff").exists());
}

#[test]
fn timezone_environment_variable_selects_the_output_timezone() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("input.tif");
    image(
        &source,
        &[
            (Tag::DateTimeOriginal, DATE),
            (Tag::OffsetTimeOriginal, "+02:00"),
        ],
    );
    let output = command()
        .env("NAMEXIF_TIMEZONE", "Asia/Kolkata")
        .arg(&source)
        .output()
        .unwrap();
    assert_exit(&output, 0);
    assert!(dir.path().join("20260601T160456+0530.tiff").exists());
}

#[test]
fn missing_and_blank_offsets_use_the_requested_timezone() {
    for offset in [None, Some("   :  "), Some("      ")] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("input.tif");
        let mut tags = vec![(Tag::DateTimeOriginal, DATE)];
        if let Some(offset) = offset {
            tags.push((Tag::OffsetTimeOriginal, offset));
        }
        image(&source, &tags);
        let output = command()
            .args(["--timezone", "Europe/Paris"])
            .arg(&source)
            .output()
            .unwrap();
        assert_exit(&output, 0);
        assert!(dir.path().join("20260601T123456+0200.tiff").exists());
    }
}

#[test]
fn fractional_seconds_keep_their_precision_during_timezone_conversion() {
    for (subsec, fraction) in [
        (None, ""),
        (Some("   "), ""),
        (Some("1"), ".1"),
        (Some("001"), ".001"),
        (Some("123456789"), ".123456789"),
        (Some("1234567899"), ".123456789"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("input.tif");
        let mut tags = vec![
            (Tag::DateTimeOriginal, DATE),
            (Tag::OffsetTimeOriginal, "+02:00"),
        ];
        if let Some(subsec) = subsec {
            tags.push((Tag::SubSecTimeOriginal, subsec));
        }
        image(&source, &tags);
        let output = command()
            .args(["--timezone", "UTC", "--format", "%Y%m%dT%H%M%S%.f%z"])
            .arg(&source)
            .output()
            .unwrap();
        assert_exit(&output, 0);
        assert!(
            dir.path()
                .join(format!("20260601T103456{fraction}+0000.tiff"))
                .exists()
        );
    }
}

#[test]
fn fractional_format_distinguishes_photos_taken_in_the_same_second() {
    let dir = tempfile::tempdir().unwrap();
    for (name, subsec) in [("a.tif", "123"), ("b.tif", "456")] {
        image(
            &dir.path().join(name),
            &[
                (Tag::DateTimeOriginal, DATE),
                (Tag::SubSecTimeOriginal, subsec),
            ],
        );
    }
    let output = command()
        .args(["--timezone", "UTC", "--format", "%Y%m%dT%H%M%S.%f%z"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_exit(&output, 0);
    assert!(dir.path().join("20260601T123456.123+0000.tiff").exists());
    assert!(dir.path().join("20260601T123456.456+0000.tiff").exists());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn malformed_optional_date_tags_are_reported_without_renaming() {
    for (tag, value) in [
        (Tag::OffsetTimeOriginal, "oops"),
        (Tag::OffsetTimeOriginal, "+02:60"),
        (Tag::OffsetTimeOriginal, "+24:00"),
        (Tag::OffsetTimeOriginal, "+02:00extra"),
        (Tag::SubSecTimeOriginal, "oops"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("input.tif");
        let bytes = image(&source, &[(Tag::DateTimeOriginal, DATE), (tag, value)]);
        let output = command()
            .args(["--timezone", "UTC"])
            .arg(&source)
            .output()
            .unwrap();
        assert_exit(&output, 1);
        assert_eq!(fs::read(source).unwrap(), bytes);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
