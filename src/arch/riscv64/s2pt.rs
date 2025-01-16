#![allow(unused)]
use super::{
    csr::{write_csr, read_csr,  CSR_HGATP},
    paging::{GenericPTE, Level3PageTable, PagingInstr},
};
use bit_field::BitField;
use core::fmt;
use numeric_enum_macro::numeric_enum;
use tock_registers::interfaces::Writeable;

use crate::memory::{
    addr::{HostPhysAddr, PhysAddr},
    MemFlags,
};
// |Reserved|  PPN  |RSW |Attr|
// |  63-54 | 53-10 |9-8 |7-0 |

bitflags::bitflags! {
    /// Memory attribute fields in the Sv39 translation table format descriptors.
    #[derive(Clone, Copy, Debug)]
    pub struct DescriptorAttr: u64 {
        // Attribute fields in stage 1 Sv39 Block and Page descriptors:

        const VALID =       1 << 0;
        // WHEN R|W|X is 0, this PTE is pointer to next level page table,else Block descriptor
        const READABLE =    1 << 1;
        const WRITABLE =    1 << 2;
        const EXECUTABLE =  1 << 3;
        const USER =        1 << 4;
        const GLOBAL =      1 << 5;
        const ACCESSED =    1 << 6;
        const DIRTY =       1 << 7;
        // RSW fields is bit[8..9]:Reserved for Software

    }
}

impl From<DescriptorAttr> for MemFlags {
    fn from(attr: DescriptorAttr) -> Self {
        let mut flags = Self::empty();
        if !attr.contains(DescriptorAttr::VALID) {
            return flags;
        }
        if attr.contains(DescriptorAttr::READABLE) {
            flags |= Self::READ;
        }
        if attr.contains(DescriptorAttr::WRITABLE) {
            flags |= Self::WRITE;
        }
        if attr.contains(DescriptorAttr::EXECUTABLE) {
            flags |= Self::EXECUTE;
        }
        if attr.contains(DescriptorAttr::USER) {
            flags |= Self::USER;
        }
        flags
    }
}

impl From<MemFlags> for DescriptorAttr {
    fn from(flags: MemFlags) -> Self {
        let mut attr = Self::empty();
        attr |= Self::VALID | Self::USER | Self::ACCESSED | Self::DIRTY;       // stage 2 page table must user
        if flags.contains(MemFlags::READ) {
            attr |= Self::READABLE;
        }
        if flags.contains(MemFlags::WRITE) {
            attr |= Self::WRITABLE;
        }
        if flags.contains(MemFlags::EXECUTE) {
            attr |= Self::EXECUTABLE;
        }
        if flags.contains(MemFlags::USER) {
            attr |= Self::USER;
        }
        attr
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(pub u64);
const PTE_PPN_MASK: u64 = 0x3F_FFFF_FFFF_FC00; //[10..53]ppn     PTE 页表项中的具体格式
const PPN_MASK: u64 = 0xFF_FFFF_FFFF_F000; //[12..55]ppn         56bit物理地址的具体格式
impl PageTableEntry {
    pub const fn empty() -> Self {
        Self(0)
    }
}

impl GenericPTE for PageTableEntry {
    fn addr(&self) -> HostPhysAddr {
        PhysAddr::from(((self.0 & PTE_PPN_MASK) << 2) as usize) //[10:53] ppn
    }

    fn flags(&self) -> MemFlags {
        DescriptorAttr::from_bits_truncate(self.0).into()
    }

    fn is_unused(&self) -> bool {
        self.0 == 0
    }

    fn is_present(&self) -> bool {
        DescriptorAttr::from_bits_truncate(self.0).contains(DescriptorAttr::VALID)
    }

    // 对于 RISC-V 来说. 当 XWR 均为 0, 并且 V 为 1 时，并且不是 leaf, 那么它就是一个 huge_page
    fn is_huge(&self) -> bool {
        self.is_present() & (DescriptorAttr::from_bits_truncate(self.0).contains(DescriptorAttr::READABLE)
            | DescriptorAttr::from_bits_truncate(self.0).contains(DescriptorAttr::WRITABLE)
            | DescriptorAttr::from_bits_truncate(self.0).contains(DescriptorAttr::EXECUTABLE))
    }

    fn set_addr(&mut self, paddr: HostPhysAddr) {
        // 设置 PTE 中的 PPN
        self.0 = (self.0.get_bits(0..7)) | ((paddr as u64 & PPN_MASK) >> 2);
    }

    fn set_flags(&mut self, flags: MemFlags) {
        // 设置 PTE 中的 flags
        let mut attr: DescriptorAttr = flags.into();
        attr |= DescriptorAttr::VALID;
        self.0 = (attr.bits() & !PTE_PPN_MASK as u64) | (self.0 as u64 & PTE_PPN_MASK as u64);
    }

    // fn set_accessed(&mut self) {
    //     // 设置 PTE 中的 flags
    //     let mut attr: DescriptorAttr = flags.into();
    //     attr |= DescriptorAttr::ACCESSED;
    //     self.0 = (attr.bits() & !PTE_PPN_MASK as u64) | (self.0 as u64 & PTE_PPN_MASK as u64);
    // }

    fn set_table(&mut self, paddr: HostPhysAddr) {
        self.set_addr(paddr);
        let attr = DescriptorAttr::VALID;
        self.0 = (attr.bits() & !PTE_PPN_MASK as u64) | (self.0 as u64 & PTE_PPN_MASK as u64);
    }

    fn clear(&mut self) {
        self.0 = 0
    }
}

impl PageTableEntry {
    fn pt_flags(&self) -> MemFlags {
        DescriptorAttr::from_bits_truncate(self.0).into()
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Stage1PageTableEntry")
            .field("raw", &self.0)
            .field("paddr", &self.addr())
            .field("attr", &DescriptorAttr::from_bits_truncate(self.0))
            .field("flags", &self.pt_flags())
            .finish()
    }
}

pub struct S2PTInstr;

impl PagingInstr for S2PTInstr {
    unsafe fn activate(root_paddr: HostPhysAddr) {
        info!("guest stage2 PT activate");
        unsafe {
            let mut bits: usize = 0;
            let mode: usize = 8;    // Mode::Sv39x4
                                    // 设置为 0/9/10 都没有问题，设置为 8 会出现问题, 疑惑, 为什么开了 Sv48x4 或者 Sv57x4, 则能够执行一点呢？ 有问题啊
            let vmid: usize = 0;
            bits.set_bits(60..64, mode as usize);
            bits.set_bits(44..58, vmid);
            // 设置 root_paddr
            bits.set_bits(0..44, root_paddr >> 12);
            info!("HGATP: {:#x?}", bits);
            write_csr!(CSR_HGATP, bits);
            let hgatp: usize = read_csr!(CSR_HGATP);
            info!("HGATP after activation: {:#x?}", hgatp);
            // core::arch::asm!("hfence.gvma");                            //not supported in rust
        }
    }

    fn flush(_vaddr: Option<usize>) {
        // do nothing
    }
}

pub type Stage2PageTable = Level3PageTable<HostPhysAddr, PageTableEntry, S2PTInstr>;
