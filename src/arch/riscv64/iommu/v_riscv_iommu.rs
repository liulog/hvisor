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
//      Jingyu Liu <liujingyu24s@ict.ac.cn

use crate::arch::iommu::viommu_ddt_emul_handler;
use riscv_h::asm;
use crate::arch::zone;
use crate::{arch::iommu::host_iommu, memory::GuestPhysAddr, percpu::this_cpu_data};

use super::riscv_iommu::*;
use spin::Mutex;
use alloc::sync::Arc;
use crate::percpu::this_zone;
use crate::memory::MemoryRegion;
use crate::zone::Zone;

/// Virtual IOMMU device structure
#[allow(unused)]
pub struct VirtualIommu {
    /// Multithread safe inner structure
    inner: Arc<Mutex<VirtualIommuInner>>,
}

/// Virtual IOMMU
struct VirtualIommuInner {
    pub caps: u64,                               // Capabilities Register
    pub fctl: u32,                  // Features-control Register
    pub ddtp: u64,                  // Device Directory Table Pointer
    /// Command Queue
    pub cqb: u64,                   // Command Queue Base
    pub cqh: u32,                   // Command Queue Head
    pub cqt: u32,                   // Command Queue Tail
    /// Fault Queue
    pub fqb: u64,                   // Fault Queue Base
    pub fqh: u32,                   // Fault Queue Head
    pub fqt: u32,                   // Fault Queue Tail
    /// Page-request Queue
    pub pqb: u64,                   // Page-request Queue Base
    pub pqh: u32,                   // Page-request Queue Head
    pub pqt: u32,                   // Page-request Queue Tail
    pub cqcsr: u32,                 // Command Queue CSR
    pub fqcsr: u32,                 // Fault Queue CSR
    pub pqcsr: u32,                 // Page-request Queue CSR
    pub ipsr: u32,                  // Interrupt Pending and Status Register
    /// HPM
    // iocntovf: u32,              // HPM Counter Overflows
    // iocntinh: u32,              // HPM Counter Inhibits
    // iohpmcycles: u64,           // HPM Cycle Counter
    // iohpmctr: [u64; 31],        // HPM Event Counters
    // iohpmevt: [u64; 31],        // HPM Event Selector
    /// DBG
    // tr_req_iova: u64,           // Translation-request IOVA
    // tr_req_ctl: u64,            // Translation-request Control
    // tr_response: u64,           // Translation-request Response
    pub icvec: u64,                 // Interrupt Control and Vector Register
    // msi_cfg_tbl: [MsiCfgTbl; 16],
}

impl VirtualIommu {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VirtualIommuInner::new())),
        }
    }

    /// vIOMMU emul access.
    pub fn viommu_emul_access(
        &self,
        zone: &mut Zone,
        offset: usize,
        size: usize,
        value: usize,
        is_write: bool,
    ) -> u64 {
        self.inner.lock().viommu_emul_access(zone, offset, size, value, is_write)
    }

    /// vIOMMU emul access.
    pub fn viommu_ddt_emul_access(
        &self,
        offset: usize,
        size: usize,
        value: usize,
        is_write: bool,
    ) -> u64 {
        self.inner.lock().viommu_ddt_emul_access(offset, size, value, is_write)
    }
}

impl VirtualIommuInner {
    pub fn new() -> Self {
        info!("Create a new vIOMMU instance.");
        info!("vIOMMU doesn't support Stage-2, MSI...");
        let mut caps = host_iommu().read().rv_iommu_regmap.caps.get();
        caps = caps & !(RV_IOMMU_CAPS_SV32X4_BIT | RV_IOMMU_CAPS_SV39X4_BIT | RV_IOMMU_CAPS_SV48X4_BIT | RV_IOMMU_CAPS_SV57X4_BIT);
        caps = caps & !(RV_IOMMU_CAPS_MSI_FLAT_BIT | RV_IOMMU_CAPS_MSI_MRIF_BIT);
        Self {
            caps: caps,
            fctl: 0,
            ddtp: 0,
            cqb: 0,
            cqh: 0,
            cqt: 0,
            fqb: 0,
            fqh: 0,
            fqt: 0,
            pqb: 0,
            pqh: 0,
            pqt: 0,
            cqcsr: 0,
            fqcsr: 0,
            pqcsr: 0,
            ipsr: 0,
            icvec: 0,
        }
    }

