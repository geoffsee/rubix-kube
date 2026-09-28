"""Independent little-endian ELF field observations; never loads or executes a binary."""
import struct

def inspect(data):
    if data[:7] != b'\x7fELF\x02\x01\x01':
        raise ValueError('expected reviewed ELF64 little-endian release artifact')
    kind,machine,version,entry,phoff,shoff,flags,ehsize,phsize,phnum=struct.unpack_from('<HHIQQQIHHH',data,16)
    if version!=1 or ehsize!=64 or phsize!=56 or phnum>128:raise ValueError('header')
    loads=[];interpreter=None;dynamic=None
    for index in range(phnum):
        tag,mode,offset,address,physical,length,memory,alignment=struct.unpack_from('<IIQQQQQQ',data,phoff+index*phsize)
        if offset+length>len(data):raise ValueError('segment range')
        segment=data[offset:offset+length]
        if tag==1:loads.append((address,length,offset))
        if tag==3:
            if interpreter is not None or not segment.endswith(b'\0'):raise ValueError('interpreter')
            interpreter=segment[:-1].decode('ascii')
        if tag==2:
            if dynamic is not None:raise ValueError('dynamic duplicate')
            dynamic=segment
    needed=[]
    if dynamic is not None:
        tags={};offsets=[]
        if len(dynamic)%16 or len(dynamic)//16>4096:raise ValueError('dynamic size')
        for tag,value in struct.iter_unpack('<QQ',dynamic):
            if tag==0:break
            if tag==1:offsets.append(value)
            else:tags[tag]=value
        else:raise ValueError('dynamic terminator')
        if offsets:
            start=tags[5];size=tags[10];candidates=[data[o+start-a:o+start-a+size] for a,n,o in loads if a<=start and start+size<=a+n]
            if len(candidates)!=1:raise ValueError('string mapping')
            strings=candidates[0]
            for offset in offsets:needed.append(strings[offset:strings.index(b'\0',offset)].decode('ascii'))
    return dict(bytes=len(data),machine=machine,elf_type=kind,entry=entry,flags=flags,interpreter=interpreter,needed=needed)
