import pefile
import pytest

import lancelot

from fixtures import *


def test_invalid_pe():
    with pytest.raises(ValueError):
        lancelot.from_bytes(b"")

    with pytest.raises(ValueError):
        lancelot.from_bytes(b"MZ\x9000")

    try:
        lancelot.from_bytes(b"")
    except ValueError as e:
        assert str(e) == "failed to fill whole buffer"


def test_load_pe(k32):
    lancelot.from_bytes(k32)


def test_arch(k32):
    ws = lancelot.from_bytes(k32)
    assert "Returns: str" in lancelot.PE.arch.__doc__
    assert ws.arch == "x64"


def test_functions(k32):
    ws = lancelot.from_bytes(k32)

    assert "Returns: List[int]" in ws.get_functions.__doc__
    functions = ws.get_functions()

    # IDA identifies 2326
    # lancelot identifies around 2200
    assert len(functions) > 2000

    # this is _security_check_cookie
    assert 0x180020250 in functions

    # exports identified by pefile should be identified as functions
    pe = pefile.PE(data=k32)
    base_address = pe.OPTIONAL_HEADER.ImageBase
    for export in pe.DIRECTORY_ENTRY_EXPORT.symbols:
        if export.forwarder is not None:
            continue
        address = base_address + export.address
        assert address in functions


def test_flow_const():
    assert lancelot.FLOW_TYPE_FALLTHROUGH == 0
    assert lancelot.FLOW_TYPE_CALL == 1


def test_cfg(k32):
    ws = lancelot.from_bytes(k32)

    assert "Returns: CFG" in ws.build_cfg.__doc__
    # this is _report_gsfailure
    # it has a diamond shape
    cfg = ws.build_cfg(0x1800202B0)

    assert cfg.address == 0x1800202B0
    assert len(cfg.basic_blocks) == 4

    assert 0x1800202B0 in cfg.basic_blocks
    assert 0x180020334 in cfg.basic_blocks
    assert 0x1800202F3 in cfg.basic_blocks
    assert 0x180020356 in cfg.basic_blocks

    bb0 = cfg.basic_blocks[0x1800202B0]
    assert 0x180020334 in map(lambda flow: flow[lancelot.FLOW_VA], bb0.successors)
    assert 0x1800202F3 in map(lambda flow: flow[lancelot.FLOW_VA], bb0.successors)
