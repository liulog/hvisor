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

use super::consts::IOMMU_CQ_ENTRY_SIZE;
use super::iommu_add_command;
use super::viommu_ddt_emul_handler;
use super::IOMMU;
use crate::arch::ipi::wait_for_other_cpus_suspend;
use crate::consts::{IPI_EVENT_HART_RESUME, IPI_EVENT_HART_SUSPEND};
use crate::event::send_event_to_all;
use crate::memory::GuestPhysAddr;

use riscv::asm::fence;

use super::cmd_queue::*;
use super::regs::*;
use crate::consts::MAX_ZONE_NUM;
use crate::memory::MemoryRegion;
use crate::zone::Zone;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr;
use spin::Mutex;

// The MAX_ZONE_NUM should be the power of 2.
lazy_static! {
    pub static ref VIOMMU_ARR: Mutex<Vec<Option<Arc<VirtualIommu>>>> =
        Mutex::new(vec![None; MAX_ZONE_NUM]);
}

/// Virtual IOMMU device structure
pub struct VirtualIommu {
    /// Multithread safe inner structure
    inner: Mutex<VirtualIommuInner>,
}

/// Virtual IOMMU
struct VirtualIommuInner {
    pub caps: u64, // Capabilities Register
    pub fctl: u32, // Features-control Register
    pub ddtp: u64, // Device Directory Table Pointer
    /// Command Queue
    pub cqb: u64, // Command Queue Base
    pub cqh: u32,  // Command Queue Head
    pub cqt: u32,  // Command Queue Tail
    pub cq_num_entrys: u32,
    pub cq_gpa: u64,
    /// Fault Queue
    pub fqb: u64, // Fault Queue Base
    pub fqh: u32,     // Fault Queue Head
    pub fqt: u32,     // Fault Queue Tail
    pub tc: Vec<u64>, // per tc attached to one device
}

impl VirtualIommu {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(VirtualIommuInner::new()),
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
        self.inner
            .lock()
            .viommu_emul_access(zone, offset, size, value, is_write)
    }

    /// vIOMMU emul access.
    pub fn viommu_ddt_emul_access(
        &self,
        zone: &Zone,
        offset: usize,
        size: usize,
        value: usize,
        is_write: bool,
    ) -> u64 {
        self.inner
            .lock()
            .viommu_ddt_emul_access(zone, offset, size, value, is_write)
    }
}

impl VirtualIommuInner {
    pub fn new() -> Self {
        info!("Create a new vIOMMU instance.");
        info!("vIOMMU doesn't support Stage-2, MSI,PASID");
        let mut caps = IOMMU.lock().rv_iommu_regmap.caps.get();
        caps = caps
            & !(RV_IOMMU_CAPS_SV32X4_BIT
                | RV_IOMMU_CAPS_SV39X4_BIT
                | RV_IOMMU_CAPS_SV48X4_BIT
                | RV_IOMMU_CAPS_SV57X4_BIT);
        caps = caps & !(RV_IOMMU_CAPS_MSI_FLAT_BIT | RV_IOMMU_CAPS_MSI_MRIF_BIT);
        caps = caps & !(RV_IOMMU_CAPS_PD8_BIT | RV_IOMMU_CAPS_PD17_BIT | RV_IOMMU_CAPS_PD20_BIT);
        caps = caps
            & !(RV_IOMMU_CAPS_ATS_BIT
                | RV_IOMMU_CAPS_SVPBMT_BIT
                | RV_IOMMU_CAPS_T2GPA_BIT
                | RV_IOMMU_CAPS_HPM_BIT
                | RV_IOMMU_CAPS_DBG_BIT
                | RV_IOMMU_CAPS_END_BIT);
        Self {
            caps: caps,
            fctl: 0,
            ddtp: 0,
            cqb: 0,
            cqh: 0,
            cqt: 0,
            cq_num_entrys: 0,
            cq_gpa: 0,
            fqb: 0,
            fqh: 0,
            fqt: 0,
            tc: vec![0; 64], // current support max 64 devices
        }
    }

