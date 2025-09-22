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
//      ForeverYolo <2572131118@qq.com>
//      Jingyu Liu <liujingyu24s@ict.ac.cn>

// RISC-V IOMMU Spec:
//
//     Hypervisor may provide an SW emulated IOMMU to allow the guest to manage the
//     first-stage page tables for fine grained control on memory accessed by guest
//     controlled devices.
//
//     A hypervisor that provides such an emulated IOMMU to the guest may retain
//     control of the second-stage address translation and clear the SvNx4 fields of the
//     emulated capabilities register.
//
//     A hypervisor that provides such an emulated IOMMU to the guest may retain
//     control of the MSI page tables used to direct MSIs to guest interrupt files in an
//     IMSIC or to a memory-resident-interrupt-file and clear the MSI_FLAT and MSI_MRIF
//     fields of the emulated capabilities register.

use alloc::vec::Vec;
use log::{error, info};
use spin::{Once, RwLock};
use crate::memory::Frame;
use vcell::VolatileCell;
use crate::platform::__board::*;
use core::mem::size_of;
use crate::zone::find_zone;
use crate::zone::Zone;
use crate::memory::MMIOAccess;
use crate::error::HvResult;

// pub const BLK_PCI_ID: usize = 0x4;
// pub const PCIE_MMIO_BEG: usize = 0x4000_0000;
// pub const PCIE_MMIO_SIZE: usize = 0x0000_0000;
// pub const PCI_MAP_BEG: usize = 0x4_0000_0000;
// pub const PCI_MAP_SIZE: usize = 0x4_0000_0000;


/// This driver's global configuration
const IOMMU_MODE: usize = IommuMode::Ddt1Lvl as _;
const IOMMU_CQ_PAGE_NUM: usize = 1;
const IOMMU_FQ_PAGE_NUM: usize = 1;
const IOMMU_PQ_PAGE_NUM: usize = 1;


/// Capabilities register fields
pub const RV_IOMMU_CAPS_VERSION_MASK: u64 = 0xFF << 0;
pub const RV_IOMMU_CAPS_SV32_BIT: u64 = 0x1 << 8;
pub const RV_IOMMU_CAPS_SV39_BIT: u64 = 0x1 << 9;
pub const RV_IOMMU_CAPS_SV48_BIT: u64 = 0x1 << 10;
pub const RV_IOMMU_CAPS_SV57_BIT: u64 = 0x1 << 11;
pub const RV_IOMMU_CAPS_SVPBMT_BIT: u64 = 0x1 << 15;
pub const RV_IOMMU_CAPS_SV32X4_BIT: u64 = 0x1 << 16;
pub const RV_IOMMU_CAPS_SV39X4_BIT: u64 = 0x1 << 17;
pub const RV_IOMMU_CAPS_SV48X4_BIT: u64 = 0x1 << 18;
pub const RV_IOMMU_CAPS_SV57X4_BIT: u64 = 0x1 << 19;
pub const RV_IOMMU_CAPS_AMO_MRIF_BIT: u64 = 0x1 << 21;
pub const RV_IOMMU_CAPS_MSI_FLAT_BIT: u64 = 0x1 << 22;
pub const RV_IOMMU_CAPS_MSI_MRIF_BIT: u64 = 0x1 << 23;
pub const RV_IOMMU_CAPS_AMO_HWAD_BIT: u64 = 0x1 << 24;
pub const RV_IOMMU_CAPS_ATS_BIT: u64 = 0x1 << 25;
pub const RV_IOMMU_CAPS_T2GPA_BIT: u64 = 0x1 << 26;
pub const RV_IOMMU_CAPS_END_BIT: u64 = 0x1 << 27;
pub const RV_IOMMU_CAPS_IGS_MASK: u64 = 0x3 << 28;
pub const RV_IOMMU_CAPS_HPM_BIT: u64 = 0x1 << 30;
pub const RV_IOMMU_CAPS_DBG_BIT: u64 = 0x1 << 31;
pub const RV_IOMMU_CAPS_PAS_MASK: u64 = 0x3F << 32;
pub const RV_IOMMU_CAPS_PD8_BIT: u64 = 0x1 << 38;
pub const RV_IOMMU_CAPS_PD17_BIT: u64 = 0x1 << 39;
pub const RV_IOMMU_CAPS_PD20_BIT: u64 = 0x1 << 40;

