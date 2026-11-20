"""Verify that frontend, Tauri, Cargo and an optional release tag agree."""
import argparse
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / 'development/apps/desktop'
SEMVER = re.compile(r'^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$')


def check_version(tag=None):
    version = json.loads((DESKTOP / 'package.json').read_text(encoding="utf-8"))['version']
    match = SEMVER.fullmatch(version)
    if not match or (match[4] and any(part.isdigit() and len(part) > 1 and part.startswith('0') for part in match[4].split('.'))):
        raise ValueError(f'Invalid release version: {version}')
    tauri = json.loads((DESKTOP / 'src-tauri/tauri.conf.json').read_text(encoding="utf-8"))['version']
    cargo = (DESKTOP / 'src-tauri/Cargo.toml').read_text(encoding="utf-8")
    cargo_match = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.MULTILINE)
    if tauri != version or not cargo_match or cargo_match[1] != version:
        raise ValueError('package.json, tauri.conf.json and Cargo.toml versions must match')
    lock_path = DESKTOP / 'package-lock.json'
    if lock_path.exists():
        lock = json.loads(lock_path.read_text(encoding="utf-8"))
        if lock['version'] != version or lock['packages']['']['version'] != version:
            raise ValueError('package-lock.json version must match package.json')
    if tag is not None and tag != f'v{version}':
        raise ValueError(f'Tag must equal v{version}, got {tag!r}')
    return version


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag')
    args = parser.parse_args()
    try:
        print(f'Version OK: {check_version(args.tag)}')
    except (ValueError, KeyError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
