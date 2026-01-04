use elf::{
    ElfLoader, MemoryMapper, PageFlags,
    mapper::{MappedRegion, VirtAddr},
};

const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

#[test]
fn load_executable_maps_data_and_bss() {
    let load_data = vec![0xCCu8; 0x10];
    let load_mem = 0x30u64;
    let entry = 0x400010u64;
    let load_vaddr = 0x400000u64;

    let elf_bytes = build_elf(
        entry,
        &[SegmentSpec {
            kind: 1, // PT_LOAD
            flags: PF_R | PF_X,
            vaddr: load_vaddr,
            align: 0x1000,
            data: load_data.clone(),
            mem_size: load_mem,
        }],
    );

    let mut mapper = MockMapper::new(VirtAddr::new_extend(0x8000));
    let bin = ElfLoader::load(&elf_bytes, &mut mapper).expect("load ok");

    let mapped = mapper.mapped.expect("region mapped");
    assert_eq!(mapped.size, 0x1000);

    let copy = mapper.find_copy(mapped.virt_addr).expect("copy recorded");
    assert_eq!(copy.1, load_data);

    assert!(mapper.zeroed.contains(&(
        mapped.virt_addr + load_data.len() as u64,
        load_mem - load_data.len() as u64
    )));

    assert!(
        mapper
            .updates
            .contains(&(mapped.virt_addr, load_mem, PageFlags::rx()))
    );

    let entry_ptr = bin.entry_point as usize;
    assert_eq!(
        entry_ptr as u64,
        mapped.virt_addr.as_u64() + (entry - load_vaddr)
    );
    assert!(bin.tls_template.is_none());
}

#[test]
fn load_tls_segment_returns_template() {
    let load_data = vec![0xAAu8; 0x10];
    let tls_data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];

    let load_vaddr = 0x400000u64;
    let tls_vaddr = 0x400800u64;

    let elf_bytes = build_elf(
        0x400010,
        &[
            SegmentSpec {
                kind: 1, // PT_LOAD
                flags: PF_R | PF_W,
                vaddr: load_vaddr,
                align: 0x1000,
                data: load_data.clone(),
                mem_size: 0x200,
            },
            SegmentSpec {
                kind: 7, // PT_TLS
                flags: PF_R,
                vaddr: tls_vaddr,
                align: 0x10,
                data: tls_data.clone(),
                mem_size: 0x20,
            },
        ],
    );

    let mut mapper = MockMapper::new(VirtAddr::new_extend(0x9000));
    let bin = ElfLoader::load(&elf_bytes, &mut mapper).expect("load ok");

    let mapped = mapper.mapped.expect("region mapped");

    let tls = bin.tls_template.expect("tls template");
    assert_eq!(tls.start, mapped.virt_addr + (tls_vaddr - load_vaddr));
    assert_eq!(tls.file_size, tls_data.len() as u64);
    assert_eq!(tls.mem_size, 0x20);

    assert!(mapper.copied_to(mapped.virt_addr + (tls_vaddr - load_vaddr), &tls_data));
    assert!(mapper.zeroed.contains(&(
        mapped.virt_addr + (tls_vaddr - load_vaddr) + tls_data.len() as u64,
        0x20 - tls_data.len() as u64
    )));
}

#[test]
fn gnu_relro_sets_readonly_flags() {
    let elf_bytes = build_elf(
        0x400010,
        &[
            SegmentSpec {
                kind: 1, // PT_LOAD
                flags: PF_R | PF_W,
                vaddr: 0x400000,
                align: 0x1000,
                data: vec![0u8; 0x20],
                mem_size: 0x200,
            },
            SegmentSpec {
                kind: 0x6474_e552, // PT_GNU_RELRO
                flags: PF_R,
                vaddr: 0x400100,
                align: 0x10,
                data: vec![],
                mem_size: 0x40,
            },
        ],
    );

    let mut mapper = MockMapper::new(VirtAddr::new_extend(0xA000));
    ElfLoader::load(&elf_bytes, &mut mapper).expect("load ok");

    assert!(mapper.updates.iter().any(|(addr, size, flags)| *addr
        == VirtAddr::new_extend(0xA000 + 0x100)
        && *size == 0x40
        && *flags == PageFlags::r()));
}

#[test]
fn unsupported_interp_rolls_back() {
    let elf_bytes = build_elf(
        0x400010,
        &[
            SegmentSpec {
                kind: 1, // PT_LOAD
                flags: PF_R | PF_X,
                vaddr: 0x400000,
                align: 0x1000,
                data: vec![0u8; 0x10],
                mem_size: 0x100,
            },
            SegmentSpec {
                kind: 3, // PT_INTERP
                flags: PF_R,
                vaddr: 0x400100,
                align: 0x10,
                data: vec![0u8; 4],
                mem_size: 4,
            },
        ],
    );

    let mut mapper = MockMapper::new(VirtAddr::new_extend(0xB000));
    let err = ElfLoader::load(&elf_bytes, &mut mapper).unwrap_err();
    assert_eq!(err, elf::ElfLoadError::UnsupportedFeature);
    assert!(
        mapper
            .unmapped
            .iter()
            .any(|r| r.virt_addr == VirtAddr::new_extend(0xB000))
    );
    assert!(mapper.rollback_called);
}

#[derive(Clone)]
struct SegmentSpec {
    kind: u32,
    flags: u32,
    vaddr: u64,
    align: u64,
    data: Vec<u8>,
    mem_size: u64,
}

