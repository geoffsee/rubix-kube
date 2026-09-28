"""Bounded strict readers shared by read-only container qualification verifiers."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
MAX = 8 * 1024 * 1024

def require(value, message):
    if not value:
        raise ValueError(message)

def read(path, limit=MAX):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        require(stat.S_ISREG(os.fstat(stream.fileno()).st_mode), 'regular file')
        raw = stream.read(limit + 1)
    require(len(raw) <= limit, 'file budget')
    return raw

def digest(path, limit=MAX):
    return hashlib.sha256(read(path, limit)).hexdigest()

def pairs(rows):
    result = {}
    for key, value in rows:
        require(key not in result, 'duplicate key')
        result[key] = value
    return result

def reject(_):
    raise ValueError('noninteger JSON number')

def loads(raw):
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=reject, parse_float=reject)

def load(path):
    return loads(read(path))

def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value

def inventory():
    # The historical compiled-source contract belongs to the approved builder.
    # This separate inventory binds every new fixture file, excluding publication.
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(HERE.rglob('*'))
            if p.is_file() and not any(x in {'evidence', '__pycache__'} for x in p.parts)
            and p.name not in {'README.md', 'provenance.json'}}
