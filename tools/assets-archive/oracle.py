"""Independent, tiny-fixture oracle. Never extracts or executes archive members."""
import hashlib
import json
import struct
import zlib

NAMES = ('amd64-repeated', 'armv7-unresolved', 'outer-crc', 'member-digest',
         'unsafe-name', 'missing-reference', 'duplicate-json', 'wrong-platform')
ERRORS = dict(zip(NAMES[2:], ('decode', 'policy:MemberDigest', 'policy:Name',
                            'policy:References', 'policy:Json', 'policy:PlatformMismatch')))
LIMIT = 1024 * 1024

def require(value, message):
    if not value:
        raise ValueError(message)

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result

def strict(raw):
    def reject(_):
        raise ValueError('nonintegral JSON')
    return json.loads(raw, object_pairs_hook=pairs, parse_float=reject, parse_constant=reject)

def ungzip(encoded):
    require(len(encoded) <= LIMIT, 'encoded fixture bound')
    decoder = zlib.decompressobj(31)
    raw = decoder.decompress(encoded, LIMIT + 1)
    require(len(raw) <= LIMIT and decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail,
            'complete bounded single gzip')
    return raw

def gzip(raw):
    # RFC1952 with independent stored DEFLATE blocks; deterministic across Python/zlib versions.
    require(0 < len(raw) <= LIMIT, 'mutation fixture bound')
    out = bytearray(b'\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\xff')
    for offset in range(0, len(raw), 65535):
        block = raw[offset:offset+65535]
        out += bytes([int(offset + len(block) == len(raw))])
        out += struct.pack('<HH', len(block), 65535-len(block)) + block
    out += struct.pack('<II', zlib.crc32(raw), len(raw))
    return bytes(out)

def checksum(header):
    header[148:156] = b'        '
    header[148:156] = ('%06o\0 ' % sum(header)).encode()

def unpack(raw):
    entries = []
    cursor = 0
    while cursor + 512 <= len(raw) and any(raw[cursor:cursor+512]):
        h = raw[cursor:cursor+512]
        check = bytearray(h); check[148:156] = b'        '
        require(int(h[148:156].strip(b'\0 '), 8) == sum(check), 'tar checksum')
        require(h[257:265] == b'ustar\x0000' and h[156:157] == b'0', 'regular USTAR')
        require(not any(h[157:257]+h[345:512]), 'no link/prefix/extensions')
        name, separator, tail = h[:100].partition(b'\0')
        require(separator and not any(tail), 'canonical name termination')
        name = name.decode('ascii')
        size = int(h[124:136].strip(b'\0 '), 8)
        start = cursor + 512; end = start + size; padded = (end+511)//512*512
        require(padded <= len(raw) and not any(raw[end:padded]), 'tar payload padding')
        require(name not in [entry[0] for entry in entries], 'unique tar members')
        entries.append((name, raw[start:end], h)); cursor = padded
    require(raw[cursor:] == bytes(1024), 'exact tar end markers')
    return entries

def pack(entries):
    out = bytearray()
    for name, body, original in entries:
        header = bytearray(original)
        require(len(name.encode()) < 100, 'fixture name bound')
        header[:100] = name.encode().ljust(100, b'\0')
        header[124:136] = ('%011o\0' % len(body)).encode()
        checksum(header)
        out += header + body + bytes((-len(body)) % 512)
    return bytes(out) + bytes(1024)

