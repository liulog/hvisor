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
// Authors: Jingyu Liu <liujingyu24s@ict.ac.cn>
//

mod riscv_iommu;
mod v_riscv_iommu;

use riscv_iommu::*;
use v_riscv_iommu::*;

use alloc::vec::Vec;
use crate::config::HvZoneConfig;
use log::{error, info};
use spin::{Once, RwLock};
use crate::memory::Frame;
use vcell::VolatileCell;
use crate::platform::__board::*;
use crate::consts::{MAX_CPU_NUM, MAX_ZONE_NUM};
use core::cell::Ref;
use core::mem::size_of;
use crate::zone::find_zone;
use crate::zone::Zone;
use crate::memory::MMIOAccess;
use crate::error::HvResult;
use heapless::FnvIndexMap;
use crate::percpu::this_cpu_data;
use crate::percpu::this_zone;

// Physical IOMMU
static IOMMU: Once<RwLock<Iommu>> = Once::new();
// The MAX_ZONE_NUM should be the power of 2.
static mut VIOMMU_MAP: Option<FnvIndexMap<usize, VirtualIommu, MAX_ZONE_NUM>> = None;

fn host_iommu<'a>() -> &'a RwLock<Iommu> {
    IOMMU.get().expect("Uninitialized hypervisor iommu!")
}

pub fn iommu_init() {
    IOMMU.call_once(|| RwLock::new(Iommu::new(IOMMU_SYS_BASE)));
    host_iommu().write().rv_iommu_init();
    unsafe {
        VIOMMU_MAP = Some(FnvIndexMap::new());
    }
}

pub fn iommu_add_device(vmid: usize, sid: usize) {
    let zone = find_zone(vmid).expect("Invalid vm id!");
    let root_pt = zone.read().gpm.root_paddr();
    host_iommu().write().rv_iommu_write_ddt(sid, vmid, root_pt);
}

impl Zone {
    pub fn viommu_init(&mut self) {
        // Create a new VirtualIOMMU for this Zone.
        unsafe {
            if let Some(map) = &mut VIOMMU_MAP {
                if map.contains_key(&self.id) {
                    panic!("VirtualIommu for Zone {} already exists!", self.id);
                }
                let viommu = v_riscv_iommu::VirtualIommu::new();
                // Insert into Map <zone_id, viommu>
                let _ = map.insert(self.id, viommu);
            } else {
                panic!("VIOMMU_MAP is not initialized!");
            }
        }
        info!("VirtualIommu for Zone {} initialized successfully", self.id);
    }

    pub fn get_viommu(&mut self) -> &VirtualIommu {
        unsafe {
            VIOMMU_MAP
                .as_ref()
                .expect("VIOMMU_MAP is not initialized!")
                .get(&self.id)
                .expect("VirtualIOMMU for this Zone does not exist!")
        }
    }

    pub fn viommu_mmio_init(&mut self) {
        info!("Init Zone's IOMMU MMIO handler");
        self.mmio_region_register(IOMMU_SYS_BASE, IOMMU_SYS_SIZE, viommu_emul_handler, self.id);
    }
}

fn get_viommu_by_zone_id(zone_id: usize) -> &'static VirtualIommu {
    unsafe {
        VIOMMU_MAP
            .as_ref()
            .expect("VIOMMU_MAP is not initialized!")
            .get(&zone_id)
            .expect("VirtualIOMMU for this Zone does not exist!")
    }
}

/// Handle Zone's iommu mmio access.
fn viommu_emul_handler(mmio: &mut MMIOAccess, zone_id: usize) -> HvResult {
    let viommu = get_viommu_by_zone_id(zone_id);
    let binding = this_zone();
    let mut zone_w = binding.write();
    let value = viommu.viommu_emul_access(&mut *zone_w, mmio.address, mmio.size, mmio.value, mmio.is_write);
    if !mmio.is_write {
        // read from viommu
        mmio.value = value as usize;
    }
    Ok(())
}

/// Handle Zone's iommu mmio access.
fn viommu_ddt_emul_handler(mmio: &mut MMIOAccess, zone_id: usize) -> HvResult {
    let value = get_viommu_by_zone_id(zone_id).viommu_ddt_emul_access(mmio.address, mmio.size, mmio.value, mmio.is_write);
    if !mmio.is_write {
        // read from viommu
        mmio.value = value as usize;
    }
    Ok(())
}