pub const RV_IOMMU_SUPPORTED_VERSION: u64 = 0x10;
pub const RV_IOMMU_IGS_MSI: u64 = 0;
pub const RV_IOMMU_IGS_WSI: u64 = 1;
pub const RV_IOMMU_IGS_BOTH: u64 = 2;

/// Features control register fields
pub const RV_IOMMU_FCTL_DEFAULT: u32 = 0x1 << 1;
pub const RV_IOMMU_FCTL_BE_BIT: u32 = 0x1 << 0;
pub const RV_IOMMU_FCTL_WSI_BIT: u32 = 0x1 << 1;
pub const RV_IOMMU_FCTL_GXL_BIT: u32 = 0x1 << 2;

/// Device-directory-table pointer fields
pub const RV_IOMMU_DDTP_MODE_MASK: usize = 0xF;
pub const RV_IOMMU_DDTP_BUSY_BIT: usize = 0x1 << 4;
pub const RV_IOMMU_DDTP_PPN_MASK: usize = 0xFFF_FFFF_FFFF << 10;    // [53:10] PPN

/// Iommu Mode
#[derive(Debug, PartialEq, Eq)]
pub enum IommuMode {
    Off = 0x0,      // No inbound memory transactions are allowed
    Bare = 0x1,     // No translation or protection
    Ddt1Lvl = 0x2,  // One-level device-directory-table
    Ddt2Lvl = 0x3,  // Two-level device-directory-table
    Ddt3Lvl = 0x4,  // Three-level device-directory-table
}

impl TryFrom<usize> for IommuMode {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::Bare),
            2 => Ok(Self::Ddt1Lvl),
            3 => Ok(Self::Ddt2Lvl),
            4 => Ok(Self::Ddt3Lvl),
            _ => Err(()),
        }
    }
}

/// DDT entry. fsc register fields
pub const RV_IOMMU_FSC_MODE_MASK:u64 = 0xF << 60;
pub const RV_IOMMU_FSC_PPN_MASK:u64 = 0xFFF_FFFF_FFFF;

#[derive(Debug, PartialEq, Eq)]
pub enum IosatpMode {
    Bare = 0x0,
    Sv39 = 0x8,
    Sv48 = 0x9,
    Sv57 = 0xA,
}

impl TryFrom<usize> for IosatpMode {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0x0 => Ok(Self::Bare),
            0x8 => Ok(Self::Sv39),
            0x9 => Ok(Self::Sv48),
            0xA => Ok(Self::Sv57),
            _ => Err(()),
        }
    }
}

/// DDT entry. iohgatp register fields
// pub const RV_IOMMU_IOHGATP_MODE_MASK: u64 = 0xF << 60;
// pub const RV_IOMMU_IOHGATP_GSCID_MASK: u64 = 0xFFFF << 44;
// pub const RV_IOMMU_IOHGATP_PPN_MASK: u64 = 0xFFF_FFFF_FFFF;

// #[derive(Debug, PartialEq, Eq)]
// pub enum IohgatpMode {
//     Bare = 0x0,
//     Sv39x4 = 0x8,
//     Sv48x4 = 0x9,
//     Sv57x4 = 0xA,
// }

// impl TryFrom<usize> for IohgatpMode {
//     type Error = ();
//     fn try_from(value: usize) -> Result<Self, Self::Error> {
//         match value {
//             0x0 => Ok(Self::Bare),
//             0x8 => Ok(Self::Sv39x4),
//             0x9 => Ok(Self::Sv48x4),
//             0xA => Ok(Self::Sv57x4),
//             _ => Err(()),
//         }
//     }
// }

