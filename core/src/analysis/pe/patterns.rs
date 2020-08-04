// https://github.com/NationalSecurityAgency/ghidra/tree/79d8f164f8bb8b15cfb60c5d4faeb8e1c25d15ca/Ghidra/Processors/x86/data/patterns
#![allow(non_snake_case)]
use anyhow::Result;
use log::debug;
use regex::bytes::Regex;

use crate::{aspace::AddressSpace, loader::pe::PE, VA};

lazy_static! {
    static ref PATTERNS: Regex = {

        // CC debug filler, x86win_patterns.xml#L4
        let CC = r"\xCC";
        // multiple CC filler bytes, x86win_patterns.xml#L5
        let CCCC = r"\xCC\xCC";
        // NOP filler, x86win_patterns.xml#L6
        let NOP = r"\x90";
        // RET filler, x86win_patterns.xml#L7
        let RET = r"\xC3";
        // LEAVE RET, x86win_patterns.xml#L8
        let LEAVE_RET = r"\xC9\xC3";

        // x86win_patterns.xml#L9
        // 0xC2 ......00 0x00
        // let RET_LONGFORM =

        let PREPATTERN = format!("(?P<prepattern>{})", vec![
            CC,
            CCCC,
            NOP,
            RET,
            LEAVE_RET,
        ].join("|"));

        // PUSH EBP; MOV EBP, ESP, x86win_patterns.xml#L12
        let P0 = r"\x55\x8B\xEC";

        // x86win_patterns.xml#L13
        //  SUB ESP, #small
        // <data>0x83ec 0.....00 </data>
        //let P1 = r"\x83\xEC (
        //  \x00|\x04|\x08|\x0c|\x10|\x14|\x18|\x1c|
        //  \x20|\x24|\x28|\x2c|\x30|\x34|\x38|\x3c|
        //  \x40|\x44|\x48|\x4c|\x50|\x54|\x58|\x5c|
        //  \x60|\x64|\x68|\x6c|\x70|\x74|\x78|\x7c)";

        // x86win_patterns.xml#L14
        //  PUSH-1; PUSH FUNC; MOV EAX, FS[0]
        // <data>0x6aff68........64a100000000 </data>
        //let P2 = r"\x6A\xFF\x68....\x64\xA1\x00\x00\x00\x00";

        // x86win_patterns.xml#L15
        //   PUSH ESI; MOV ESI, ECX
        // <data>0x568bf1 </data>
        //let P3 = r"\x56\x8B\xF1";

        // x86win_patterns.xml#L16
        //   MOV EAX, ??; CALL; SUB ESP
        // <data>0xb8........e8........ 100000.1 0xec</data>
        // let P4 = r"";

        // x86win_patterns.xml#L17
        //    MOV EAX, ??; CALL
        // <data>0xb8........e8</data>
        // let P5 = r"";

        // x86win_patterns.xml#L18
        //    MOV EDI,EDI : PUSH EBP : MOV EBP,ESP
        // <data>0x8bff558bec</data>
        let P6 = r"\x8B\xFF\x55\x8B\xEC";

        // x86win_patterns.xml#L20
        //  PUSH EBX : MOV EBX,E*X
        // <data>0x538b 110110..</data>
        // let P7 = r"";

        // x86win_patterns.xml#L21
        //   PUSH EBX : PUSH ESI : PUSH EDI
        // <data>0x535657</data>
        // let P8 = r"";

        // x86win_patterns.xml#L22
        //   PUSH EBX : PUSH EBP : PUSH ESI
        // <data>0x535556</data>
        // let P9 = r"";

        // x86win_patterns.xml#L23
        //  PUSH EBX : PUSH ESI : PUSH ECX
        // <data>0x535651</data>
        // let P10 = r"";

        // x86win_patterns.xml#L25
        //   PUSH EBX : PUSH ESI : MOV ESI,EDX
        // <data>0x53568bf2</data>
        // let P11 = r"";

        // x86win_patterns.xml#L26
        //   PUSH EBX : PUSH ESI : MOV EBX,EAX
        // <data>0x53568bd8</data>
        // let P12 = r"";

        // x86win_patterns.xml#L27
        //   PUSH EBX : PUSH ESI : MOV ESI,ECX
        // <data>0x53568bf1</data>
        // let P13 = r"";

        // x86win_patterns.xml#L28
        //   PUSH EBX : PUSH ESI : MOV EBX,EDX
        // <data>0x53568bda</data>
        // let P14 = r"";

        // x86win_patterns.xml#L29
        //   PUSH EBX : PUSH ESI : MOV ESI,EAX
        // <data>0x53568bf0</data>
        // let P15 = r"";

        // x86win_patterns.xml#L30
        //   PUSH ESI : PUSH EDI : MOV EDI,ECX
        // <data>0x56578bf9</data>
        // let P16 = r"";

        // x86win_patterns.xml#L31
        //   PUSH ESI : PUSH EDI : MOV ESI,ECX
        // <data>0x56578bf1</data>
        // let P17 = r"";

        let POSTPATTERN = format!("(?P<postpattern>{})", vec![
            P0,
            P6,
        ].join("|"));

        let re = format!(
            r"(?x)   # whitespace allowed
              (?-u)  # disable unicode mode, so we can match raw bytes
              (:?{})   # capture the pre match
              (:?{})   # capture the match
            ",
            PREPATTERN,
            POSTPATTERN,
            );

        Regex::new(&re).unwrap()
    };
}

const INDEX_ALL: usize = 0;
const INDEX_PREMATCH: usize = 2;
const INDEX_MATCH: usize = 4;

pub fn find_function_prologues(pe: &PE) -> Result<Vec<VA>> {
    let mut ret = vec![];
    for section in pe.executable_sections() {
        let vstart: VA = section.virtual_range.start;
        let vsize = (section.virtual_range.end - section.virtual_range.start) as usize;
        let sec_buf = pe.module.address_space.read_buf(vstart, vsize)?;

        for capture in PATTERNS.captures_iter(&sec_buf) {
            let m = capture.get(INDEX_MATCH).unwrap();
            let va = vstart + m.start() as u64;
            ret.push(va);
        }
    }
    Ok(ret)
}
