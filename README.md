# namexif

Rename photos according to their EXIF date tag.

Dates come from `DateTimeOriginal`, including `OffsetTimeOriginal` and
`SubSecTimeOriginal` when present. `--timezone` (or `NAMEXIF_TIMEZONE`) selects
the time zone used in filenames, defaulting to the system time zone. Dates
without an EXIF offset, or with a blank offset, are interpreted in that zone.
If such a date falls in a skipped or repeated hour during a clock change, the
photo is left unchanged and an error is reported. An explicit EXIF offset
determines the instant even during these transitions.

Use `--format '%Y%m%dT%H%M%S%.f%z'` to include fractional seconds and distinguish
photos taken within the same second. If multiple photos still produce the same
filename, the batch is rejected before renaming any files. `--dry-run` previews
the proposed names. The format must produce a filename, without directory
components; formats such as `%D` (which contains slashes) are rejected.

Directories are scanned without recursion. Special files such as named pipes
and sockets are skipped; errors accessing files are reported.
If a rename would break a symbolic link found in the scanned directory, the
entire batch is rejected, including in dry-run mode. Link chains and links with
unsupported extensions are checked too. Links outside the scanned directory
are not checked.
Case aliases are checked on case-insensitive filesystems. On Unix, if an alias
cannot be distinguished from several hard links in the same directory, the
batch is conservatively rejected when any candidate is being renamed.

## Screenshot

![Screenshot](assets/screenshot.png)

## Installation

You may install `namexif` locally by running

```console
$ cargo install --git https://github.com/vivienm/namexif.git
```

## Tests

Run `cargo test`. To require the case-insensitive filesystem regressions to run,
set `NAMEXIF_CASE_INSENSITIVE_DIR` to a writable directory on such a filesystem.
CI runs these tests on an ext4 directory with casefold enabled.