struct MockMapper {
    next_addr: VirtAddr,
    mapped: Option<MappedRegion>,
    copies: Vec<(VirtAddr, Vec<u8>)>,
    zeroed: Vec<(VirtAddr, u64)>,
    updates: Vec<(VirtAddr, u64, PageFlags)>,
    unmapped: Vec<MappedRegion>,
    rollback_called: bool,
}

impl MockMapper {
    fn new(base: VirtAddr) -> Self {
        Self {
            next_addr: base,
            mapped: None,
            copies: Vec::new(),
            zeroed: Vec::new(),
            updates: Vec::new(),
            unmapped: Vec::new(),
            rollback_called: false,
        }
    }

    fn find_copy(&self, addr: VirtAddr) -> Option<&(VirtAddr, Vec<u8>)> {
        self.copies.iter().find(|(dest, _)| *dest == addr)
    }

    fn copied_to(&self, addr: VirtAddr, data: &[u8]) -> bool {
        self.copies
            .iter()
            .any(|(dest, bytes)| *dest == addr && bytes.as_slice() == data)
    }
}

impl MemoryMapper for MockMapper {
    fn map_region(
        &mut self,
        size: u64,
        _flags: PageFlags,
    ) -> core::result::Result<MappedRegion, ()> {
        let region = MappedRegion {
            virt_addr: self.next_addr,
            size,
        };
        self.mapped = Some(region);
        self.next_addr = self.next_addr + size;
        Ok(region)
    }

    fn copy_data(&mut self, dest: VirtAddr, src: &[u8]) -> core::result::Result<(), ()> {
        self.copies.push((dest, src.to_vec()));
        Ok(())
    }

    fn zero_region(&mut self, dest: VirtAddr, size: u64) -> core::result::Result<(), ()> {
        self.zeroed.push((dest, size));
        Ok(())
    }

    fn update_flags(
        &mut self,
        region: MappedRegion,
        flags: PageFlags,
    ) -> core::result::Result<(), ()> {
        self.updates.push((region.virt_addr, region.size, flags));
        Ok(())
    }

    fn unmap_region(&mut self, region: MappedRegion) -> core::result::Result<(), ()> {
        self.unmapped.push(region);
        Ok(())
    }

    fn rollback(&mut self) {
        self.rollback_called = true;
    }
}

fn build_elf(entry: u64, segments: &[SegmentSpec]) -> Vec<u8> {
    let phnum = segments.len() as u16;
    let phoff = 0x40u64;
    let phentsize = 56u16;
    let shentsize = 64u16; // SHT_NULL

    let ph_table_end = phoff as usize + phnum as usize * phentsize as usize;
    let mut elf = vec![0u8; ph_table_end];

    // e_ident
    elf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    elf[4] = 2; // 64-bit
    elf[5] = 1; // little-endian
    elf[6] = 1; // version
    // rest already zero

    write_u16(&mut elf, 0x10, 2); // ET_EXEC
    write_u16(&mut elf, 0x12, 0x3E); // x86_64
    write_u32(&mut elf, 0x14, 1); // version
    write_u64(&mut elf, 0x18, entry);
    write_u64(&mut elf, 0x20, phoff);
    // shoff written later once cursor is known
    write_u32(&mut elf, 0x30, 0); // flags
    write_u16(&mut elf, 0x34, 64); // ehsize
    write_u16(&mut elf, 0x36, phentsize);
    write_u16(&mut elf, 0x38, phnum);
    // shentsize/shnum/shstrndx written after section table is appended

    let mut cursor = align_to_mod(ph_table_end as u64, 0x1000, 0);

    for (idx, seg) in segments.iter().enumerate() {
        if seg.mem_size < seg.data.len() as u64 {
            panic!("mem_size < file_size");
        }

        let align = seg.align.max(1);
        let desired_mod = seg.vaddr % align;
        cursor = align_to_mod(cursor, align, desired_mod);

        let base = phoff as usize + idx * phentsize as usize;
        write_u32(&mut elf, base, seg.kind);
        write_u32(&mut elf, base + 4, seg.flags);
        write_u64(&mut elf, base + 8, cursor);
        write_u64(&mut elf, base + 16, seg.vaddr);
        write_u64(&mut elf, base + 24, seg.vaddr);
        write_u64(&mut elf, base + 32, seg.data.len() as u64);
        write_u64(&mut elf, base + 40, seg.mem_size);
        write_u64(&mut elf, base + 48, align);

        let end = cursor + seg.data.len() as u64;
        if elf.len() < end as usize {
            elf.resize(end as usize, 0);
        }
        elf[cursor as usize..end as usize].copy_from_slice(&seg.data);

        cursor = end;
    }

    // Append a minimal section header table (single SHT_NULL entry)
    let shoff = align_to_mod(cursor, 8, 0);
    let shnum = 1u16;

    let sh_table_end = shoff + shentsize as u64 * shnum as u64;
    if elf.len() < sh_table_end as usize {
        elf.resize(sh_table_end as usize, 0);
    }

    // Write final section header metadata into ELF header
    write_u64(&mut elf, 0x28, shoff);
    write_u16(&mut elf, 0x3A, shentsize);
    write_u16(&mut elf, 0x3C, shnum);
    write_u16(&mut elf, 0x3E, 0); // shstrndx (SHT_NULL)

    elf
}

fn align_to_mod(val: u64, align: u64, modulo: u64) -> u64 {
    let a = align.max(1);
    let mod_target = modulo % a;
    let r = val % a;
    let delta = (mod_target + a - r) % a;
    val + delta
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(buf: &mut [u8], offset: usize, value: u64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
