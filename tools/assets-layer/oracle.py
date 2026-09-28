"""Independent bounded synthetic byte oracle; no extraction or image execution."""
import base64
import hashlib
import json
import struct
import zlib
LIMIT = 1024 * 1024
POSITIVES = ('gzip', 'gzip-concat', 'zstd', 'zstd-concat')
NAMES = POSITIVES + ('gzip-crc', 'zstd-checksum', 'gzip-truncated', 'zstd-truncated',
                     'wrong-diffid', 'outer-crc', 'decoded-limit', 'ordered-limit')
ERRORS = {'gzip-crc':'policy:Layer(Checksum)', 'zstd-checksum':'policy:Layer(Decoder)',
          'gzip-truncated':'policy:Layer(Truncated)', 'zstd-truncated':'policy:Layer(Truncated)',
          'wrong-diffid':'policy:Layer(DiffId)', 'outer-crc':'decode',
          'decoded-limit':'policy:Layer(DecodedLimit)', 'ordered-limit':'policy:Layer(OrderedLimit)'}
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

def same_json(actual, expected):
    """JSON equality preserving bool/int distinctions, including receipt records."""
    if type(actual) is not type(expected):
        return False
    if type(expected) is dict:
        return actual.keys() == expected.keys() and all(same_json(actual[k],v) for k,v in expected.items())
    if type(expected) is list:
        return len(actual)==len(expected) and all(same_json(a,b) for a,b in zip(actual,expected))
    return actual == expected

def observation_schema(row):
    require(type(row) is dict and set(row)=={'case','archive_sha256','observed','retained_budget','retry_retained_budget'}, 'consumer row schema')
    require(type(row['case']) is str and type(row['archive_sha256']) is str, 'consumer identity types')
    require(type(row['retained_budget']) is bool and type(row['retry_retained_budget']) is bool, 'consumer retention bool types')
    observed=row['observed']
    require(type(observed) is dict and type(observed.get('status')) is str, 'consumer observation schema')
    if observed['status'] != 'ok':
        require(set(observed)=={'status'}, 'failed observation schema')
        return
    require(set(observed)=={'status','outer_bytes','outer_sha256','layers'}, 'successful observation schema')
    require(type(observed['outer_bytes']) is int and observed['outer_bytes']>0 and type(observed['outer_sha256']) is str, 'outer observation types')
    require(type(observed['layers']) is list, 'layer observation list')
    for layer in observed['layers']:
        require(type(layer) is dict and set(layer)=={'stored_sha256','stored_bytes','diff_id','decoded_bytes','codec','frames'}, 'layer observation schema')
        require(all(type(layer[k]) is int and layer[k]>0 for k in ['stored_bytes','decoded_bytes','frames']), 'layer count integer types')
        require(all(type(layer[k]) is str for k in ['stored_sha256','diff_id','codec']), 'layer identity string types')

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


def known_raw(label):
    body = b'synthetic-layer-a\n'*2049 if label == 'a' else b''.join(
        hashlib.sha256(('synthetic-layer-b:%d' % i).encode()).digest() for i in range(1100))
    h = bytearray(512)
    h[100:108]=b'0000644\0'; h[108:116]=b'0000000\0'; h[116:124]=b'0000000\0'
    h[136:148]=b'00000000000\0'; h[156]=ord('0'); h[257:265]=b'ustar\x0000'
    h[329:337]=b'0000000\0'; h[337:345]=b'0000000\0'
    return pack([('fixture.txt', body, bytes(h))])

def upstream_rows(upstream):
    require(set(upstream) == set(POSITIVES), 'exact upstream image identities')
    for name, rows in upstream.items():
        require(type(rows) is list and len(rows)==3 and rows[0]==rows[2], 'upstream repeated references')
        for row,label in zip(rows,('a','b','a')):
            require(set(row)=={'stored_sha256','stored_bytes','diff_id','decoded_base64','frames','codec'}, 'upstream layer schema')
            raw=base64.b64decode(row['decoded_base64'],validate=True)
            require(raw==known_raw(label), 'upstream decode equals independent whole tar bytes')
            require(row['diff_id']==sha(raw), 'upstream DiffID identity')
            require(row['codec']==name.split('-')[0] and type(row['frames']) is int and row['frames']==(2 if name.endswith('-concat') else 1), 'upstream codec/frame profile')
            require(type(row['stored_bytes']) is int and 0<row['stored_bytes']<=LIMIT and type(row['stored_sha256']) is str and len(row['stored_sha256'])==64, 'upstream stored identity')

