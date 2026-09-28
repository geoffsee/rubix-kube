"""Verify the complete selected Go module graph, including zip and go.mod checksums."""
import json
import sys
from pathlib import Path

def normalized(raw):
    decoder = json.JSONDecoder()
    rows = []
    while raw.strip():
        row, end = decoder.raw_decode(raw.lstrip())
        raw = raw.lstrip()[end:]
        if row.get('Replace') or row.get('Error'):
            raise ValueError('replacement or failed module')
        if row.get('Main'):
            if row['Path'] != 'rubix.invalid/assets-archive-fixture':
                raise ValueError('main module')
            continue
        selected = {key: row[key] for key in ['Path', 'Version', 'Sum', 'GoModSum']}
        if not selected['Sum'].startswith('h1:') or not selected['GoModSum'].startswith('h1:'):
            raise ValueError('module checksum')
        rows.append(selected)
    if not rows or len({row['Path'] for row in rows}) != len(rows):
        raise ValueError('module graph identity')
    return sorted(rows, key=lambda row: row['Path'])

if __name__ == '__main__':
    actual = normalized(Path(sys.argv[1]).read_text())
    expected = json.loads(Path(sys.argv[2]).read_text())
    if actual != expected:
        raise ValueError('pinned module graph mismatch')
