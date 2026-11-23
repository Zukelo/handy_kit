"""Generate reproducible native icons from the checked-in 小工坊 SVG master."""
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / 'development/apps/desktop'
OUT = DESKTOP / 'src-tauri/icons'
BRAND = ROOT / 'docs/design/brand'


def generate():
    # Invoke the installed CLI directly: no npx downloads or image-editing dependencies.
    cli = DESKTOP / 'node_modules/@tauri-apps/cli/tauri.js'
    with tempfile.TemporaryDirectory(prefix='handykit-icons-') as temporary:
        destination = Path(temporary)
        subprocess.run(['node', str(cli), 'icon', str(BRAND / 'handykit.svg'), '--output', str(destination)], check=True, cwd=DESKTOP)
        OUT.mkdir(parents=True, exist_ok=True)
        for name in ['32x32.png', '128x128.png', '128x128@2x.png', 'icon.png', 'icon.ico', 'icon.icns']:
            shutil.copy2(destination / name, OUT / name)
        # Native template is drawn independently for legibility at menu-bar sizes.
        subprocess.run(['node', str(cli), 'icon', str(BRAND / 'tray-template.svg'), '--output', str(destination / 'tray'), '--png', '44'], check=True, cwd=DESKTOP)
        shutil.copy2(destination / 'tray/44x44.png', OUT / 'tray-template.png')
    shutil.copy2(BRAND / 'handykit.svg', DESKTOP / 'src/assets/brand.svg')
    print(f'Generated desktop and tray icons: {OUT}')


if __name__ == '__main__':
    generate()
