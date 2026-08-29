"""Audit that no file in the repo contains the pre-migration org slug or
maintainer email. Driven by `repo-metadata.toml` at the repo root.

This is the read-only side of the metadata pipeline. It does not patch any
files — it just fails CI when drift sneaks in. Pair with a future
`--write` mode (auto-fix + signed commit on main) once the migration has
settled.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
METADATA_PATH = REPO_ROOT / "repo-metadata.toml"


def load_metadata() -> dict:
    with METADATA_PATH.open("rb") as f:
        return tomllib.load(f)


def is_allowlisted(rel_path: str, allowlist: list[str]) -> bool:
    norm = rel_path.replace(os.sep, "/")
    for entry in allowlist:
        entry_norm = entry.replace(os.sep, "/")
        if entry_norm.endswith("/"):
            if norm.startswith(entry_norm):
                return True
        else:
            if norm == entry_norm:
                return True
    return False


def tracked_files() -> list[str]:
    """List git-tracked files relative to the repo root."""
    out = subprocess.run(
        ["git", "ls-files"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return [line for line in out.stdout.splitlines() if line]


def scan(forbidden: list[str], allowlist: list[str]) -> list[tuple[str, int, str, str]]:
    """Return a list of (rel_path, line_no, needle, line_text) findings.

    Only git-tracked files are scanned, so local-only ghost-ignored files
    (`.claude/`, drafts) never trigger false positives.
    """
    findings: list[tuple[str, int, str, str]] = []
    for rel_path in tracked_files():
        if is_allowlisted(rel_path, allowlist):
            continue
        abs_path = REPO_ROOT / rel_path
        if not abs_path.is_file():
            continue
        try:
            lines = abs_path.read_text(encoding="utf-8", errors="replace").splitlines()
        except (OSError, UnicodeDecodeError):
            continue
        for lineno, line in enumerate(lines, start=1):
            for needle in forbidden:
                if needle in line:
                    findings.append((rel_path, lineno, needle, line.strip()))
    return findings


# Each declared identity value, and the file that has to carry it. A value in
# repo-metadata.toml that nothing checks is a comment: it can say one thing
# while the README, the issue-template config or the badge link says another,
# and the audit that exists to catch exactly that drift stays green.
DECLARED = [
    ("repo", "homepage", "README.md", "{}"),
    ("repo", "discussions", ".github/ISSUE_TEMPLATE/config.yml", "{}"),
    ("repo", "issues_url", ".github/ISSUE_TEMPLATE/config.yml", None),
    ("repo", "security_url", ".github/ISSUE_TEMPLATE/config.yml", "{}"),
    ("badges", "codecov_repo", "README.md", "codecov.io/gh/{}"),
]


def check_declared(meta: dict) -> list[str]:
    """Verify every declared URL actually appears where it is supposed to."""
    problems: list[str] = []
    for section, key, rel_path, template in DECLARED:
        value = meta.get(section, {}).get(key)
        if value is None or template is None:
            continue
        needle = template.format(value)
        path = REPO_ROOT / rel_path
        if not path.is_file():
            problems.append(f"{rel_path}: declared [{section}].{key} but the file is missing")
            continue
        if needle not in path.read_text(encoding="utf-8", errors="replace"):
            problems.append(f"{rel_path}: does not contain {needle!r} from [{section}].{key}")
    return problems

def check_about(meta: dict) -> list[str]:
    """The GitHub About description, which is the one count surface no test reaches.

    Every other place the indicator count appears is inside the repository and is
    held by a test: the README markers, the citation anchor, the eight language
    floors. The About string lives in GitHub's settings, so it drifted -- it said
    460 while the registry held 455, and nothing noticed until someone read both.

    Compared against the README marker rather than against the registry directly,
    because the marker is already pinned to `DEFAULTS.len()` by
    `docs_examples::the_documented_indicator_count_is_the_real_one`. One truth,
    one hop.

    Network, so it is opt-in: skipped with a notice when the description cannot
    be read, rather than failing a job for an unrelated reason.

    Set GITHUB_TOKEN. An unauthenticated read of this endpoint can come back from
    a CDN cache minutes stale, which shows up as a mismatch against a description
    that was just corrected -- seen while writing this. CI always has one
    (`github.token`); a local run without one may report a value that is no
    longer there.
    """
    readme = (REPO_ROOT / 'README.md').read_text(encoding='utf-8')
    match = re.search(r'<!--indicator-count-->(\d+)<!--/indicator-count-->', readme)
    if match is None:
        return ['README.md: no indicator-count marker to compare the About against']
    expected = match.group(1)

    slug = f"{meta['repo']['org']}/{meta['repo']['name']}"
    request = urllib.request.Request(
        f'https://api.github.com/repos/{slug}',
        headers={'Accept': 'application/vnd.github+json', 'User-Agent': 'wickra-sync-metadata'},
    )
    token = os.environ.get('GITHUB_TOKEN')
    if token:
        request.add_header('Authorization', f'Bearer {token}')
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            description = json.load(response).get('description') or ''
    except (urllib.error.URLError, OSError, json.JSONDecodeError) as err:
        print(f'::notice::could not read the About description ({err}); skipping that check')
        return []

    found = re.search(r'(\d+)\s+Wickra indicators', description)
    if found is None:
        return [f'the About description does not state an indicator count: {description!r}']
    if found.group(1) != expected:
        return [
            f'the About description says {found.group(1)} indicators; the repository says {expected}. '
            f'Fix it with: gh repo edit {slug} --description ...'
        ]
    print(f'sync-metadata: the About description agrees with the repository ({expected}).')
    return []

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="audit-only (default)")
    parser.add_argument(
        "--about",
        action="store_true",
        help="also check the GitHub About description against the README count",
    )
    args = parser.parse_args()


    meta = load_metadata()
    audit = meta.get("audit", {})
    forbidden: list[str] = list(audit.get("forbidden", []))
    allowlist: list[str] = list(audit.get("allowlist", []))

    if not forbidden:
        print("repo-metadata.toml [audit].forbidden is empty — nothing to scan.")
        return 0

    findings = scan(forbidden, allowlist)
    if findings:
        print(f"sync-metadata: {len(findings)} forbidden-substring hits:", file=sys.stderr)
        for rel_path, lineno, needle, text in findings:
            print(f"  {rel_path}:{lineno}: matched {needle!r}", file=sys.stderr)
            print(f"      {text}", file=sys.stderr)
        print(
            "\nUpdate the offending lines to use the values from repo-metadata.toml,",
            "or add the path to [audit].allowlist if the reference is intentional",
            "(e.g. historical CHANGELOG entries).",
            file=sys.stderr,
        )
        return 1

    drift = check_declared(meta)
    if args.about:
        drift += check_about(meta)
    if drift:
        print(f"sync-metadata: {len(drift)} declared value(s) not where repo-metadata.toml says:", file=sys.stderr)
        for problem in drift:
            print(f"  {problem}", file=sys.stderr)
        return 1

    org = meta["repo"]["org"]
    email = meta["maintainer"]["email"]
    print(f"sync-metadata: clean. org={org!r} email={email!r}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
