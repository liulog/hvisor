// Copyright (c) 2025 Syswonder
// hvisor is licensed under Mulan PSL v2.
// You can use this software according to the terms and conditions of the Mulan PSL v2.
// You may obtain a copy of Mulan PSL v2 at:
//     http://license.coscl.org.cn/MulanPSL2
// THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY KIND, EITHER
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT, MERCHANTABILITY OR
// FIT FOR A PARTICULAR PURPOSE.
// See the Mulan PSL v2 for more details.
//
// Syswonder Website:
//      https://www.syswonder.org
//
// Authors:
//      Jingyu Liu <liujingyu24s@ict.ac.cn>
//

#![allow(unused)]

use crate::{arch::zone::HvArchZoneConfig, config::*};

/// Platform Hardware Configuration
pub const BOARD_NAME: &str = "qem-plic";

pub const BOARD_NCPUS: usize = 4;
#[rustfmt::skip]
pub static BOARD_HARTID_MAP: [usize; BOARD_NCPUS] = [
    0x0,            // core0   \
    0x1,            // core1    | -> cluster0 -> CPU
    0x2,            // core2    |
    0x3,            // core3   / 
];

pub const ACLINT_SSWI_BASE: usize = 0x2F00000;

pub const PLIC_BASE: usize = 0xc000000;
pub const PLIC_SIZE: usize = 0x4000000;
pub const BOARD_PLIC_INTERRUPTS_NUM: usize = 1023; // except irq 0
pub const NUM_CONTEXTS_PER_HART: usize = 2; // M-mode、S-mode

pub const TIMEBASE_FREQ: u64 = 10_000_000; // 10MHz

pub const IOMMU_SYS_BASE: usize = 0x3010000;
pub const IOMMU_SYS_SIZE: usize = 0x1000;
pub const SIFIVE_TEST_BASE: u64 = 0x100000; // This device is used for qemu-quit.

pub const PCI_ECAM_BASE: usize = 0x30000000;
pub const PCI_ECAM_SIZE: usize = 0x10000000;
pub const PCI_MEM32_BASE: usize = 0x4000_0000;
pub const PCI_MEM32_SIZE: usize = 0x4000_0000;
pub const PCI_MEM64_BASE: usize = 0x4_0000_0000;
pub const PCI_MEM64_SIZE: usize = 0x4_0000_0000;

/// Root Zone Configuration
pub const ROOT_ZONE_NAME: &str = "root-linux";
// ROOT_ZONE_DTB_ADDR is HPA (Host physical Address).
pub const ROOT_ZONE_DTB_ADDR: u64 = 0x8F000000;
// ROOT_ZONE_KERNEL_ADDR is HPA (Host Physical Address), but it isn't used now.
pub const ROOT_ZONE_KERNEL_ADDR: u64 = 0x90000000;
// ROOT_ZONE_ENTRY is GPA (Guest Physical Address).
pub const ROOT_ZONE_ENTRY: u64 = 0x90000000;
pub const ROOT_ZONE_CPUS: u64 = (1 << 0) | (1 << 1);

pub const ROOT_ZONE_MEMORY_REGIONS: &[HvConfigMemoryRegion] = &[
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x83000000,
        virtual_start: 0x83000000,
        size: 0x7D000000,
    }, // ram
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x10000000,
        virtual_start: 0x10000000,
        size: 0x1000,
    }, // serial
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x30000000,
        virtual_start: 0x30000000,
        size: 0x10000000,
    }, // pci
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x10001000,
        virtual_start: 0x10001000,
        size: 0x1000,
    }, // virtio
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x10002000,
        virtual_start: 0x10002000,
        size: 0x1000,
    }, // virtio
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x10003000,
        virtual_start: 0x10003000,
        size: 0x1000,
    }, // virtio
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x10004000,
        virtual_start: 0x10004000,
        size: 0x1000,
    }, // virtio
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x10005000,
        virtual_start: 0x10005000,
        size: 0x1000,
    }, // virtio
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x10008000,
        virtual_start: 0x10008000,
        size: 0x1000,
    }, // virtio
];

// Note: all here's irqs are hardware irqs,
//  only these irq can be transferred to the physical PLIC.
// If you have one irq_iq = 0x20, you should modify IRQ_WAKEUP_VIRTIO_DEVICE(default 0x20) in virtio_trampoline.rs to avoid 0x20.
#[rustfmt::skip]
pub const HW_IRQS: &[u32] = &[
    // 0x6, 0x7, // virtio-mmio
    0x8, // virtio-mmio
    0xA, // uart0
    // 0x20, 0x21, 0x22, 0x23, // pci/pcie
];

// irqs belong to the root zone.
#[rustfmt::skip]
pub const ROOT_ZONE_IRQS: &[u32] = &[
    0x8,        // virtio-mmio
    0xA,        // uart0
    // 0x20,    // pci pinA
    // 0x21,    // pci pinB
    // 0x22,    // pci pinC
    // 0x23,    // pci pinD
];

pub const ROOT_ARCH_ZONE_CONFIG: HvArchZoneConfig = HvArchZoneConfig {
    plic_base: PLIC_BASE,
    plic_size: PLIC_SIZE,
    aplic_base: 0x0, // Unused here
    aplic_size: 0x0, // Unused here
};
