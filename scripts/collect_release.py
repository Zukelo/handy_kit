"""Collect only expected installers and assign collision-free release names."""
import argparse
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = {'windows': {'.exe'}, 'macos': {'.dmg'}, 'linux': {'.deb', '.AppImage'}}


def collect(bundle, destination, platform, arch, edition, tag):
    files = [path for path in bundle.rglob('*') if path.is_file() and path.suffix in EXPECTED[platform]]
    suffixes = [path.suffix for path in files]
    if len(suffixes) != len(EXPECTED[platform]) or set(suffixes) != EXPECTED[platform]:
        raise ValueError(f'Expected exactly {EXPECTED[platform]}, found {files}')
    destination.mkdir(parents=True, exist_ok=True)
    result = []
    for source in files:
        target = destination / f'HandyKit-{tag}-{platform}-{arch}-{edition}{source.suffix}'
        shutil.copy2(source, target)
        result.append(target)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', required=True)
    parser.add_argument('--platform', required=True, choices=EXPECTED)
    parser.add_argument('--arch', required=True, choices=['x64', 'arm64'])
    parser.add_argument('--edition', required=True, choices=['online', 'offline', 'system-webview'])
    parser.add_argument('--tag', required=True)
    args = parser.parse_args()
    bundle = ROOT / 'development/apps/desktop/src-tauri/target' / args.target / 'release/bundle'
    for path in collect(bundle, ROOT / 'release-assets', args.platform, args.arch, args.edition, args.tag):
        print(path.name)