/// Device-context fields
pub const RV_IOMMU_DC_TC_VALID_BIT: u64 = 1;
pub const RV_IOMMU_DC_TC_EN_ATS_BIT: u64 = 1 << 1;
pub const RV_IOMMU_DC_TC_EN_PRI_BIT: u64 = 1 << 2;
pub const RV_IOMMU_DC_TC_T2GPA_BIT: u64 = 1 << 3;
pub const RV_IOMMU_DC_TC_DTF_BIT: u64 = 1 << 4;
pub const RV_IOMMU_DC_TC_PDTV_BIT: u64 = 1 << 5;
pub const RV_IOMMU_DC_TC_PRPR_BIT: u64 = 1 << 6;
pub const RV_IOMMU_DC_TC_GADE_BIT: u64 = 1 << 7;
pub const RV_IOMMU_DC_TC_SADE_BIT: u64 = 1 << 8;
pub const RV_IOMMU_DC_TC_DPE_BIT: u64 = 1 << 9;
pub const RV_IOMMU_DC_TC_SBE_BIT: u64 = 1 << 10;
pub const RV_IOMMU_DC_TC_SXL_BIT: u64 = 1 << 11;
pub const RV_IOMMU_DC_IOHGATP_MODE_MASK: u64 = 0xF << 60;
pub const RV_IOMMU_DC_IOHGATP_GSCID_MASK: u64 = 0xFFFF << 44;
pub const RV_IOMMU_DC_IOHGATP_PPN_MASK: u64 = 0xFFF_FFFF_FFFF;

/// Icvec register fields
pub const RV_IOMMU_ICVEC_CIV_MASK: u64 = 0xF;
pub const RV_IOMMU_ICVEC_FIV_MASK: u64 = 0xF << 4;
pub const RV_IOMMU_ICVEC_PMIV_MASK: u64 = 0xF << 8;
pub const RV_IOMMU_ICVEC_PIV_MASK: u64 = 0xF << 12;
pub const RV_IOMMU_ICVEC_MASK: u64 = RV_IOMMU_ICVEC_CIV_MASK | RV_IOMMU_ICVEC_FIV_MASK;

/// Interrupt pending status register fields
pub const RV_IOMMU_IPSR_CIP_BIT: u32 = 1;
pub const RV_IOMMU_IPSR_FIP_BIT: u32 = 1 << 1;
pub const RV_IOMMU_IPSR_PMIP_BIT: u32 = 1 << 2;
pub const RV_IOMMU_IPSR_PIP_BIT: u32 = 1 << 3;
pub const RV_IOMMU_IPSR_CLEAR: u32 = RV_IOMMU_IPSR_CIP_BIT | RV_IOMMU_IPSR_FIP_BIT | RV_IOMMU_IPSR_PMIP_BIT | RV_IOMMU_IPSR_PIP_BIT;

/// In-memory Queue common fields
pub const RV_IOMMU_XQCSR_XQEN_BIT: u32 = 1 << 0;
pub const RV_IOMMU_XQCSR_XIE_BIT: u32 = 1 << 1;
pub const RV_IOMMU_FQCSR_XQMF_BIT: u32 = 1 << 8;
pub const RV_IOMMU_XQCSR_XQON_BIT: u32 = 1 << 16;
pub const RV_IOMMU_XQCSR_BUSY_BIT: u32 = 1 << 17;
/// Command queue CSR fields
pub const RV_IOMMU_CQCSR_CMD_TO_BIT: u32 = 1 << 9;
pub const RV_IOMMU_CQCSR_CMD_ILL_BIT: u32 = 1 << 10;
pub const RV_IOMMU_CQCSR_CMD_FENCE_WIP_BIT: u32 = 1 << 11;
/// Fault queue CSR fields
pub const RV_IOMMU_FQCSR_FQOF_BIT: u32 = 1 << 9;
/// Page-request queue CSR fields
pub const RV_IOMMU_PQCSR_PQOF_BIT: u32 = 1 << 9;

/// MSI configuration table structure
#[repr(C)]
pub struct MsiCfgTbl{
    msg_addr: VolatileCell<u64>,
    msg_data: VolatileCell<u32>,
    vector_ctl: VolatileCell<u32>,
}

