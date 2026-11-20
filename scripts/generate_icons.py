"""Generate HandyKit's initial geometric app icon using only the standard library."""
from pathlib import Path
import struct
import zlib

OUT = Path(__file__).resolve().parents[1] / 'development/apps/desktop/src-tauri/icons'


def png(size):
    def pixel(x, y):
        x, y = x / size, y / size
        radius = .20
        dx, dy = max(.09 + radius - x, 0, x - (.91 - radius)), max(.09 + radius - y, 0, y - (.91 - radius))
        if dx * dx + dy * dy > radius * radius:
            return (0, 0, 0, 0)
        h = (.25 <= x <= .33 and .28 <= y <= .72) or (.43 <= x <= .51 and .28 <= y <= .72) or (.25 <= x <= .51 and .46 <= y <= .54)
        k = (.57 <= x <= .64 and .28 <= y <= .72) or (.62 <= x <= .81 and abs(y - (.5 + (x - .64))) <= .047) or (.62 <= x <= .81 and abs(y - (.5 - (x - .64))) <= .047)
        return (241, 247, 230, 255) if h else (177, 210, 148, 255) if k else (52, 103, 79, 255)
    raw = b''.join(b'\0' + bytes(channel for x in range(size) for channel in pixel(x, y)) for y in range(size))
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')


if __name__ == '__main__':
    OUT.mkdir(parents=True, exist_ok=True)
    for size, name in [(32, '32x32.png'), (128, '128x128.png'), (256, '128x128@2x.png'), (256, 'icon.png')]:
        (OUT / name).write_bytes(png(size))
    images = [(size, png(size)) for size in (32, 48, 256)]
    offset = 6 + 16 * len(images)
    entries = []
    for size, data in images:
        entries.append(struct.pack('<BBBBHHII', size % 256, size % 256, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    (OUT / 'icon.ico').write_bytes(struct.pack('<HHH', 0, 1, len(images)) + b''.join(entries) + b''.join(data for _, data in images))
    chunks = b''.join(kind + struct.pack('>I', len(data) + 8) + data for kind, data in [(b'ic08', png(256)), (b'ic09', png(512))])
    (OUT / 'icon.icns').write_bytes(b'icns' + struct.pack('>I', len(chunks) + 8) + chunks)
    print(OUT)
