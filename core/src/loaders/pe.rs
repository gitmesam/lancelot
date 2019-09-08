use log::{debug};
use failure::{Error};
use goblin::{Object};
use goblin::pe::section_table::SectionTable;

use super::super::util;
use super::super::arch::{Arch, VA, RVA};
use super::super::pagemap::{PageMap};
use super::super::loader::{FileFormat, LoadedModule, Loader, Platform, Section, LoaderError, Permissions};
use super::super::analysis::{Analyzer, OrphanFunctionAnalyzer};
use super::super::analysis::pe;


/// The section can be executed as code.
const IMAGE_SCN_MEM_EXECUTE: u32 = 0x20000000;

/// The section can be read.
const IMAGE_SCN_MEM_READ: u32 = 0x40000000;

/// The section can be written to.
const IMAGE_SCN_MEM_WRITE: u32 = 0x80000000;


pub struct PELoader {
    arch: Arch,
}

impl PELoader {
    pub fn new(arch: Arch) -> PELoader {
        PELoader {
            arch
        }
    }

    fn load_header(&self, buf: &[u8], pe: &goblin::pe::PE) -> Result<Section, Error> {
        let hdr_raw_size = match pe.header.optional_header {
            Some(opt) => opt.windows_fields.size_of_headers,
            // assumption: header is at most 0x200 bytes.
            _ => 0x200,
        };

        //   on disk:
        //
        //   +---------------------------------+
        //   |   header        |  sections...  |
        //   +---------------------------------+
        //   .                  \
        //   .  in memory:       \
        //   .                    \
        //   +-----------------+---+        +-------------
        //   |   header        |   |        |  sections...
        //   +-----------------+---+        +-------------
        //                     ^   ^
        //                     |   +--- virtual size
        //                     |        aligned to 0x200
        //                     +-- raw size
        //                         no alignment

        let hdr_raw_size = std::cmp::min(hdr_raw_size as usize, buf.len());
        let hdr_virt_size = util::align(hdr_raw_size, 0x200);
        let mut headerbuf = vec![0; hdr_virt_size];
        {
            let rawbuf = &mut headerbuf[..hdr_raw_size];
            rawbuf.copy_from_slice(&buf[0x0..hdr_raw_size]);
        }

        Ok(Section {
            addr: RVA(0x0),
            size: hdr_virt_size as u32,  // danger
            perms: Permissions::R,
            name: String::from("header"),
        })
    }

    fn load_section(&self, section: &SectionTable) -> Result<Section, Error> {
        let name = String::from_utf8_lossy(&section.name[..])
            .into_owned()
            .trim_end_matches("\u{0}")
            .trim_end()
            .splitn(2, "\u{0}")
            .next()
            .unwrap()
            .to_string();

        // assumption: each section fits within one u32
        let virtual_size = util::align(section.virtual_size as usize, 0x200) as usize;

        let mut perms = Permissions::empty();
        if section.characteristics & IMAGE_SCN_MEM_READ > 0 {
            perms.insert(Permissions::R);
        }
        if section.characteristics & IMAGE_SCN_MEM_WRITE > 0 {
            perms.insert(Permissions::W);
        }
        if section.characteristics & IMAGE_SCN_MEM_EXECUTE > 0 {
            perms.insert(Permissions::X);
        }

        Ok(Section{
            addr: RVA::from(section.virtual_address as i64), // ok: u32 -> i64
            size: virtual_size as u32,  // danger
            perms,
            name,
        })
    }
}

impl Loader for PELoader {
    fn get_arch(&self) -> Arch {
        self.arch
    }

    fn get_plat(&self) -> Platform {
        Platform::Windows
    }

    fn get_file_format(&self) -> FileFormat {
        FileFormat::PE
    }