/// IOMMU Memory-mapped Register Layout
#[repr(C)]
pub struct IommuRegMap {
    pub caps: VolatileCell<u64>,                  // Capabilities Register
    pub fctl: VolatileCell<u32>,                  // Features-control Register
    __custom1: [u8; 4],
    pub ddtp: VolatileCell<u64>,                  // Device Directory Table Pointer
    /// Command Queue
    pub cqb: VolatileCell<u64>,                   // Command Queue Base
    pub cqh: VolatileCell<u32>,                   // Command Queue Head
    pub cqt: VolatileCell<u32>,                   // Command Queue Tail
    /// Fault Queue
    pub fqb: VolatileCell<u64>,                   // Fault Queue Base
    pub fqh: VolatileCell<u32>,                   // Fault Queue Head
    pub fqt: VolatileCell<u32>,                   // Fault Queue Tail
    /// Page-request Queue
    pub pqb: VolatileCell<u64>,                   // Page-request Queue Base
    pub pqh: VolatileCell<u32>,                   // Page-request Queue Head
    pub pqt: VolatileCell<u32>,                   // Page-request Queue Tail
    pub cqcsr: VolatileCell<u32>,                 // Command Queue CSR
    pub fqcsr: VolatileCell<u32>,                 // Fault Queue CSR
    pub pqcsr: VolatileCell<u32>,                 // Page-request Queue CSR
    pub ipsr: VolatileCell<u32>,                  // Interrupt Pending and Status Register
    /// HPM
    pub iocntovf: VolatileCell<u32>,              // HPM Counter Overflows
    pub iocntinh: VolatileCell<u32>,              // HPM Counter Inhibits
    pub iohpmcycles: VolatileCell<u64>,           // HPM Cycle Counter
    pub iohpmctr: [VolatileCell<u64>; 31],        // HPM Event Counters
    pub iohpmevt: [VolatileCell<u64>; 31],        // HPM Event Selector
    /// DBG
    pub tr_req_iova: VolatileCell<u64>,           // Translation-request IOVA
    pub tr_req_ctl: VolatileCell<u64>,            // Translation-request Control
    pub tr_response: VolatileCell<u64>,           // Translation-request Response
    __rsv1: [u8; 64],
    __custom2: [u8; 72],
    pub icvec: VolatileCell<u64>,                 // Interrupt Control and Vector Register
    pub msi_cfg_tbl: [MsiCfgTbl; 16],
    __rsv2: [u8; 3072],
}


// Note: If capabilities.MSI_FLAT is 1 then the Extended Format is used else the Base Format is used.
// Now: only supports Extended Format Device Context

/// Device Directory Table Entry(Extended Format)
#[repr(C)]
pub struct DdtEntry{
    tc: VolatileCell<u64>,
    iohgatp: VolatileCell<u64>,
    ta: VolatileCell<u64>,
    fsc: VolatileCell<u64>,
    msiptp: VolatileCell<u64>,
    msi_addr_mask: VolatileCell<u64>,
    msi_addr_pattern: VolatileCell<u64>,
    __rsv: u64,
}

/// Fault Queue Entry
#[repr(C)]
pub struct FqEntry{
    tags: VolatileCell<u64>,
    __rsv: u32,
    __custom: u32,
    iotval: VolatileCell<u64>,
    iotval2: VolatileCell<u64>,
}

#[repr(C)]
pub struct Iommu{
    pub rv_iommu_regmap: &'static mut IommuRegMap,
    ddt: Vec<Frame>,
    dev_num_max: usize,
    cmd_queue: Vec<Frame>,
    fault_queue: Vec<Frame>,
    page_request_queue: Vec<Frame>,
}

// Due to iommu hw regmap contains unsafe cell, it is needed in multi-thread env.
unsafe impl Sync for Iommu {}

impl Iommu {
    /// Create a new IOMMU instance
    pub fn new(base: usize) -> Self{
        // Note: for extened format device context.
        let dev_num_max: usize = match IOMMU_MODE {
            x if x == IommuMode::Ddt1Lvl as usize => 1 << 6,
            x if x == IommuMode::Ddt2Lvl as usize => 1 << (6 + 9),
            x if x == IommuMode::Ddt3Lvl as usize => 1 << (6 + 9 + 9),
            _ => 0,
        };
        Self { 
            rv_iommu_regmap: unsafe { &mut *(base as *mut _) },
            ddt: Vec::new(),
            dev_num_max: dev_num_max,
            cmd_queue: Vec::new(),
            fault_queue: Vec::new(),
            page_request_queue: Vec::new(),
        }
    }