    /// vIOMMU emul access inner.
    pub fn viommu_emul_access(
        &mut self,
        zone: &mut Zone,
        offset: usize,
        size: usize,
        value: usize,
        is_write: bool,
    ) -> u64 {
        match offset {
            0x0..=0x7 => { // Capabilities
                if is_write {
                    error!("Capabilities register is read-only!");
                } else {
                    return self.caps;
                }
            }
            0x8..=0xB => {
                if is_write { // fctl
                    let host_fctl = host_iommu().read().rv_iommu_regmap.fctl.get();
                    // Only support the same fctl value as host IOMMU.
                    if value != host_fctl as usize {
                        error!("vIOMMU fctl write value {:#x} not match host fctl {:#x}!", value, host_fctl);
                    } else {
                        self.fctl = value as u32;
                    }
                } else {
                    return self.fctl as u64;
                }
            }
            0x10..=0x17 => { // ddtp
                if is_write {
                    info!("vIOMMU ddtp access, is_write: {}, value: {:#x}", is_write, value);
                    let viommu_mode = value & RV_IOMMU_DDTP_MODE_MASK;
                    match IommuMode::try_from(viommu_mode) {
                        Ok(mode) => {
                            info!("Guest try to set vIOMMU mode to {:?}", mode);
                            match mode {
                                IommuMode::Off | IommuMode::Bare => {
                                    self.ddtp = value as u64;
                                }
                                IommuMode::Ddt1Lvl => {
                                    let ppn = value & RV_IOMMU_DDTP_PPN_MASK;
                                    let ddt_gpa = ppn << 2;
                                    info!("vIOMMU's DDT Table GPA: {:#x}", ddt_gpa);
                                    // We unmap this page to trigger a page fault when the guest accesses it.
                                    // let gpm = zone.get_gpm();
                                    // match gpm.get_region(ddt_gpa as GuestPhysAddr) {
                                    //     // unmap this region 4K
                                    //     Some(region) => {
                                    //         gpm.delete(region.start).unwrap();
                                    //         let region_start = region.start;
                                    //         let region_end = region.start + region.size;
                                    //         if region_start < ddt_gpa {
                                    //             gpm.insert(MemoryRegion::new_with_offset_mapper(
                                    //                 region_start as GuestPhysAddr,
                                    //                 region_start,
                                    //                 (ddt_gpa - region_start) as usize,
                                    //                 region.flags,
                                    //             )).unwrap();
                                    //         }
                                    //         if region_end > ddt_gpa + 0x1000 {  // For 1LVL DDT, size is 4K
                                    //             gpm.insert(MemoryRegion::new_with_offset_mapper(
                                    //                 (ddt_gpa + 0x1000) as GuestPhysAddr,
                                    //                 ddt_gpa + 0x1000,
                                    //                 region_end - (ddt_gpa + 0x1000),
                                    //                 region.flags,
                                    //             )).unwrap();
                                    //         }
                                    //         info!("gpm after unmap vIOMMU ddtp region: {:#x?}", gpm);
                                    //         unsafe {riscv_h::asm::hfence_gvma(0, 0) };
                                    //         // TODO: hfence.gvma and send IPI to other CPUs
                                    //         zone.mmio_region_register(ddt_gpa, 0x1000, viommu_ddt_emul_handler, 0);
                                    //     }
                                    //     None => {
                                    //         error!("vIOMMU ddtp region not found in gpm!");
                                    //         return 0;
                                    //     }
                                    // }
                                    // self.ddtp = value as u64;
                                }
                                _ => {
                                    warn!("vIOMMU ddtp mode {:?} not supported!", mode);
                                }
                            }
                        }
                        Err(_) => {
                            error!("vIOMMU ddtp mode {:#x} not supported!", viommu_mode);
                        }
                    }
                } else {
                    return self.ddtp;
                }
            }
            0x18..=0x1F => { // cqb
                info!("vIOMMU cqb access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.cqb.set(value as u64);
                    self.cqb = value as u64;
                } else {
                    return host_iommu().write().rv_iommu_regmap.cqb.get() as u64;
                }
            }
            0x20..=0x23 => { // cqh
                info!("vIOMMU cqh access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.cqh.set(value as u32);
                    self.cqh = value as u32;
                } else {
                    return host_iommu().write().rv_iommu_regmap.cqh.get() as u64;
                }
            }
            0x24..=0x27 => { // cqt
                info!("vIOMMU cqt access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.cqt.set(value as u32);
                    self.cqh = value as u32;
                } else {
                    return host_iommu().write().rv_iommu_regmap.cqt.get() as u64;
                }
            }
            0x28..=0x2F => { // fqb
                info!("vIOMMU fqb access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.fqb.set(value as u64);
                    self.fqb = value as u64;
                    info!("vIOMMU fqb set to {:#x}", value);
                } else {
                    return host_iommu().write().rv_iommu_regmap.fqb.get() as u64;
                }
            }
            0x30..=0x33 => { // fqh
                info!("vIOMMU fqh access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.fqh.set(value as u32);
                    self.fqh = value as u32;
                } else {
                    return host_iommu().write().rv_iommu_regmap.fqh.get() as u64;
                }
            }
            0x34..=0x37 => { // fqt
                info!("vIOMMU fqt access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.fqt.set(value as u32);
                    self.fqt = value as u32;
                } else {
                    return host_iommu().write().rv_iommu_regmap.fqt.get() as u64;
                }
            }
            0x48..=0x4B => { // cqcsr
                info!("vIOMMU cqcsr access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.cqcsr.set(value as u32);
                } else {
                    return host_iommu().write().rv_iommu_regmap.cqcsr.get() as u64;
                }
            }
            0x4C..=0x4F => { // fqcsr
                info!("vIOMMU fqcsr access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.fqcsr.set(value as u32);
                } else {
                    return host_iommu().read().rv_iommu_regmap.fqcsr.get() as u64;
                }
            }
            0x54..=0x57 => { // ipsr
                info!("vIOMMU ipsr access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.ipsr.set(value as u32);
                } else {
                    return host_iommu().read().rv_iommu_regmap.ipsr.get() as u64;
                }
            }
            0x2f8..=0x2ff => { // icvec
                info!("vIOMMU icvec access, is_write: {}, value: {:#x}", is_write, value);
                if is_write {
                    host_iommu().write().rv_iommu_regmap.icvec.set(value as u64);
                } else {
                    return host_iommu().read().rv_iommu_regmap.icvec.get() as u64;
                }
            }
            _ => {
                error!("IOMMU mmio access offset {:#x} not supported!", offset);
            }
        }
        0
    }

