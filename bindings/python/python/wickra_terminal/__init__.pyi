"""Type stubs for the wickra-terminal Python binding."""

__version__: str

class Terminal:
    """A trading terminal instance driven by JSON commands.

    The handle is bound to the thread that created it: the terminal owns
    non-``Send`` feed sources, so the pyclass is declared ``unsendable``.
    """

    def __init__(self, config_json: str) -> None:
        """Build a terminal from a JSON config string.

        Raises:
            ValueError: if the config is not valid JSON or not a valid config.
        """
        ...

    def command(self, cmd_json: str) -> str:
        """Apply a command JSON and return the resulting frame JSON.

        Raises:
            ValueError: if the command is not valid JSON or not a known command.
        """
        ...

    @staticmethod
    def version() -> str:
        """The library version."""
        ...