    /// Check IOMMU features
    pub fn rv_iommu_check_features(&self) {
        let caps = self.rv_iommu_regmap.caps.get();
        let version = caps & RV_IOMMU_CAPS_VERSION_MASK;
        // Note: here hvisor supports version 1.0
        if version != RV_IOMMU_SUPPORTED_VERSION {
            panic!("RISC-V IOMMU unsupported version: {}", version);
        } else {
            info!("RISC-V IOMMU version 1.0.0");
        }
        if caps & RV_IOMMU_CAPS_SV39_BIT != 0 {
            info!("RISC-V IOMMU HW supports Sv32");
        }
        if caps & RV_IOMMU_CAPS_SV48_BIT != 0 {
            info!("RISC-V IOMMU HW supports Sv48");
        }
        if caps & RV_IOMMU_CAPS_SV57_BIT != 0 {
            info!("RISC-V IOMMU HW supports Sv57");
        }
        if caps & (RV_IOMMU_CAPS_SV39_BIT | RV_IOMMU_CAPS_SV48_BIT | RV_IOMMU_CAPS_SV57_BIT) == 0 {
            panic!("RISC-V IOMMU HW unsupported SvN");
        }
        if caps & RV_IOMMU_CAPS_SVPBMT_BIT != 0 {
            info!("RISC-V IOMMU HW supports Svpbmt (Page-based memory types)");
        }
        if caps & RV_IOMMU_CAPS_SV39X4_BIT != 0 {
            info!("RISC-V IOMMU HW supports Sv39x4");
        }
        if caps & RV_IOMMU_CAPS_SV48X4_BIT != 0 {
            info!("RISC-V IOMMU HW supports Sv48x4");
        }
        if caps & RV_IOMMU_CAPS_SV57X4_BIT != 0 {
            info!("RISC-V IOMMU HW supports Sv57x4");
        }
        // Note: here riscv iommu must support SvNx4
        if caps & (RV_IOMMU_CAPS_SV39X4_BIT | RV_IOMMU_CAPS_SV48X4_BIT | RV_IOMMU_CAPS_SV57X4_BIT) == 0 {
            panic!("RISC-V IOMMU HW unsupported SvNx4");
        }
        if caps & RV_IOMMU_CAPS_AMO_MRIF_BIT != 0 {
            info!("RISC-V IOMMU HW supports AMO MRIF (Atomic updates to MRIF)");
        }
        if caps & RV_IOMMU_CAPS_MSI_FLAT_BIT != 0 {
            info!("RISC-V IOMMU HW supports MSI FLAT");
            info!("Using Extended Format Device Context");
        } else {
            panic!("Now only supports Extended Format Device Context");
        }
        if caps & RV_IOMMU_CAPS_MSI_MRIF_BIT != 0 {
            info!("RISC-V IOMMU HW supports MSI MRIF");
        }
        if caps & RV_IOMMU_CAPS_ATS_BIT != 0 {
            info!("RISC-V IOMMU HW supports ATS");
        }
        if caps & RV_IOMMU_CAPS_T2GPA_BIT != 0 {
            info!("RISC-V IOMMU HW supports T2GPA");
        }
        if caps & RV_IOMMU_CAPS_END_BIT != 0 {
            info!("RISC-V IOMMU HW supports both endianness");
        } else {
            info!("RISC-V IOMMU HW supports one endianness (either little or big)");
        }
        let igs = (caps & RV_IOMMU_CAPS_IGS_MASK) >> 28;
        match igs {
            RV_IOMMU_IGS_MSI => info!("RISC-V IOMMU HW supports MSI generation"),
            RV_IOMMU_IGS_WSI => info!("RISC-V IOMMU HW supports WSI generation"),
            RV_IOMMU_IGS_BOTH => info!("RISC-V IOMMU HW supports Both MSI and WSI generation"),
            _ => error!("RISC-V IOMMU HW unsupported interrupt generation scheme"),
        }
        if caps & RV_IOMMU_CAPS_HPM_BIT != 0 {
            info!("RISC-V IOMMU HW supports HPM");
        }
        if caps & RV_IOMMU_CAPS_DBG_BIT != 0 {
            info!("RISC-V IOMMU HW supports DBG");
        }
        let pas = (caps & RV_IOMMU_CAPS_PAS_MASK) >> 32;
        info!("RISC-V IOMMU HW supports physical address size: {} bits", pas);
        if caps & RV_IOMMU_CAPS_PD8_BIT != 0 {
            info!("RISC-V IOMMU HW: One level PDT with 8-bit process_id supported");
        }
        if caps & RV_IOMMU_CAPS_PD17_BIT != 0 {
            info!("RISC-V IOMMU HW: Two level PDT with 17-bit process_id supported");
        }
        if caps & RV_IOMMU_CAPS_PD20_BIT != 0 {
            info!("RISC-V IOMMU HW: Three level PDT with 20-bit process_id supported");
        }
    }
    
