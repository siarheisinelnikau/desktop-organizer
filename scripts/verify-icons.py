"""Render embedded EXE icons with Win32 DrawIconEx; never launch or capture an app.
Usage: python scripts/verify-icons.py ABSOLUTE_EXE ABSOLUTE_OUTPUT_DIRECTORY
Requires Pillow only for PNG encoding and the contact sheet.
"""
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
from pathlib import Path
import struct
import sys
from PIL import Image, ImageDraw

exe, output = map(lambda value: Path(value).resolve(), sys.argv[1:])
output.mkdir(parents=True, exist_ok=True)
kernel = c.WinDLL("kernel32", use_last_error=True)
user = c.WinDLL("user32", use_last_error=True)
gdi = c.WinDLL("gdi32", use_last_error=True)
def bind(dll, name, result, arguments):
    fn = getattr(dll, name)
    fn.restype, fn.argtypes = result, arguments
    return fn
load = bind(kernel, "LoadLibraryExW", w.HMODULE, [w.LPCWSTR, w.HANDLE, w.DWORD])
free = bind(kernel, "FreeLibrary", w.BOOL, [w.HMODULE])
find = bind(kernel, "FindResourceW", w.HRSRC, [w.HMODULE, c.c_void_p, c.c_void_p])
load_resource = bind(kernel, "LoadResource", w.HGLOBAL, [w.HMODULE, w.HRSRC])
lock = bind(kernel, "LockResource", c.c_void_p, [w.HGLOBAL])
size_resource = bind(kernel, "SizeofResource", w.DWORD, [w.HMODULE, w.HRSRC])
load_icon = bind(user, "LoadImageW", w.HANDLE, [w.HINSTANCE, c.c_void_p, w.UINT, c.c_int, c.c_int, w.UINT])
destroy = bind(user, "DestroyIcon", w.BOOL, [w.HICON])
create_dc = bind(gdi, "CreateCompatibleDC", w.HDC, [w.HDC])
delete_dc = bind(gdi, "DeleteDC", w.BOOL, [w.HDC])
select = bind(gdi, "SelectObject", w.HGDIOBJ, [w.HDC, w.HGDIOBJ])
delete = bind(gdi, "DeleteObject", w.BOOL, [w.HGDIOBJ])
dib = bind(gdi, "CreateDIBSection", w.HBITMAP, [w.HDC, c.c_void_p, w.UINT, c.POINTER(c.c_void_p), w.HANDLE, w.DWORD])
draw = bind(user, "DrawIconEx", w.BOOL, [w.HDC, c.c_int, c.c_int, w.HICON, c.c_int, c.c_int, w.UINT, w.HBRUSH, w.UINT])
flush = bind(gdi, "GdiFlush", w.BOOL, [])
module = load(str(exe), None, 0x22)  # data/image resources only, no DllMain/code execution
if not module:
    raise c.WinError(c.get_last_error())
try:
    resource = find(module, 1, 14)
    if not resource:
        raise RuntimeError("The executable has no application icon group")
    data = c.string_at(lock(load_resource(module, resource)), size_resource(module, resource))
    count = struct.unpack_from("<H", data, 4)[0]
    sizes = [struct.unpack_from("<BB", data, 6 + i * 14) for i in range(count)]
    sizes = [(a or 256, b or 256) for a, b in sizes]
    assert all((n, n) in sizes for n in [16, 20, 24, 32, 48, 64, 128, 256]), sizes
    sheet = Image.new("RGB", (720, 900), "#e4e4e4")
    labels = ImageDraw.Draw(sheet)
    reports = []
    next_y = 0
    for row, n in enumerate([16, 20, 24, 32, 48, 64]):
        icon = load_icon(module, 1, 1, n, n, 0)
        if not icon:
            raise c.WinError(c.get_last_error())
        try:
            for column, background in enumerate([(250, 250, 250), (28, 31, 38)]):
                dc = create_dc(None)
                bits = c.c_void_p()
                header = c.create_string_buffer(struct.pack("<IiiHHIIiiII", 40, n, -n, 1, 32, 0, n*n*4, 0, 0, 0, 0))
                bitmap = dib(dc, header, 0, c.byref(bits), None, 0)
                if not bitmap:
                    raise c.WinError(c.get_last_error())
                previous = select(dc, bitmap)
                try:
                    bg = bytes([background[2], background[1], background[0], 255]) * (n*n)
                    c.memmove(bits, bg, len(bg))
                    if not draw(dc, 0, 0, icon, n, n, 0, None, 3):
                        raise c.WinError(c.get_last_error())
                    flush()
                    rgba = c.string_at(bits, n*n*4)
                    pixels = Image.frombytes("RGBA", (n,n), rgba, "raw", "BGRA").convert("RGB")
                    changed = [(x,y) for y in range(n) for x in range(n) if pixels.getpixel((x,y)) != background]
                    assert changed, f"Blank packaged icon at {n}px"
                    bounds = [min(x for x,y in changed), min(y for x,y in changed), max(x for x,y in changed)+1, max(y for x,y in changed)+1]
                    pixels.save(output / f"embedded-{n}-{'light' if column == 0 else 'dark'}.png")
                    x,y = column*350+12, next_y+22
                    labels.text((x,y-17), f"{n}px native and 3x / bounds {bounds}", fill="black")
                    sheet.paste(pixels, (x,y))
                    sheet.paste(pixels.resize((n*3,n*3), Image.Resampling.NEAREST), (x+80,y))
                    reports.append({"size":n,"background":background,"visible_bounds":bounds,"changed_pixels":len(changed)})
                finally:
                    select(dc, previous)
                    delete(bitmap)
                    delete_dc(dc)
        finally:
            destroy(icon)
        next_y += max(85, n*3+35)
    sheet.save(output / "embedded-icons.png")
    report = {"executable":str(exe),"sha256":hashlib.sha256(exe.read_bytes()).hexdigest(),"resource_sizes":sizes,"renders":reports,"scope":"LoadLibraryEx resource-only + LoadImageW + DrawIconEx, no process launch or OS tray screenshot"}
    (output / "icons.json").write_text(json.dumps(report, indent=2))
    print(json.dumps({"sizes":sizes,"proof":str(output / "embedded-icons.png")}))
finally:
    free(module)
