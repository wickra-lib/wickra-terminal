"""Parity guard: the Python binding exposes the full public surface of the
terminal (Terminal + command + version + __version__), so an export dropped in a
refactor fails loudly here (mirrors the completeness check in the main wickra
repo)."""

import wickra_terminal as wt


def test_module_exports():
    assert hasattr(wt, "Terminal")
    assert isinstance(wt.__version__, str)


def test_terminal_surface_complete():
    for name in ["command", "version"]:
        assert callable(getattr(wt.Terminal, name, None)), f"Terminal is missing {name}"
