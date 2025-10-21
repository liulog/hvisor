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

use super::cmd_queue::CqEntry;
use super::consts::*;
use super::regs::*;
use crate::memory::Frame;
use crate::platform::__board::*;
use crate::zone::find_zone;
use alloc::vec::Vec;
use core::ptr::write_volatile;
use log::{error, info};
use spin::Mutex;
use vcell::VolatileCell;
use crate::device::irqchip::plic::plic_enable_irq;
use crate::percpu::this_cpu_data;
use core::sync::atomic::{fence, Ordering};

// Physical IOMMU
lazy_static! {
    pub static ref IOMMU: Mutex<Iommu> = Mutex::new(Iommu::new(IOMMU_SYS_BASE));
}

pub fn iommu_init() {
    info!("Initializing RISC-V IOMMU...");
    IOMMU.lock().rv_iommu_init();
    let cpu_id = this_cpu_data().id;
    // Enable IOMMU IRQs
    #[cfg(feature = "plic")]
    for &irq_id in IOMMU_IRQS.iter() {
        plic_enable_irq(cpu_id, irq_id as _, true);
    }
    info!("RISC-V IOMMU initialized successfully");
}

pub fn iommu_add_device(vmid: usize, sid: usize) {
    let zone = find_zone(vmid).expect("Invalid vm id!");
    let root_pt = zone.read().gpm.root_paddr();
    IOMMU.lock().rv_iommu_write_ddt(sid, vmid, root_pt);
}

pub fn iommu_add_command(cmd: CqEntry) {
    IOMMU.lock().rv_iommu_add_command(cmd);
}

/// MSI configuration table structure
#[repr(C)]
pub struct MsiCfgTbl {
    msg_addr: VolatileCell<u64>,
    msg_data: VolatileCell<u32>,
    vector_ctl: VolatileCell<u32>,
}

/// IOMMU Memory-mapped Register Layout
#[repr(C)]
pub struct IommuRegMap {
    pub caps: VolatileCell<u64>, // Capabilities Register
    pub fctl: VolatileCell<u32>, // Features-control Register
    __custom1: [u8; 4],
    pub ddtp: VolatileCell<u64>, // Device Directory Table Pointer
    /// Command Queue
    pub cqb: VolatileCell<u64>, // Command Queue Base
    pub cqh: VolatileCell<u32>,  // Command Queue Head
    pub cqt: VolatileCell<u32>,  // Command Queue Tail
    /// Fault Queue
    pub fqb: VolatileCell<u64>, // Fault Queue Base
    pub fqh: VolatileCell<u32>,  // Fault Queue Head
    pub fqt: VolatileCell<u32>,  // Fault Queue Tail
    /// Page-request Queue
    pub pqb: VolatileCell<u64>, // Page-request Queue Base
    pub pqh: VolatileCell<u32>,  // Page-request Queue Head
    pub pqt: VolatileCell<u32>,  // Page-request Queue Tail
    pub cqcsr: VolatileCell<u32>, // Command Queue CSR
    pub fqcsr: VolatileCell<u32>, // Fault Queue CSR
    pub pqcsr: VolatileCell<u32>, // Page-request Queue CSR
    pub ipsr: VolatileCell<u32>, // Interrupt Pending and Status Register
    /// HPM
    pub iocntovf: VolatileCell<u32>, // HPM Counter Overflows
    pub iocntinh: VolatileCell<u32>, // HPM Counter Inhibits
    pub iohpmcycles: VolatileCell<u64>, // HPM Cycle Counter
    pub iohpmctr: [VolatileCell<u64>; 31], // HPM Event Counters
    pub iohpmevt: [VolatileCell<u64>; 31], // HPM Event Selector
    /// DBG
    pub tr_req_iova: VolatileCell<u64>, // Translation-request IOVA
    pub tr_req_ctl: VolatileCell<u64>, // Translation-request Control
    pub tr_response: VolatileCell<u64>, // Translation-request Response
    __rsv1: [u8; 64],
    __custom2: [u8; 72],
    pub icvec: VolatileCell<u64>, // Interrupt Control and Vector Register
    pub msi_cfg_tbl: [MsiCfgTbl; 16],
    __rsv2: [u8; 3072],
}