    pub fn rv_iommu_init(&mut self){
        // Read and Check IOMMU Capabilities
        self.rv_iommu_check_features();

        // Little Endian and Wire Signal Interrupt
        self.rv_iommu_regmap.fctl.set(RV_IOMMU_FCTL_DEFAULT);
        info!("Now only supports Wire Signal Interrupt");
        
        // Configure ddtp with DDT base address and IOMMU mode
        //  Note: here hvisor only supports one-level DDT, only alloc one page
        let ddt_frame = Frame::new_zero().unwrap();
        let ddt_addr = ddt_frame.start_paddr();
        self.ddt.push(ddt_frame);
        self.rv_iommu_regmap.ddtp.set(IOMMU_MODE as u64 | ((ddt_addr >> 2) & RV_IOMMU_DDTP_PPN_MASK) as u64);
        info!("RV_IOMMU: Write DDTP, mode {:#x}, ddt_addr {:#x}", IOMMU_MODE, ddt_addr);
    }

    /// Write DDT entry for a device
    pub fn rv_iommu_write_ddt(&mut self, _device_id: usize, vm_id: usize, root_pt: usize){
        for device_id in 0..self.dev_num_max {
            // configure DC
            let tc: u64 = RV_IOMMU_DC_TC_VALID_BIT as u64;
            // Note: this only valid for 1lvl DDT
            let ddt_ptr = self.ddt[0].start_paddr() as *mut DdtEntry;
            let dc_ptr = unsafe { ddt_ptr.add(device_id) };
            let mut iohgatp: u64 = 0;
            iohgatp |= ((root_pt as u64) >> 12) & RV_IOMMU_DC_IOHGATP_PPN_MASK as u64;
            iohgatp |= ((vm_id as u64) << 44) & RV_IOMMU_DC_IOHGATP_GSCID_MASK as u64;
            iohgatp |= ((0xa as u64) << 60) & RV_IOMMU_DC_IOHGATP_MODE_MASK;
            unsafe {
                (*dc_ptr).tc.set(tc);
                (*dc_ptr).iohgatp.set(iohgatp);
                (*dc_ptr).fsc.set(0);
            }
            info!("RV IOMMU: Write DDT, device id {:#x}, vm id {:#x}, iohgatp {:#x}", device_id, vm_id, iohgatp);
        }
        // else{
        //     warn!("RV IOMMU: Invalid device ID: {}", device_id);
        // }
    }

    /// Update DDT entry's fsc field
    // pub fn rv_iommu_write_ddt_fsc(&mut self, device_id: usize, mode: IosatpMode, ppn: usize){
    //     if device_id < self.dev_num_max {
    //         let fsc_value: u64 = ((mode.into()) << 60) & RV_IOMMU_FSC_MODE_MASK | (ppn as u64 & RV_IOMMU_FSC_PPN_MASK);
    //         // Note: this only valid for 1lvl DDT
    //         let ddt_ptr = self.ddt[0].start_paddr() as *mut DdtEntry;
    //         let dc_ptr = unsafe { ddt_ptr.add(device_id) };
    //         unsafe {
    //             (*dc_ptr).fsc.set(fsc_value);
    //         }
    //     }
    //     else{
    //         warn!("RV IOMMU: Invalid device ID: {}", device_id);
    //     }
    // }

    /// Handle Command Queue IRQ
    pub fn rv_iommu_cq_irq_handler(&mut self) { 
    }

    /// Handle Fault Queue IRQ
    pub fn rv_iommu_fq_irq_handler(&mut self) {
    }

    /// Handle Page-request Queue IRQ
    pub fn rv_iommu_pq_irq_handler(&mut self) {
    }

}
