"""Parity guard: the Python binding exposes the full public surface of the
terminal (Terminal + command + version + __version__), so an export dropped in a
refactor fails loudly here (mirrors the completeness check in the main wickra
repo)."""

import ast
from pathlib import Path

import wickra_terminal as wt


def test_module_exports():
    assert hasattr(wt, "Terminal")
    assert isinstance(wt.__version__, str)


def test_terminal_surface_complete():
    for name in ["command", "version"]:
        assert callable(getattr(wt.Terminal, name, None)), f"Terminal is missing {name}"


def test_typing_marker_and_stub_ship_with_the_package():
    """PEP 561: a type checker only reads the stub if `py.typed` is installed
    alongside it. Both files live under maturin's `python-source`, so they ship
    with the wheel -- but nothing else would notice if a packaging change dropped
    them, and the failure is silent: type checking degrades to `Any` rather than
    erroring."""
    package = Path(wt.__file__).parent
    assert (package / "py.typed").is_file(), "PEP 561 marker missing from the installed package"
    assert (package / "__init__.pyi").is_file(), "type stub missing from the installed package"


def test_stub_declares_the_whole_runtime_surface():
    """The stub is hand-written, so it can drift from the pyclass. Every public
    callable on Terminal must appear in it."""
    stub = ast.parse((Path(wt.__file__).parent / "__init__.pyi").read_text(encoding="utf-8"))
    declared = {
        method.name
        for node in stub.body
        if isinstance(node, ast.ClassDef) and node.name == "Terminal"
        for method in node.body
        if isinstance(method, ast.FunctionDef)
    }
    runtime = {n for n in dir(wt.Terminal) if not n.startswith("_") and callable(getattr(wt.Terminal, n))}
    assert runtime <= declared, f"stub is missing {sorted(runtime - declared)}"