def expected(encoded, name):
    require(name in NAMES[:2], 'positive fixture identity')
    raw = ungzip(encoded); entries = unpack(raw)
    require(len(entries) == 4 and entries[-1][0] == 'manifest.json', 'manifest last with two unique layers')
    manifest = strict(entries[-1][1])
    require(type(manifest) is list and len(manifest) == 1, 'single image manifest')
    descriptor = manifest[0]
    require(set(descriptor) <= {'Config','Layers','RepoTags','LayerSources'} and
            not descriptor.get('LayerSources'), 'local image descriptor')
    config_name, config_raw, _ = entries[0]
    require(config_name == 'sha256:'+sha(config_raw) == descriptor['Config'], 'colon config identity')
    config = strict(config_raw)
    arch = 'amd64' if name == NAMES[0] else 'arm'
    require(config['os'] == 'linux' and config['architecture'] == arch and not config.get('variant'), 'synthetic platform')
    require(config['rootfs']['type'] == 'layers', 'rootfs type')
    tags = ['example.invalid/fixture:'+name]
    require(descriptor['RepoTags'] == tags, 'synthetic tag metadata')
    a, b = entries[1:3]
    require(descriptor['Layers'] == [a[0], b[0], a[0]], 'ordered repeated reference')
    layer_rows = []
    diffs = []
    for entry, label in [(a, 'synthetic-a'), (b, 'synthetic-b')]:
        layer_name, stored, _ = entry
        require(layer_name == sha(stored)+'.tar.gz', 'stored layer identity')
        inner = ungzip(stored); content = unpack(inner)
        require(len(content) == 1 and content[0][0] == 'fixture.txt' and
                content[0][1] == ((label+'\n')*1025).encode(), 'synthetic layer content')
        diffs.append('sha256:'+sha(inner))
        layer_rows.append({'sha256':sha(stored), 'bytes':len(stored), 'declared_diff_id':sha(inner)})
    require(config['rootfs']['diff_ids'] == [diffs[0], diffs[1], diffs[0]], 'fixture declared DiffIDs')
    return {'status':'ok', 'decoded_bytes':len(raw), 'decoded_sha256':sha(raw),
            'config_sha256':sha(config_raw), 'config_bytes':len(config_raw), 'os':'linux',
            'architecture':arch, 'variant':None,
            'platform_status':'DeclaredMatch' if arch == 'amd64' else 'VariantUnresolved',
            'archive_manifest_sha256':sha(entries[-1][1]), 'repo_tags':tags,
            'layers':[layer_rows[0],layer_rows[1],layer_rows[0]]}

def cases(first, arm):
    expected(first, NAMES[0]); expected(arm, NAMES[1])
    raw = ungzip(first); entries = unpack(raw)
    outputs = {NAMES[0]:first, NAMES[1]:arm}
    damaged = bytearray(first); damaged[-8] ^= 1; outputs['outer-crc'] = bytes(damaged)
    changed = list(entries); n, body, h = changed[1]; body = bytes([body[0]^1])+body[1:]
    changed[1] = (n, body, h); outputs['member-digest'] = gzip(pack(changed))
    changed = list(entries); _, body, h = changed[0]; changed[0] = ('../config', body, h)
    outputs['unsafe-name'] = gzip(pack(changed))
    outputs['missing-reference'] = gzip(pack([entries[0],entries[1],entries[3]]))
    for name in ['duplicate-json','wrong-platform']:
        changed = list(entries); old, body, h = changed[0]
        if name == 'duplicate-json':
            body = body[:-1] + b',"architecture":"amd64"}'
        else:
            config = strict(body); config['architecture'] = 'arm64'
            body = json.dumps(config, separators=(',',':')).encode()
        new = 'sha256:'+sha(body); changed[0] = (new,body,h)
        manifest = strict(entries[-1][1]); manifest[0]['Config'] = new
        changed[-1] = ('manifest.json',json.dumps(manifest,separators=(',',':')).encode(),entries[-1][2])
        outputs[name] = gzip(pack(changed))
    require(tuple(outputs) == NAMES, 'case order')
    return outputs

def observations(inputs):
    require(tuple(inputs) == NAMES, 'exact ordered fixture inputs')
    require(inputs == cases(inputs[NAMES[0]], inputs[NAMES[1]]), 'exact independently derived corruptions')
    return {name: {'case':name, 'archive_sha256':sha(raw),
                   'observed':expected(raw,name) if name in NAMES[:2] else {'status':ERRORS[name]}}
            for name,raw in inputs.items()}