    /// vIOMMU emul access inner.
    pub fn viommu_emul_access(
        &mut self,
        zone: &mut Zone,
        offset: usize,
        _size: usize,
        value: usize,
        is_write: bool,
    ) -> u64 {
        // Jump those devices don't belong to this zone.
        match offset {
            0x0..=0x7 => {
                // Capabilities
                if is_write {
                    error!("Capabilities register is read-only!");
                } else {
                    return self.caps;
                }
            }
            0x8..=0xB => {
                if is_write {
                    // fctl
                    let host_fctl = IOMMU.lock().rv_iommu_regmap.fctl.get();
                    // Only support the same fctl value as host IOMMU.
                    if value != host_fctl as usize {
                        error!(
                            "vIOMMU fctl write value {:#x} not match host fctl {:#x}!",
                            value, host_fctl
                        );
                    } else {
                        self.fctl = value as u32;
                    }
                } else {
                    return self.fctl as u64;
                }
            }
            0x10..=0x17 => {
                // ddtp
                if is_write {
                    info!(
                        "vIOMMU ddtp access, is_write: {}, value: {:#x}",
                        is_write, value
                    );
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
                                    let cpu_set = zone.cpu_set;
                                    // Send IPI to signal other CPUs.
                                    send_event_to_all(cpu_set, 0, IPI_EVENT_HART_SUSPEND);
                                    fence();
                                    // Wait for other CPUs suspend.
                                    wait_for_other_cpus_suspend(cpu_set);
                                    let gpm = zone.get_gpm();
                                    match gpm.get_region(ddt_gpa as GuestPhysAddr) {
                                        // unmap this region 4K
                                        Some(region) => {
                                            gpm.delete(region.start).unwrap();
                                            let region_start = region.start;
                                            let region_end = region.start + region.size;
                                            if region_start < ddt_gpa {
                                                gpm.insert(MemoryRegion::new_with_offset_mapper(
                                                    region_start as GuestPhysAddr,
                                                    region_start,
                                                    (ddt_gpa - region_start) as usize,
                                                    region.flags,
                                                ))
                                                .unwrap();
                                            }
                                            if region_end > ddt_gpa + 0x1000 {
                                                // For 1LVL DDT, size is 4K
                                                gpm.insert(MemoryRegion::new_with_offset_mapper(
                                                    (ddt_gpa + 0x1000) as GuestPhysAddr,
                                                    ddt_gpa + 0x1000,
                                                    region_end - (ddt_gpa + 0x1000),
                                                    region.flags,
                                                ))
                                                .unwrap();
                                            }
                                            info!(
                                                "gpm after unmap vIOMMU ddtp region: {:#x?}",
                                                gpm
                                            );
                                            unsafe { riscv_h::asm::hfence_gvma(0, 0) };
                                            // TODO: hfence.gvma and send IPI to other CPUs
                                            zone.mmio_region_register(
                                                ddt_gpa,
                                                0x1000,
                                                viommu_ddt_emul_handler,
                                                0,
                                            );
                                        }
                                        None => {
                                            error!("vIOMMU ddtp region not found in gpm!");
                                            return 0;
                                        }
                                    }
                                    send_event_to_all(zone.cpu_set, 0, IPI_EVENT_HART_RESUME);
                                    self.ddtp = value as u64;
                                }
                                _ => {
                                    warn!("vIOMMU ddtp mode {:?} not supported!", mode);
                                }
                            }
                        }
                        Ok(IommuMode::Ddt2Lvl) | Ok(IommuMode::Ddt3Lvl) => {
                            unimplemented!("vIOMMU ddtp mode {:?} not supported yet!", viommu_mode);
                        }
                        Err(_) => {
                            error!("vIOMMU ddtp mode {:#x} not supported!", viommu_mode);
                        }
                    }
                } else {
                    return self.ddtp;
                }
            }
            // Command Queue Related Registers
            0x18..=0x1F => {
                // Command Queue Base (readable & writeable)
                if is_write {
                    // IOMMU.lock().rv_iommu_regmap.cqb.set(value as u64);
                    // self.cqb = value as u64;
                    let ppn = value as u64 & (0xFFF_FFFF_FFFFu64 << 10);
                    let log2sz_1 = value & 0x1F;
                    let mut num_entrys: u64 = 1 << (log2sz_1 + 1);
                    info!("vIOMMU cqb set to {:#x}", value);
                    info!("vIOMMU cqb ppn: {:#x}, lof2sz_1: {:#x}", ppn, log2sz_1);
                    info!("vIOMMU cqb num_entrys: {}", num_entrys);
                    let mut new_value = value;
                    // Don't support more than 256 entrys now.
                    if num_entrys > 256 {
                        info!(
                            "vIOMMU cqb num_entrys {} too large, limit it to 256.",
                            num_entrys
                        );
                        new_value = (value & !0x1F) | 0x7; // 2^(7+1) = 256, set log2sz_1 to 7
                        num_entrys = 256;
                    }
                    // One-Page Command Queue
                    let cqb_gpa = (ppn << 2) as usize;
                    self.cq_num_entrys = num_entrys as u32;
                    self.cq_gpa = cqb_gpa as u64;
                    info!("vIOMMU's Command Queue GPA: {:#x}", cqb_gpa);
                    self.cqb = new_value as u64;
                } else {
                    info!("vIOMMU cqb read, value: {:#x}", self.cqb);
                    return self.cqb;
                }
            }
            0x20..=0x23 => {
                // Command Queue Head (read-only)
                if is_write {
                    unreachable!("vIOMMU cqh is read-only!");
                } else {
                    return self.cqh as u64;
                }
            }
            0x24..=0x27 => {
                // Command Queue Tail (readable & writeable)
                if is_write {
                    let value = value as u32 % self.cq_num_entrys;

                    if value == self.cqh % self.cq_num_entrys {
                        return 0;
                    }
                    // Handle all new commands.
                    let mut cqh = self.cqh;
                    while cqh != value {
                        // info!("vIOMMU fetch command at cqh: {} value {}", cqh, value);

                        let cqe_addr = (self.cq_gpa as usize)
                            + (cqh as usize % self.cq_num_entrys as usize)
                                * IOMMU_CQ_ENTRY_SIZE as usize;
                        let bytes: [u8; 16] = unsafe { ptr::read(cqe_addr as *const [u8; 16]) };
                        let command = CqEntry::from_bytes(bytes).parse();
                        match command {
                            Command::IotInval(cmd) => match cmd.func3() {
                                IotInvalFunc::Vma => {
                                    if cmd.gv() != 0 {
                                        unreachable!("vIOMMU IOTINVAL VMA GV=1 not supported! (GSCID is not visible to guest)");
                                    }
                                    // info!(
                                    //     "vIOMMU IOTINVAL VMA Command: pscid {}, pscv {}, av {}",
                                    //     cmd.pscid(),
                                    //     cmd.pscv(),
                                    //     cmd.av(),
                                    // );
                                    // Reference:
                                    //  IOMMU Spec 6.3.5. Changing first-stage page table entry
                                    let mut new_cmd = cmd;
                                    new_cmd.set_gv(1);
                                    new_cmd.set_gscid(zone.id as u16);
                                    iommu_add_command(new_cmd.into());
                                }
                                IotInvalFunc::Gvma => {
                                    unimplemented!("vIOMMU IOTINVAL GVMA not supported yet!");
                                }
                            },
                            Command::IoFence(cmd) => {
                                // Most of the time, the guest may add the Iofence.C command after other commands.
                                // Carefully, we don't support Iofence.C to generate interrupts (WSI or MSI).
                                match cmd.func3_or_err() {
                                    Ok(IoFenceFunc::C) => {
                                        if cmd.av() != 0 {
                                            // It is used for MSI interrupt.
                                            unimplemented!(
                                                "vIOMMU IOFENCE.C AV=1 not supported yet!"
                                            );
                                        }
                                        if cmd.wsi() != 0 {
                                            unimplemented!(
                                                "vIOMMU IOFENCE.C WSI=1 not supported yet!"
                                            );
                                        }
                                        // info!(
                                        //     "vIOMMU IOFENCE.C Command: pr {}, pw {}",
                                        //     cmd.pr(),
                                        //     cmd.pw(),
                                        // );
                                        iommu_add_command(cmd.into());
                                    }
                                    Err(_) => {
                                        unimplemented!("vIOMMU IOFENCE None not supported yet!");
                                    }
                                }
                            }
                            Command::IoDir(cmd) => {
                                // info!("vIOMMU IODIR Command");
                                match cmd.func3_or_err() {
                                    Ok(IoDirFunc::InvalDdt) => {
                                        let did = cmd.did();
                                        let dv = cmd.dv();
                                        if (dv == 1 && did == 0)
                                            || zone.pciroot.is_assigned_device(did as _) == false
                                        {
                                            warn!("vIOMMU IODIR INVAL_DDT Command: did {} not belong to this zone!", did);
                                        } else {
                                            // info!(
                                            //     "vIOMMU IODIR INVAL_DDT Command: did {}, dv {}",
                                            //     did, dv,
                                            // );
                                            if dv == 1 {
                                                iommu_add_command(cmd.into());
                                                let iohgatp = IOMMU
                                                    .lock()
                                                    .rv_iommu_read_ddt_iohgatp(did as _);
                                                let iohgatp_mode =
                                                    (iohgatp & RV_IOMMU_DC_IOHGATP_MODE_MASK) >> 60;
                                                if iohgatp_mode == IosatpMode::Bare as u64 {
                                                    unimplemented!("vIOMMU IODIR DV=1 with iohgatp mode Bare not supported yet!");
                                                }
                                                let gscid = (iohgatp
                                                    & RV_IOMMU_DC_IOHGATP_GSCID_MASK)
                                                    >> 44;
                                                if gscid != zone.id as u64 {
                                                    unreachable!("vIOMMU IODIR DV=1 with gscid {} not match current zone id {}!", gscid, zone.id);
                                                }

                                                // Here simply invalid all the entries related to GSCID.
                                                // Optimization can be carried out with a smaller granularity.
                                                iommu_add_command(
                                                    CqEntryIotinval::new()
                                                        .with_opcode(OpCode::IotInval)
                                                        .with_func3(IotInvalFunc::Vma)
                                                        .with_gv(1)
                                                        .with_av(0)
                                                        .with_pscv(0)
                                                        .with_gscid(gscid as _)
                                                        .into(),
                                                );

                                                iommu_add_command(
                                                    CqEntryIotinval::new()
                                                        .with_opcode(OpCode::IotInval)
                                                        .with_func3(IotInvalFunc::Gvma)
                                                        .with_gv(1)
                                                        .with_av(0)
                                                        .with_gscid(gscid as _)
                                                        .into(),
                                                );
                                            } else if dv == 0 {
                                                // Here simply invalid all the entries in DDT.
                                                // Optimization can be carried out with a smaller granularity.
                                                iommu_add_command(cmd.into());
                                            } else {
                                                unimplemented!("vIOMMU IODIR DV bit invalid!");
                                            }
                                        }
                                    }
                                    Ok(IoDirFunc::InvalPdt) => {
                                        unimplemented!("vIOMMU IODIR INVAL_PDT not supported yet!");
                                    }
                                    Err(_) => {
                                        unimplemented!("vIOMMU IODIR None not supported yet!");
                                    }
                                }
                            }
                            Command::Ats(_cmd) => {
                                unimplemented!("vIOMMU ATS Command not supported yet!");
                            }
                            Command::Unknown => {
                                unimplemented!("vIOMMU Unknown Command!");
                            }
                        }
                        cqh = (cqh + 1) % self.cq_num_entrys;
                    }
                    // Update cqt and cqh, fetch all commands in command queue.
                    self.cqt = value as u32 % self.cq_num_entrys;
                    self.cqh = self.cqt;
                } else {
                    return self.cqt as u64;
                }
            }
            // Fault Queue Related Registers
            0x28..=0x2F => {
                // fqb
                if is_write {
                    IOMMU.lock().rv_iommu_regmap.fqb.set(value as u64);
                    self.fqb = value as u64;
                    info!("vIOMMU fqb set to {:#x}", value);
                } else {
                    return IOMMU.lock().rv_iommu_regmap.fqb.get() as u64;
                }
            }
            0x30..=0x33 => {
                // fqh
                if is_write {
                    IOMMU.lock().rv_iommu_regmap.fqh.set(value as u32);
                    self.fqh = value as u32;
                } else {
                    return IOMMU.lock().rv_iommu_regmap.fqh.get() as u64;
                }
            }
            0x34..=0x37 => {
                // fqt
                if is_write {
                    IOMMU.lock().rv_iommu_regmap.fqt.set(value as u32);
                    self.fqt = value as u32;
                } else {
                    return IOMMU.lock().rv_iommu_regmap.fqt.get() as u64;
                }
            }
            0x48..=0x4B => {
                // cqcsr
                if is_write {
                    IOMMU.lock().rv_iommu_regmap.cqcsr.set(value as u32);
                } else {
                    return IOMMU.lock().rv_iommu_regmap.cqcsr.get() as u64;
                }
            }
            0x4C..=0x4F => {
                // fqcsr
                if is_write {
                    IOMMU.lock().rv_iommu_regmap.fqcsr.set(value as u32);
                } else {
                    return IOMMU.lock().rv_iommu_regmap.fqcsr.get() as u64;
                }
            }
            0x54..=0x57 => {
                // ipsr
                if is_write {
                    IOMMU.lock().rv_iommu_regmap.ipsr.set(value as u32);
                } else {
                    return IOMMU.lock().rv_iommu_regmap.ipsr.get() as u64;
                }
            }
            0x2f8..=0x2ff => {
                // icvec
                if is_write {
                    IOMMU.lock().rv_iommu_regmap.icvec.set(value as u64);
                } else {
                    return IOMMU.lock().rv_iommu_regmap.icvec.get() as u64;
                }
            }
            _ => {
                unimplemented!("IOMMU mmio access offset {:#x} not supported!", offset);
            }
        }
        0
    }

    /// Handle Zone's iommu ddt mmio access. (now only support 1LVL DDT)
    pub fn viommu_ddt_emul_access(
        &mut self,
        zone: &Zone,
        offset: usize,
        _size: usize,
        value: usize,
        is_write: bool,
    ) -> u64 {
        // A hypervisor that provides such an emulated IOMMU to the guest may
        //      1.retain control of the MSI page tables used to direct MSIs to guest interrupt files
        //      2.clear the MSI_FLAT and MSI_MRIF fields of the emulated capabilities register.

        // So its Device-Context Format is below:
        //      - Translation Control (tc) 8bytes
        //      - IO Hypervisor guest address translation and protection (iohgatp) 8bytes
        //      - Transiation-attributes (ta) 8bytes
        //      - First-stage-context (fsc) 8bytes

        let ddt_index = offset / 32; // each entry is 32 bytes (base format)

        // Don't allow to access those devices not belong to this zone.
        if ddt_index == 0 || zone.pciroot.is_assigned_device(ddt_index as _) == false {
            warn!(
                "bdf: {} not belong to zone {}, ignore the access!",
                ddt_index, zone.id
            );
            return 0;
        }

        match offset % 32 {
            0x0..=0x7 => {
                // tc
                if is_write {
                    if value & 0xFFFF_FFFEusize != 0 {
                        unimplemented!(
                            "vIOMMU ddt entry {} tc value {:#x} not supported!",
                            ddt_index,
                            value
                        );
                    }
                    self.tc[ddt_index] = value as u64;
                } else {
                    return self.tc[ddt_index];
                }
            }
            0x8..=0xF => {
                // iohgatp
                // Note: we only support first level translation for guest in caps register.
                if is_write {
                    if value != 0 {
                        error!(
                            "vIOMMU ddt entry {} iohgatp value {:#x} not supported!",
                            ddt_index, value
                        );
                    }
                } else {
                    return 0;
                }
            }
            0x10..=0x17 => {
                // ta
                // The PSCID filed in ta is used as the address-space ID
                //      if DC.tc.PDTV is 0 and the iosatp.MODE field is not Bare.
                // When DC.tc.PDTV is 1, the PSCID filed in ta is ignored.
                if is_write {
                    // if value != 0 {
                    // error!("vIOMMU ddt entry {} ta value {:#x} not supported!", ddt_index, value);
                    // }
                    IOMMU.lock().rv_iommu_write_ddt_ta(ddt_index, value as u64);
                } else {
                    error!("vIOMMU ddt entry {} ta read not supported!", ddt_index);
                }
            }
            0x18..=0x1F => {
                // fsc
                if is_write {
                    let mode = ((value as u64 & RV_IOMMU_FSC_MODE_MASK) >> 60) as usize;
                    let ppn = (value as u64) & RV_IOMMU_FSC_PPN_MASK;
                    match IosatpMode::try_from(mode as usize) {
                        Ok(m) => {
                            info!("vIOMMU ddt entry {} fsc set to mode: {:?}, addr: {:#x} value {:#x}", ddt_index, m, ppn << 12, value);
                            IOMMU.lock().rv_iommu_write_ddt_fsc(ddt_index, m, ppn);
                        }
                        Err(_) => {
                            error!(
                                "vIOMMU ddt entry {} fsc mode {:#x} not supported!",
                                ddt_index, mode
                            );
                            return 0;
                        }
                    }
                } else {
                    return IOMMU.lock().rv_iommu_read_ddt_fsc(ddt_index);
                }
            }
            _ => {
                error!(
                    "Unexpected offset value: {:#x}. This should never happen!",
                    offset
                );
            }
        }
        0
    }
}