def expected(encoded,name,upstream):
    require(name in POSITIVES, 'positive image identity')
    raw=ungzip(encoded); entries=unpack(raw)
    require(len(entries)==4 and entries[-1][0]=='manifest.json','unique layers and manifest last')
    config_name,config_raw,_=entries[0]; config=strict(config_raw); manifest=strict(entries[-1][1])
    require(type(manifest) is list and len(manifest)==1,'single image')
    m=manifest[0]
    require(not m.get('LayerSources') and m['Config']==config_name=='sha256:'+sha(config_raw),'config identity')
    require(config['architecture']=='amd64' and config['os']=='linux' and not config.get('variant'),'platform')
    require(m['RepoTags']==['example.invalid/layer:'+name], 'synthetic tag')
    require(m['Layers']==[entries[1][0],entries[2][0],entries[1][0]], 'ordered repeated layers')
    rows=upstream[name]
    require(config['rootfs']=={'type':'layers','diff_ids':['sha256:'+r['diff_id'] for r in rows]}, 'ordered upstream DiffIDs')
    for entry,row in zip(entries[1:3],rows[:2]):
        n,b,_=entry
        require(n==sha(b)+'.tar.gz' and sha(b)==row['stored_sha256'] and len(b)==row['stored_bytes'], 'upstream stored bytes bound to actual crane member')
        require(b.startswith(b'\x1f\x8b') if row['codec']=='gzip' else b.startswith(b'\x28\xb5\x2f\xfd'), 'actual codec framing')
        if row['codec']=='gzip':
            # Independent zlib validates all members and hashes the complete concatenation.
            remaining=b; decoded=b''; frames=0
            while remaining:
                d=zlib.decompressobj(31); decoded+=d.decompress(remaining,LIMIT-len(decoded)+1)
                require(d.eof and not d.unconsumed_tail and len(decoded)<=LIMIT,'complete bounded gzip layer')
                remaining=d.unused_data; frames+=1
            require(decoded==base64.b64decode(row['decoded_base64']) and frames==row['frames'],'independent gzip decode')
    return {'status':'ok','outer_bytes':len(raw),'outer_sha256':sha(raw),
            'layers':[{'stored_sha256':r['stored_sha256'],'stored_bytes':r['stored_bytes'],
                       'diff_id':r['diff_id'],'decoded_bytes':len(base64.b64decode(r['decoded_base64'])),
                       'codec':r['codec'],'frames':r['frames']} for r in rows]}

def rewrite(encoded, transform_layer=None, wrong_diff=False):
    entries=unpack(ungzip(encoded));m=strict(entries[-1][1]);cfg=strict(entries[0][1])
    if transform_layer:
        old,body,h=entries[1];body=transform_layer(body);new=sha(body)+'.tar.gz';entries[1]=(new,body,h)
        m[0]['Layers']=[new if n==old else n for n in m[0]['Layers']]
    if wrong_diff:
        # Repeated A references must retain equal declarations to reach nested DiffID validation.
        cfg['rootfs']['diff_ids'][0]=cfg['rootfs']['diff_ids'][2]='sha256:'+'0'*64
        body=json.dumps(cfg,separators=(',',':')).encode();new='sha256:'+sha(body)
        entries[0]=(new,body,entries[0][2]);m[0]['Config']=new
    entries[-1]=('manifest.json',json.dumps(m,separators=(',',':')).encode(),entries[-1][2])
    return gzip(pack(entries))

def flip(raw,offset):
    result=bytearray(raw);result[offset]^=1;return bytes(result)

def cases(positives,upstream):
    require(tuple(positives)==POSITIVES,'positive case order');upstream_rows(upstream)
    for name,raw in positives.items():expected(raw,name,upstream)
    out=dict(positives)
    out['gzip-crc']=rewrite(positives['gzip'],lambda b:flip(b,-8))
    out['zstd-checksum']=rewrite(positives['zstd'],lambda b:flip(b,-1))
    out['gzip-truncated']=rewrite(positives['gzip'],lambda b:b[:-1])
    out['zstd-truncated']=rewrite(positives['zstd'],lambda b:b[:-1])
    out['wrong-diffid']=rewrite(positives['gzip'],wrong_diff=True)
    out['outer-crc']=flip(positives['gzip'],-8)
    out['decoded-limit']=positives['gzip'];out['ordered-limit']=positives['zstd']
    require(tuple(out)==NAMES,'all case order');return out

def observations(inputs,upstream):
    require(tuple(inputs)==NAMES,'exact ordered cases')
    require(inputs==cases({n:inputs[n] for n in POSITIVES},upstream),'exact independently derived corruptions')
    return {name:{'case':name,'archive_sha256':sha(raw),'observed':expected(raw,name,upstream) if name in POSITIVES else {'status':ERRORS[name]},
                  'retained_budget':True,'retry_retained_budget':True}
            for name,raw in inputs.items()}
