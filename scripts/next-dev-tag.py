#!/usr/bin/env python3

import argparse
import datetime as dt
import re
import subprocess
import sys


TAG_RE = re.compile(r"^[0-9]{2}w[0-9]{2}[a-z]*$")


def git(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def require_git(args: list[str]) -> str:
    result = git(args)
    if result.returncode != 0:
        print(result.stderr.strip() or f"git {' '.join(args)} failed", file=sys.stderr)
        sys.exit(result.returncode)
    return result.stdout


def current_prefix() -> str:
    iso = dt.date.today().isocalendar()
    return f"{iso.year % 100:02d}w{iso.week:02d}"


def next_suffix(suffix: str) -> str:
    if not suffix:
        return "a"
    if not suffix.isalpha() or not suffix.islower():
        return "a"

    chars = list(suffix)
    for index in range(len(chars) - 1, -1, -1):
        if chars[index] != "z":
            chars[index] = chr(ord(chars[index]) + 1)
            return "".join(chars)
        chars[index] = "a"
    return "a" + "".join(chars)


def latest_tag(prefix: str) -> str | None:
    tags = require_git(["tag", "--list", f"{prefix}*", "--sort=-v:refname"])
    return next((tag for tag in tags.splitlines() if TAG_RE.fullmatch(tag)), None)


def next_tag(prefix: str) -> str:
    latest = latest_tag(prefix)
    if latest is None:
        return f"{prefix}a"
    return f"{prefix}{next_suffix(latest[len(prefix):])}"


def ensure_clean_worktree(force: bool) -> None:
    if force:
        return
    status = require_git(["status", "--porcelain"])
    if status.strip():
        print("Refusing to tag with a dirty worktree. Use --force to override.", file=sys.stderr)
        sys.exit(1)


def tag_exists(tag: str) -> bool:
    result = git(["rev-parse", "--verify", "--quiet", f"refs/tags/{tag}"])
    return result.returncode == 0


def create_tag(tag: str, force: bool) -> None:
    ensure_clean_worktree(force)
    if tag_exists(tag):
        print(f"Tag already exists: {tag}", file=sys.stderr)
        sys.exit(1)
    require_git(["tag", tag])


def push_tag(tag: str) -> None:
    require_git(["push", "origin", tag])


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compute and optionally create the next SwitchYard dev-release tag."
    )
    parser.add_argument(
        "action",
        choices=("print", "create", "push"),
        nargs="?",
        default="print",
        help="print the next tag, create it locally, or create and push it",
    )
    parser.add_argument(
        "--prefix",
        default=current_prefix(),
        help="override the YYwWW prefix, defaulting to the current ISO week",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="allow creating a tag with a dirty worktree",
    )
    args = parser.parse_args()

    if not re.fullmatch(r"[0-9]{2}w[0-9]{2}", args.prefix):
        print("--prefix must match YYwWW", file=sys.stderr)
        return 2

    tag = next_tag(args.prefix)
    print(tag)

    if args.action in ("create", "push"):
        create_tag(tag, args.force)
    if args.action == "push":
        push_tag(tag)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
