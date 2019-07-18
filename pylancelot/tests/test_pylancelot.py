import pytest

import pylancelot

import fixtures
from fixtures import *


def test_workspace():
    ws = pylancelot.from_bytes('foo.bin', b'\xEB\xFE')
    assert str(ws) == 'PyWorkspace(filename: foo.bin loader: Windows/x32/Raw)'
    assert repr(ws) == 'PyWorkspace(filename: foo.bin loader: Windows/x32/Raw)'


def test_filename():
    ws = pylancelot.from_bytes('foo.bin', b'\xEB\xFE')
    assert ws.filename == 'foo.bin'


def test_loader():
    ws = pylancelot.from_bytes('foo.bin', b'\xEB\xFE')
    assert ws.loader == 'Windows/x32/Raw'


def test_base_address():
    ws = pylancelot.from_bytes('foo.bin', b'\xEB\xFE')
    assert ws.base_address == 0x0


def test_perms():
    assert pylancelot.PERM_R == 0x1


def test_sections():
    ws = pylancelot.from_bytes('foo.bin', b'\xEB\xFE')
    sec = ws.sections[0]

    assert sec.addr == 0x0
    assert sec.length == 0x2
    assert sec.perms == pylancelot.PERM_RWX
    assert sec.name == 'raw'
    assert str(sec) == 'PySection(addr: 0x0 length: 0x2 perms: 0x7 name: raw)'
    assert repr(sec) == 'PySection(addr: 0x0 length: 0x2 perms: 0x7 name: raw)'


def test_probe():
    ws = pylancelot.from_bytes('foo.bin', b'\xEB\xFE')
    assert     ws.probe(0x0)
    assert     ws.probe(0x0, 1)
    assert     ws.probe(0x0, 2)
    assert not ws.probe(0x0, 3)
    assert     ws.probe(0x1)
    assert not ws.probe(0x2)


def test_read_bytes():
    ws = pylancelot.from_bytes('foo.bin', b'\xEB\xFE')
    assert ws.read_bytes(0x0, 0) == b''
    assert ws.read_bytes(0x0, 1) == b'\xEB'
    assert ws.read_bytes(0x0, 2) == b'\xEB\xFE'

    with pytest.raises(LookupError):
        assert ws.read_bytes(0x2, 2)


def test_read_element():
    ws = pylancelot.from_bytes('foo.bin', b'\x00\x11\x22\x33\x44\x55\x66\x77')

    with pytest.raises(LookupError):
        assert ws.read_u8(0x10)

    assert ws.read_u8(0x0) == 0x00
    assert ws.read_u8(0x1) == 0x11

    assert ws.read_u16(0x0) == 0x1100
    assert ws.read_u32(0x0) == 0x33221100
    assert ws.read_u64(0x0) == 0x7766554433221100

    assert ws.read_rva(0x0) == 0x33221100
    assert ws.read_va(0x0) == 0x33221100


def test_xrefs_from(k32):
    # fallthrough
    assert 0x130D8 in map(lambda x: x.dst, k32.get_xrefs_from(0x130D6))
    # cjmp
    assert 0x130DD in map(lambda x: x.dst, k32.get_xrefs_from(0x130D6))


def test_xrefs_to(k32):
    # fallthrough
    assert 0x130D8 in map(lambda x: x.src, k32.get_xrefs_to(0x130DD))
    # cjmp
    assert 0x130D6 in map(lambda x: x.src, k32.get_xrefs_to(0x130DD))


def test_zydis_issue_21(k32):
    # there should not be a crash here
    _ = k32.read_insn(0x10D7)


def test_read_insn():
    ws = pylancelot.from_bytes('foo.bin', b'\xEB\xFE')
    insn = ws.read_insn(0x0)
    assert insn['mnemonic'] == 'JMP'
    assert insn['operands'][0]['ty'] == 'IMMEDIATE'
    # TODO: make this value signed (-2)
    assert insn['operands'][0]['imm']['value'] == 0xfffffffffffffffe
