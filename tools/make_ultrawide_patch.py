#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""The "Resolution Patch - WxH" entry of an ultrawide resolution, with a Hor+ field of view (needs capstone).

  python tools/make_ultrawide_patch.py EBOOT_6E72 XML WIDTH HEIGHT [--write]
      Prints (or with --write puts into XML, replacing an earlier one) the patch for WIDTH x HEIGHT.
      EBOOT_6E72 is the eboot.bin with sha256 6e72330e... (the build shadPS4's patches target): the
      sites below are addresses in it. Every site, cave and constant is then found in any build by a
      signature (Pattern/At) that is unique in that eboot.bin, like the other patches of the XML.

  python tools/make_ultrawide_patch.py --rebase EBOOT_D85C XML WIDTH HEIGHT
      Rewrites the Address attributes of that entry to the addresses of the reference build
      (sha256 d85c8135...), found by the same signatures. The Address values are what the reference
      build test compares the signatures with; run it with the reference eboot.bin when you have it.

Why. The game's camera has a fixed horizontal field of view: with only the render resolution changed,
a 21:9 picture is the 16:9 one cropped top and bottom ("zoomed in"). The patch
  1. sets the render resolution (the sites of the other resolution patches) and the video mode's aspect
     ratio floats (16/9 -> WIDTH/HEIGHT; many sizes are derived from width and height by the game),
  2. scales the tangent t of the field of view by k = (WIDTH/HEIGHT) / (16/9) where the cameras read it:
     the projection matrix is x = 2/t, y = 2A/t, so t*k with the new aspect A keeps the 16:9 vertical
     view and widens the horizontal one. The same t feeds the culling vectors (two builders) and the
     standard camera's tan(fov/2), so all four are scaled and nothing is culled at the sides.
There is no room for the multiplication in place, so each of the four sites becomes a jmp into two
chained 15-byte NOP pads between functions (caves), and a ninth pad holds the constant k.
"""
import argparse
import re
import struct
import sys
from pathlib import Path

import capstone
from capstone import x86
import xml.etree.ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'scripts'))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from make_signatures import code_signature, load, pattern_text  # noqa: E402
from patches import EBOOT_BASE, find_pattern, locate  # noqa: E402

REFERENCE_ASPECT = 16 / 9
# Addresses in the 6e72 build. (address, original instruction bytes, register holding the FOV tangent after it)
CAMERA_SITES = [
    (0x5cc01f, 'c5fa1089d8000000', 1),   # vmovss xmm1, [rcx+0xd8]         projection of the flagged camera
    (0x5cbd89, 'c5fa1080d8000000', 0),   # vmovss xmm0, [rax+0xd8]         culling vectors, builder 1
    (0x5ccaa9, 'c5fa1080d8000000', 0),   # vmovss xmm0, [rax+0xd8]         culling vectors, builder 2
    (0x5cc20f, 'c5fa5e852cffffff', 0),   # vdivss xmm0, xmm0, [rbp-0xd4]   standard camera, tan(fov/2)
]
ASPECT_SITE = (0x5f0c77, '398ee33f398ee33f')  # movabs rax, <16/9,16/9>: the video mode's aspect ratio floats
CAVE_REGION = (0x5c0000, 0x5e0000)  # the pads are taken near the camera code (any address would do)
CAVE_PAD = re.compile(rb'(?:\xc3|\xe9....|\xeb.)((?:\x66)+\x2e\x0f\x1f\x84\x00\x00\x00\x00\x00)', re.S)


def branch_targets(image, text):
    """Every address a direct branch or rip-relative operand points at (a pad that is one is not a cave)."""
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    md.detail = True
    md.skipdata = True
    targets, at = set(), text[0]
    while at < text[1]:
        decoded = 0
        for ins in md.disasm(image[at:min(text[1], at + 0x4000)], at):
            decoded += 1
            at = ins.address + ins.size
            if ins.id == 0:
                continue
            if ins.imm_offset and ins.imm_size == 4 and set(ins.groups) & {x86.X86_GRP_JUMP, x86.X86_GRP_CALL}:
                targets.add(at + int.from_bytes(image[ins.address + ins.imm_offset:at], 'little', signed=True))
            for op in ins.operands:
                if op.type == x86.X86_OP_MEM and op.mem.base == x86.X86_REG_RIP:
                    targets.add(at + op.mem.disp)
        if not decoded:
            at += 1
    return targets


def free_pads(image, text, count):
    """(offset, signature, at) of `count` 15-byte NOP pads after a ret or jmp that nothing points at."""
    targets = branch_targets(image, text)
    pads = []
    for m in CAVE_PAD.finditer(image):
        offset = m.start(1)
        if len(m.group(1)) != 15 or not CAVE_REGION[0] <= offset + EBOOT_BASE < CAVE_REGION[1]:
            continue
        if any(t in targets for t in range(offset, offset + 15)):
            continue
        try:
            pattern, at = code_signature(image, text, offset, 15)
        except SystemExit:
            continue
        pads.append((offset, pattern, at))
        if len(pads) == count:
            return pads
    raise SystemExit(f'only {len(pads)} usable pads')


def entry(image, text, reference_lines, width, height):
    aspect = width / height
    k = aspect / REFERENCE_ASPECT
    area = width * height
    arena, dmem, gib = (('000000b0', 3072, '2.75') if area <= 2560 * 1440 else
                        ('000000d0', 4608, '3.25') if area <= 3200 * 1800 else ('000000f0', 6144, '3.75'))
    values = {0x5f0b3b: struct.pack('<I', width), 0x5f0b43: struct.pack('<I', height),
              0x5f0b4c: struct.pack('<H', width), 0x5f0b53: struct.pack('<H', height),
              0x5643f3: struct.pack('<I', height), 0x564400: struct.pack('<I', width),
              0x5ff769: struct.pack('<I', width), 0x5ff76f: struct.pack('<I', height),
              0x7c46b4: struct.pack('<I', width), 0x7c46b8: struct.pack('<I', height),
              0x56bb82: struct.pack('<I', height), 0x60953d: struct.pack('<I', height),
              0x7f2a3c: struct.pack('<I', width // 2), 0x7f2a40: struct.pack('<I', height // 2),
              0x7cd918: bytes.fromhex(arena)}
    lines = []  # (Address, Value, Pattern, At)
    for line in reference_lines:  # the render resolution sites, as the other resolution patches have them
        address = int(line.get('Address'), 0)
        lines.append((line.get('Address'), values[address].hex(), line.get('Pattern'), line.get('At')))
    targets = []  # (Name, Line index or None, Address, Pattern, At)

    def signature(offset, length):
        pattern, at = code_signature(image, text, offset, length)
        return pattern_text(pattern), str(at)

    off = ASPECT_SITE[0] - EBOOT_BASE
    assert image[off:off + 8].hex() == ASPECT_SITE[1], 'the aspect ratio site differs: needs the 6e72 eboot.bin'
    pattern, at = signature(off, 8)
    lines.append((f'0x{ASPECT_SITE[0]:08x}', (struct.pack('<f', aspect) * 2).hex(), pattern, at))

    pads = free_pads(image, text, 1 + 2 * len(CAMERA_SITES))
    constant = pads[0]

    def add(offset, value, signature_, name=None):
        if name:
            targets.append((name, len(lines), f'0x{offset + EBOOT_BASE:08x}', None, None))
        lines.append((f'0x{offset + EBOOT_BASE:08x}', value) + signature_)

    add(constant[0], struct.pack('<f', k).hex(), (pattern_text(constant[1]), str(constant[2])), 'K')
    for n, (address, original, register) in enumerate(CAMERA_SITES):
        off = address - EBOOT_BASE
        assert image[off:off + 8].hex() == original, f'camera site {address:#x} differs: needs the 6e72 eboot.bin'
        first, second = pads[1 + 2 * n], pads[2 + 2 * n]
        site = signature(off, 8)
        vex = 0x80 | ((~register & 0xf) << 3) | 0x02  # vmulss xmm<r>, xmm<r>, [rip+K]
        add(off, f'e9{{rel:first{n}:0}}0f1f00', site)
        add(first[0], original + f'e9{{rel:second{n}:0}}', (pattern_text(first[1]), str(first[2])), f'first{n}')
        add(second[0], f'c5{vex:02x}59{(register << 3) | 0x05:02x}{{rel:K:0}}e9{{rel:back{n}:0}}',
            (pattern_text(second[1]), str(second[2])), f'second{n}')
        # where the original code goes on: 8 bytes after the site
        targets.append((f'back{n}', None, f'0x{address + 8:08x}', site[0], str(int(site[1]) + 8)))

    note = (f'Ultrawide {width}x{height} ({aspect:.3f}:1), Hor+. Sets the game output resolution and the display '
            f'aspect ratio, widens the horizontal field of view so the 16:9 vertical view is kept (no zoom) and sets '
            f'the Video Arena to {gib} GiB. Requires setting DMEM to {dmem} MB or more. Only leave one resolution '
            f'patch enabled; do not enable it with Bug Fix - Texture Corruption Fix.')
    out = [f'    <Metadata Title="God of War III Remastered" Name="Resolution Patch - {width}x{height}" Note="{note}" '
           f'Author="gow3_pc ultrawide" PatchVer="1.0" AppVer="01.02" AppElf="eboot.bin" isEnabled="false">',
           '        <PatchList>']
    out += [f'            <Line Type="bytes" Address="{a}" Value="{v}" Pattern="{p}" At="{t}"/>' for a, v, p, t in lines]
    out.append('        </PatchList>')
    for name, index, address, pattern, at in targets:
        extra = f'Line="{index}"' if index is not None else f'Pattern="{pattern}" At="{at}"'
        out.append(f'        <Target Name="{name}" Address="{address}" {extra}/>')
    out.append('    </Metadata>')
    return '\n'.join(out) + '\n', k, aspect


def rebase(xml_path, eboot, width, height):
    """Address attributes of the entry -> the addresses its signatures find in `eboot` (the reference build)."""
    image, _ = load(eboot)
    name = f'Resolution Patch - {width}x{height}'
    text = Path(xml_path).read_text(encoding='utf-8')
    block = re.search(rf'    <Metadata [^>]*Name="{re.escape(name)}".*?</Metadata>\n', text, re.S)
    if not block:
        raise SystemExit(f'{name} is not in {xml_path}')
    meta = ET.fromstring(block.group(0))
    where = {}  # old Address attribute -> the address the element's signature finds in the reference eboot.bin
    for element in meta.iter():
        if element.tag not in ('Line', 'Target') or element.get('Pattern') is None:
            continue
        try:
            where[element.get('Address')] = f'0x{locate(image, element) + EBOOT_BASE:08x}'
        except LookupError as error:
            raise SystemExit(f'{element.tag} {element.get("Name") or element.get("Address")}: {error} in {eboot}')
    # Targets that follow a line (Line="n") share its Address, so one pass over the attributes covers them.
    new = re.sub(r'Address="(0x[0-9a-f]+)"', lambda m: f'Address="{where.get(m.group(1), m.group(1))}"', block.group(0))
    Path(xml_path).write_text(text.replace(block.group(0), new), encoding='utf-8')
    print(f'{name}: {len(where)} addresses rewritten from {eboot}')


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('eboot')
    p.add_argument('xml')
    p.add_argument('width', type=int)
    p.add_argument('height', type=int)
    p.add_argument('--write', action='store_true')
    p.add_argument('--rebase', action='store_true')
    a = p.parse_args()
    if a.rebase:
        return rebase(a.xml, a.eboot, a.width, a.height)
    image, text = load(a.eboot)
    root = ET.parse(a.xml).getroot()
    base = next(m for m in root.iter('Metadata') if m.get('Name') == 'Resolution Patch - 1800p')
    result, k, aspect = entry(image, text, list(base.iter('Line')), a.width, a.height)
    if not a.write:
        print(result)
        return
    name = f'Resolution Patch - {a.width}x{a.height}'
    xml = Path(a.xml).read_text(encoding='utf-8')
    xml = re.sub(rf'    <Metadata [^>]*Name="{re.escape(name)}".*?</Metadata>\n', '', xml, flags=re.S)
    Path(a.xml).write_text(xml.replace('</Patch>', result + '</Patch>'), encoding='utf-8')
    print(f'{name}: written to {a.xml} (aspect {aspect:.5f}, field of view x{k:.5f})')


if __name__ == '__main__':
    main()