    /// Handle Zone's iommu ddt mmio access. (now only support 1LVL DDT)
    pub fn viommu_ddt_emul_access(
        &mut self,
        offset: usize,
        size: usize,
        value: usize,
        is_write: bool,
    ) -> u64 {
        // A hypervisor that provides such an emulated IOMMU to the guest may 
        //      1.retain control of the MSI page tables used to direct MSIs to guest interrupt files
        //      2.clear the MSI_FLAT and MSI_MRIF fields of the emulated capabilities register.

        // So its Device-Context Format is below:
        //      - First-stage-context (fsc) 8bytes
        //      - Transiation-attributes (ta) 8bytes
        //      - IO Hypervisor guest address translation and protection (iohgatp) 8bytes
        //      - Translation Control (tc) 4bytes

        let ddt_index = offset / 32; // each entry is 32 bytes (base format)
        match offset % 32 {
            0x0..=0x7 => { // fsc
                // let mode = ((value as u64 & RV_IOMMU_FSC_MODE_MASK) >> 60) as usize;
                // let ppn = (value as u64) & RV_IOMMU_FSC_PPN_MASK;
                // match IosatpMode::try_from(mode as usize) {
                //     Ok(m) => {
                //         info!("vIOMMU ddt entry {} fsc set to mode: {:?}, addr: {:#x}", ddt_index, m, ppn << 12);
                //         // host_iommu().write().rv_iommu_write_ddt_fsc(ddt_index, m, ppn);
                //     }
                //     Err(_) => {
                //         error!("vIOMMU ddt entry {} fsc mode {:#x} not supported!", ddt_index, mode);
                //         return 0;
                //     }
                // }
                info!("vIOMMU ddt entry {} fsc access, is_write: {}, value: {:#x}", ddt_index, is_write, value);
            }
            0x8..=0xF => { // ta
                info!("vIOMMU ddt entry {} ta access, is_write: {}, value: {:#x}", ddt_index, is_write, value);
            }
            0x10..=0x17 => { // iohgatp
                // Note: we only support first level translation for guest in caps register.
                if is_write {
                    error!("DDT Entry iohgatp register can not be modified by guest!");
                } else {
                    // return 0;
                }
            }
            0x18..=0x1F => { // tc
                info!("vIOMMU ddt entry {} tc access, is_write: {}, value: {:#x}", ddt_index, is_write, value);
            }
            _ => {
                error!("Unexpected offset value: {:#x}. This should never happen!", offset);
            }
        }
        0
    }
}



