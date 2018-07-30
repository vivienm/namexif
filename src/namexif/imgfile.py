"""Image module."""

from datetime import datetime
from subprocess import check_output


class ImageFile:
    """An image file."""

    _EXIV_DATETIME_KEY = "Exif.Photo.DateTimeOriginal"
    _EXIV_DATETIME_FMT = "%Y:%m:%d %H:%M:%S"

    def __init__(self, filepath):
        self._filepath = filepath

    @property
    def filepath(self):
        """Path of the image file."""
        return self._filepath

    def datetime(self, tzinfo=None):
        """Original creation date of the image file."""
        command = [
            "exiv2",
            "-K",
            self._EXIV_DATETIME_KEY,
            "-pt",
            self.filepath,
        ]
        output = check_output(command, env={"LANG": "C"}).decode()
        tokens = output.rstrip().split(None, 3)
        if tokens[0] != self._EXIV_DATETIME_KEY:
            raise RuntimeError("Unexpected exiv2 output: {!r}".format(output))
        exif_datetime = datetime.strptime(tokens[-1], self._EXIV_DATETIME_FMT)
        if tzinfo:
            exif_datetime = exif_datetime.replace(tzinfo=tzinfo)
        return exif_datetime