    /// ```
    /// use lancelot::rsrc::*;
    /// use lancelot::arch::*;
    /// use lancelot::loader::*;
    ///
    /// let loader32 = lancelot::loaders::pe::PELoader::new(Arch::X32);
    /// let loader64 = lancelot::loaders::pe::PELoader::new(Arch::X64);
    /// assert!( ! loader32.taste(&get_buf(Rsrc::K32)));
    /// assert!(   loader64.taste(&get_buf(Rsrc::K32)));
    /// ```
    fn taste(&self, buf: &[u8]) -> bool {
        if let Ok(Object::PE(pe)) = Object::parse(buf) {
            match self.arch {
                Arch::X32 => !pe.is_64,
                Arch::X64 =>  pe.is_64,
            }
        } else {
            false
        }
    }

    /// ```
    /// use lancelot::rsrc::*;
    /// use lancelot::arch::*;
    /// use lancelot::loader::*;
    ///
    /// let loader64 = lancelot::loaders::pe::PELoader::new(Arch::X64);
    /// let (module, analyzers) = loader64.load(&get_buf(Rsrc::K32)).unwrap();
    /// assert_eq!(module.base_address, VA(0x180000000));
    ///
    /// // mismatched bitness
    /// let loader32 = lancelot::loaders::pe::PELoader::new(Arch::X32);
    /// assert!(loader32.load(&get_buf(Rsrc::K32)).is_err());
    /// ```
    fn load(&self, buf: &[u8]) -> Result<(LoadedModule, Vec<Box<dyn Analyzer>>), Error> {
        if let Ok(Object::PE(pe)) = Object::parse(buf) {
            match self.arch {
                Arch::X32 => if  pe.is_64 { return Err(LoaderError::MismatchedBitness.into());},
                Arch::X64 => if !pe.is_64 { return Err(LoaderError::MismatchedBitness.into());},
            }

            let base_address = match pe.header.optional_header {
                Some(opt) => opt.windows_fields.image_base,
                _ => {
                    debug!("using default base address: 0x40:000");
                    0x40_000
                }
            };

            let base_address = VA::from(base_address);

            let mut sections = vec![self.load_header(buf, &pe)?];
            for section in pe.sections.iter() {
                sections.push(self.load_section(section)?);
            }

            let max_address = pe.sections
                .iter()
                .map(|sec| sec.virtual_address + sec.virtual_size)
                .max()
                .unwrap();  // danger
            let max_page_address: RVA = util::align(max_address as usize, 0x1000).into();  // danger
            let mut address_space: PageMap<u8> = PageMap::with_capacity(max_page_address);

            // TODO: map header buffer

            for section in pe.sections.iter() {
                // in nop.exe, we have virtualsize=0x12FE and rawsize=0x2000.
                // this teaches us that we have to handle the case where rawsize > virtualsize.
                //
                // TODO: do we pick align(virtualsize, 0x200) or just virtualsize?
                let raw_size = std::cmp::min(section.virtual_size, section.size_of_raw_data);
                let pstart = section.pointer_to_raw_data as usize;
                let pend = pstart + raw_size as usize;
                let secbuf = &buf[pstart..pend];
                address_space.mapzx(RVA::from(section.virtual_address as i32), secbuf)?;  // danger
            }

            let mut analyzers: Vec<Box<dyn Analyzer>> = vec![
                Box::new(pe::EntryPointAnalyzer::new()),
                Box::new(pe::ExportsAnalyzer::new()),
                Box::new(pe::ImportsAnalyzer::new()),
                Box::new(pe::CFGuardTableAnalyzer::new()),
                Box::new(pe::RelocAnalyzer::new()),
                Box::new(pe::ByteSigAnalyzer::new()),
            ];

            if pe.is_64 {
                analyzers.push(Box::new(pe::RuntimeFunctionAnalyzer::new()));
            }

            // this always needs to go last
            analyzers.push(Box::new(OrphanFunctionAnalyzer::new()));

            Ok((LoadedModule {
                base_address,
                sections,
                address_space,
            }, analyzers))
        } else {
            Err(LoaderError::NotSupported.into())
        }
    }
}
