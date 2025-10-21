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

// #![deny(warnings)]
// #![deny(unused)]
// #![deny(dead_code)]
// #![warn(clippy::all)]
// #![warn(clippy::pedantic)]

mod cmd_queue;
mod consts;
mod iommu_hw;
mod regs;
mod viommu;

use iommu_hw::*;
use viommu::*;

use crate::consts::MAX_ZONE_NUM;
use crate::error::HvResult;
use crate::memory::MMIOAccess;
use crate::percpu::this_zone;
use crate::platform::__board::*;
use crate::zone::Zone;
use alloc::sync::Arc;
pub use iommu_hw::{iommu_add_device, iommu_init};

use super::Iommu;
use log::{error, info};

pub struct RiscvIommu;

unsafe impl Send for RiscvIommu {}
unsafe impl Sync for RiscvIommu {}

impl Iommu for RiscvIommu {
    fn initialize(&self, iommu_base: usize, iommu_size: usize) {
        iommu_init();
    }
    fn add_device_share_s2pt(&self, vmid: usize, device_id: usize) {
        iommu_add_device(vmid, device_id);
    }
    fn add_device_exclusive_s2pt(&self, vmid: usize, device_id: usize, regions: alloc::vec::Vec<crate::memory::MemoryRegion<crate::memory::GuestPhysAddr>>) {
        warn!("RiscvIommu does not support exclusive S2PT for device id {} and VMID {}", device_id, vmid);
    }
    fn iommu_flush_all(&self) {
        warn!("RiscvIommu flush all not implemented yet.");
    }
    fn iommu_flush_s2pt_cache(&self, vmid: usize) {
        warn!("RiscvIommu flush s2pt cache for VMID {} not implemented yet.", vmid);
    }
    fn iommu_flush_dev_dir_cache(&self, device_id: usize) {
        warn!("RiscvIommu flush dev dir cache for device id {} not implemented yet.", device_id);
    }
    fn interrupt_handler(&self, irq_id: usize) {
        warn!("RiscvIommu interrupt handler for irq id {} not implemented yet.", irq_id);
    }
    fn viommu_init(&self, zone_id: usize) {
        let mut viommu_arr = VIOMMU_ARR.lock();
        if viommu_arr[zone_id].is_some() {
            warn!("Zone {}'s Virtual IOMMU already initialized.", zone_id);
            return;
        }
        viommu_arr[zone_id] = Some(Arc::new(VirtualIommu::new()));
        info!("Zone {}'s Virtual IOMMU initialized.", zone_id);
    }
    fn viommu_mmio_handler(&self, zone: &mut Zone, viommu_base: usize, viommu_size: usize) {
        zone.mmio_region_register(
            viommu_base,
            viommu_size,
            viommu_emul_handler,
            zone.id,
        );
    }
}

/// Handle Zone's iommu mmio access.
fn viommu_emul_handler(mmio: &mut MMIOAccess, zone_id: usize) -> HvResult {
    let viommu = get_viommu_by_zone_id(zone_id).expect("Zone's viommu not found!");
    let binding = this_zone();
    let mut zone_w = binding.write();
    let value = viommu.viommu_emul_access(
        &mut *zone_w,
        mmio.address,
        mmio.size,
        mmio.value,
        mmio.is_write,
    );
    if !mmio.is_write {
        // read from viommu
        mmio.value = value as usize;
    }
    Ok(())
}

fn get_viommu_by_zone_id(zone_id: usize) -> Option<Arc<VirtualIommu>> {
    // 0..MAX_ZONE_NUM are valid zone ids.
    if zone_id >= MAX_ZONE_NUM {
        error!("Invalid zone id: {}", zone_id);
        return None;
    }

    let viommu_array = VIOMMU_ARR.lock();
    // ref, don't move viommu out of the array
    match &viommu_array[zone_id] {
        Some(viommu) => Some(Arc::clone(viommu)),
        None => {
            error!("VirtualIommu for Zone {} does not exist!", zone_id);
            None
        }
    }
}


/// Handle Zone's iommu mmio access.
fn viommu_ddt_emul_handler(mmio: &mut MMIOAccess, zone_id: usize) -> HvResult {
    let viommu = get_viommu_by_zone_id(zone_id).expect("Zone's viommu not found!");
    let value = viommu.viommu_ddt_emul_access(
        &*this_zone().read(),
        mmio.address,
        mmio.size,
        mmio.value,
        mmio.is_write,
    );
    if !mmio.is_write {
        // read from viommu
        mmio.value = value as usize;
    }
    Ok(())
}