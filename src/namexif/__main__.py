"""Entry point."""

import click
from dateutil.tz import gettz

from .__about__ import __version__
from .app import App


class TimeZoneParamType(click.ParamType):
    """Timezone parameter type."""

    name = "timezone"

    def convert(self, value, param, ctx):
        """Parse string parameter."""
        timezone = gettz(value)
        if timezone is None:
            self.fail("{!r} is not a valid timezone".format(value), param, ctx)
        else:
            return timezone


@click.command(context_settings={"help_option_names": ["-h", "--help"]})
@click.version_option(__version__)
@click.option(
    "-c",
    "--config",
    type=click.Path(exists=True, dir_okay=False),
    help="Configuration file.",
)
@click.option("-j", "--jobs", type=int, help="Number of concurrent jobs.")
@click.option(
    "-q", "--quiet", is_flag=True, help="Do not display information messages."
)
@click.option(
    "-y", "--yes", is_flag=True, help="Do not prompt for confirmation."
)
@click.option(
    "-n", "--dry-run", is_flag=True, help="Do not actually rename files."
)
@click.argument("target")
@click.argument("timezone", type=TimeZoneParamType())
def main(config, jobs, quiet, yes, dry_run, target, timezone):
    """Rename image files according to their metadata."""
    app = App(config, quiet)
    with app:
        app.rename(target, timezone, dry_run, not yes, jobs)


if __name__ == "__main__":
    main()
