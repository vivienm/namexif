"""Application module."""

import json
import os
from collections import namedtuple
from contextlib import ExitStack
from multiprocessing.dummy import Pool as ThreadPool
from os.path import basename, dirname, exists, isfile, join, splitext
from shlex import quote as shquote

import click

from .__about__ import __title__
from .imgfile import ImageFile


class Config(
    namedtuple("Config", "extension_map extension_ci filename_format")
):
    """Configuration class."""

    def __new__(
        cls,
        extension_map=None,
        extension_ci=True,
        filename_format="%Y-%m-%dT%H:%M:%S%z",
    ):
        """Create a new configuration object with default values."""
        if not extension_map:
            extension_map = {".jpg": ".jpg", ".jpeg": ".jpg"}
        return super().__new__(
            cls,
            extension_map=extension_map,
            extension_ci=extension_ci,
            filename_format=filename_format,
        )

    @classmethod
    def from_json(cls, data):
        """Load a configuration object from a JSON array."""
        return cls(**data)


class App:
    """Application class."""

    name = __title__

    def __init__(self, config_filepath=None, quiet=False):
        self.config_filepath = config_filepath
        self.config = None
        self.quiet = quiet

    def _load_config(self):
        """Load the configuration."""
        if self.config_filepath is None:
            config_filepath = join(click.get_app_dir(self.name), "config.json")
            if exists(config_filepath):
                self.config_filepath = config_filepath
        if self.config_filepath is not None:
            with open(self.config_filepath) as config_fileobj:
                self.config = Config.from_json(json.load(config_fileobj))
        else:
            self.config = Config()

    def __enter__(self):
        self._load_config()
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        pass

    def echo(self, *args, **kwargs):
        """Display an information message."""
        if not self.quiet:
            click.echo(*args, **kwargs)

    def _renamed(self, filepath, tzinfo, bar):
        """Return the target name of *filepath*."""
        _, fileext = splitext(filepath)
        if self.config.extension_ci:
            fileext = fileext.lower()
        try:
            fileext = self.config.extension_map[fileext]
        except KeyError:
            self.echo(
                "Skip file {}: unknown extension".format(shquote(filepath)),
                err=True,
            )
            return
        img_datetime = ImageFile(filepath).datetime(tzinfo)
        filestem = img_datetime.strftime(self.config.filename_format)
        filename = filestem + fileext
        filepath = join(dirname(filepath), filename)
        if bar is not None:
            bar.update(1)
        return filepath

    def _iter_renamings(self, target, tzinfo, num_threads):
        """Yield path pairs (old, new) for files in *target*."""
        if isfile(target):
            src_filepaths = [target]
            dst_filepaths = [self._renamed(target)]
        else:
            src_filepaths = [
                join(target, src_filename)
                for src_filename in os.listdir(target)
            ]
            pool = ThreadPool(num_threads)
            with ExitStack() as stack:
                if self.quiet:
                    bar = None
                else:
                    bar = stack.enter_context(
                        click.progressbar(
                            length=len(src_filepaths),
                            label="Processing pictures",
                        )
                    )
                args = [
                    (src_filepath, tzinfo, bar)
                    for src_filepath in src_filepaths
                ]
                dst_filepaths = pool.starmap(self._renamed, args)
            pool.close()
            pool.join()
        for src_filepath, dst_filepath in zip(src_filepaths, dst_filepaths):
            if dst_filepath is None:
                continue
            if src_filepath != dst_filepath:
                yield src_filepath, dst_filepath

    def rename(
        self, target, tzinfo, dry_run=False, confirm=True, num_threads=None
    ):
        """Interactively rename image files in *target*."""
        renamings = list(self._iter_renamings(target, tzinfo, num_threads))
        if len(renamings) == 0:
            self.echo("Nothing to rename!")
            return
        for src_filepath, dst_filepath in renamings:
            src_filename = basename(src_filepath)
            dst_filename = basename(dst_filepath)
            self.echo(
                "{} => {}".format(shquote(src_filename), shquote(dst_filename))
            )
        if dry_run:
            return
        while confirm:
            click.echo("OK? [yn] ", nl=False)
            char = click.getchar().lower()
            click.echo(char)
            if char == "n":
                return
            elif char == "y":
                break
            else:
                click.echo("Invalid input", err=True)
        for src_filepath, dst_filepath in renamings:
            os.rename(src_filepath, dst_filepath)
