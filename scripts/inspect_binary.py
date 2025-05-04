"""Inspect the packaged PE without loading or executing any Windows code."""
import json
from pathlib import Path
import struct
import sys
import xml.etree.ElementTree as ET

path = Path(sys.argv[1])
data = path.read_bytes()


def u16(offset):
    return struct.unpack_from('<H', data, offset)[0]


def u32(offset):
    return struct.unpack_from('<I', data, offset)[0]


assert data[:2] == b'MZ', 'Missing DOS header'
pe = u32(0x3C)
assert data[pe:pe + 4] == b'PE\0\0', 'Missing PE signature'
assert u16(pe + 4) == 0x8664, 'Expected x64'
optional = pe + 24
assert u16(optional) == 0x20B, 'Expected PE32+'
assert u16(optional + 68) == 2, 'Expected Windows GUI subsystem'
sections = optional + u16(pe + 20)


def raw(rva):
    for index in range(u16(pe + 6)):
        section = sections + index * 40
        address = u32(section + 12)
        if address <= rva < address + max(u32(section + 8), u32(section + 16)):
            return u32(section + 20) + rva - address
    raise ValueError(f'RVA outside sections: {rva}')


imports = []
descriptor = raw(u32(optional + 112 + 8))
while any(data[descriptor:descriptor + 20]):
    start = raw(u32(descriptor + 12))
    imports.append(data[start:data.index(0, start)].decode('ascii'))
    descriptor += 20
assert not any(name.lower().startswith(('vcruntime', 'msvcp', 'api-ms-win-crt', 'ucrtbase')) for name in imports), 'Dynamic MSVC runtime dependency'

resource = raw(u32(optional + 112 + 16))


def resource_entries(relative):
    offset = resource + relative
    count = u16(offset + 12) + u16(offset + 14)
    return [(u32(offset + 16 + i * 8), u32(offset + 20 + i * 8)) for i in range(count)]


manifest_type = dict(resource_entries(0))[24]
manifest_id = dict(resource_entries(manifest_type & 0x7FFFFFFF))[1]
language, leaf = resource_entries(manifest_id & 0x7FFFFFFF)[0]
assert not leaf & 0x80000000
entry = resource + leaf
start = raw(u32(entry))
manifest = data[start:start + u32(entry + 4)].rstrip(b'\0')
root = ET.fromstring(manifest)
ns = {'a': 'urn:schemas-microsoft-com:asm.v1', 'v3': 'urn:schemas-microsoft-com:asm.v3', 'dpi': 'http://schemas.microsoft.com/SMI/2016/WindowsSettings'}
assert root.find('v3:trustInfo/v3:security/v3:requestedPrivileges/v3:requestedExecutionLevel', ns).get('level') == 'asInvoker'
assert root.find('v3:application/v3:windowsSettings/dpi:dpiAwareness', ns).text == 'PerMonitorV2'
assert any(element.get('name') == 'Microsoft.Windows.Common-Controls' for element in root.findall('a:dependency/a:dependentAssembly/a:assemblyIdentity', ns))
print(json.dumps({'path': str(path.resolve()), 'bytes': len(data), 'machine': 'x64', 'subsystem': 'Windows GUI', 'manifest': 'asInvoker / PerMonitorV2 / Common Controls v6', 'imports': imports}, indent=2))