// Note: If capabilities.MSI_FLAT is 1 then the Extended Format is used else the Base Format is used.
// Now: only supports Extended Format Device Context

/// Device Directory Table Entry(Extended Format)
#[repr(C)]
pub struct DdtEntry {
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
// #[repr(C)]
// struct FqEntry {
//     tags: VolatileCell<u64>,
//     __rsv: u32,
//     __custom: u32,
//     iotval: VolatileCell<u64>,
//     iotval2: VolatileCell<u64>,
// }

#[repr(C)]
pub struct Iommu {
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
    pub fn new(base: usize) -> Self {
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
        if caps & (RV_IOMMU_CAPS_SV39X4_BIT | RV_IOMMU_CAPS_SV48X4_BIT | RV_IOMMU_CAPS_SV57X4_BIT)
            == 0
        {
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
        info!(
            "RISC-V IOMMU HW supports physical address size: {} bits",
            pas
        );
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

    pub fn rv_iommu_init(&mut self) {
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
        self.rv_iommu_regmap
            .ddtp
            .set(IOMMU_MODE as u64 | ((ddt_addr >> 2) & RV_IOMMU_DDTP_PPN_MASK) as u64);
        info!(
            "RV_IOMMU: Write DDTP, mode {:#x}, ddt_addr {:#x}",
            IOMMU_MODE, ddt_addr
        );

        // Initialize Command Queue
        self.rv_iommu_regmap.cqt.set(0);
        let cmd_queue_frame = Frame::new_zero().unwrap();
        let cmd_queue_addr = cmd_queue_frame.start_paddr();
        self.cmd_queue.push(cmd_queue_frame);
        let cq_num_entries: usize = 4096 / 16; // 2^n
        let log2sz_1 = cq_num_entries.trailing_zeros() - 1;
        let cqb_value =
            ((cmd_queue_addr as u64 >> 2) & 0x3F_FFFF_FFFF_FC00) | (log2sz_1 as u64 & 0x1F);
        self.rv_iommu_regmap.cqb.set(cqb_value);

        // Initialize Fault Queue
        self.rv_iommu_regmap.fqh.set(0);
        self.fault_queue.push(Frame::new_zero().unwrap());

        // Current not support Page-request Queue
    }

    /// Write DDT entry for a device
    pub fn rv_iommu_write_ddt(&mut self, device_id: usize, vm_id: usize, root_pt: usize) {
        if device_id < self.dev_num_max {
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
            info!(
                "RV IOMMU: Write DDT, device id {:#x}, vm id {:#x}, iohgatp {:#x}",
                device_id, vm_id, iohgatp
            );
        } else {
            warn!("RV IOMMU: Invalid device ID: {}", device_id);
        }
    }

    /// Read DDT entry's fsc field
    pub fn rv_iommu_read_ddt_iohgatp(&self, device_id: usize) -> u64 {
        if device_id < self.dev_num_max {
            // Note: this only valid for 1lvl DDT
            let ddt_ptr = self.ddt[0].start_paddr() as *mut DdtEntry;
            let dc_ptr = unsafe { ddt_ptr.add(device_id) };
            let value = unsafe { (*dc_ptr).iohgatp.get() as u64 };
            trace!(
                "RV IOMMU: Read DDT, device id {:#x}, iohgatp value: {:#x}",
                device_id, value
            );
            value
        } else {
            warn!("RV IOMMU: Invalid device ID: {}", device_id);
            0
        }
    }

    /// Update DDT entry's fsc field
    pub fn rv_iommu_write_ddt_fsc(&mut self, device_id: usize, mode: IosatpMode, ppn: u64) {
        if device_id < self.dev_num_max {
            let fsc_value: u64 = ((mode as u64) << 60) & RV_IOMMU_FSC_MODE_MASK
                | (ppn as u64 & RV_IOMMU_FSC_PPN_MASK);
            // Note: this only valid for 1lvl DDT
            let ddt_ptr = self.ddt[0].start_paddr() as *mut DdtEntry;
            let dc_ptr = unsafe { ddt_ptr.add(device_id) };
            unsafe {
                (*dc_ptr).fsc.set(fsc_value);
            }
        } else {
            warn!("RV IOMMU: Invalid device ID: {}", device_id);
        }
    }

    /// Update DDT entry's fsc field
    pub fn rv_iommu_write_ddt_ta(&mut self, device_id: usize, value: u64) {
        if device_id < self.dev_num_max {
            // Note: this only valid for 1lvl DDT
            let ddt_ptr = self.ddt[0].start_paddr() as *mut DdtEntry;
            let dc_ptr = unsafe { ddt_ptr.add(device_id) };
            unsafe {
                (*dc_ptr).ta.set(value);
            }
        } else {
            warn!("RV IOMMU: Invalid device ID: {}", device_id);
        }
    }

    /// Read DDT entry's fsc field
    pub fn rv_iommu_read_ddt_fsc(&self, device_id: usize) -> u64 {
        if device_id < self.dev_num_max {
            // Note: this only valid for 1lvl DDT
            let ddt_ptr = self.ddt[0].start_paddr() as *mut DdtEntry;
            let dc_ptr = unsafe { ddt_ptr.add(device_id) };
            let value = unsafe { (*dc_ptr).fsc.get() as u64 };
            trace!(
                "RV IOMMU: Read DDT, device id {:#x}, fsc value: {:#x}",
                device_id, value
            );
            value
        } else {
            warn!("RV IOMMU: Invalid device ID: {}", device_id);
            0
        }
    }

    /// Handle Command Queue IRQ
    #[allow(dead_code)]
    pub fn rv_iommu_cq_irq_handler(&mut self) {}

    /// Handle Fault Queue IRQ
    #[allow(dead_code)]
    pub fn rv_iommu_fq_irq_handler(&mut self) {}

    /// Handle Page-request Queue IRQ
    #[allow(dead_code)]
    pub fn rv_iommu_pq_irq_handler(&mut self) {}

    /// Write one command to command queue
    pub fn rv_iommu_add_command(&mut self, cmd: CqEntry) {
        let cqt = self.rv_iommu_regmap.cqt.get();
        // if cqt == cqh - 1, then the command queue is full
        // if command queue is full, wait.
        while (self.rv_iommu_regmap.cqh.get().wrapping_sub(1)) % IOMMU_CQ_NUM_ENTRIES == cqt {
            // Wait for previous command to be processed
            fence(Ordering::Acquire);
            warn!("RV IOMMU: Command Queue is full, wait...");
        }

        // Insert command to command queue
        let cq_hpa = self.cmd_queue[0].start_paddr();
        let cqe_hpa = (cq_hpa + cqt as usize * IOMMU_CQ_ENTRY_SIZE as usize) as *mut u64;
        let cmd_bytes = cmd.into_bytes();
        let lower_bytes = u64::from_le_bytes(cmd_bytes[0..8].try_into().unwrap());
        let upper_bytes = u64::from_le_bytes(cmd_bytes[8..16].try_into().unwrap());
        unsafe {
            write_volatile(cqe_hpa, lower_bytes);
            write_volatile(cqe_hpa.add(1), upper_bytes);
        }

        fence(Ordering::SeqCst);

        // Update cqt
        self.rv_iommu_regmap
            .cqt
            .set((cqt + 1) % IOMMU_CQ_NUM_ENTRIES);
    }
}
