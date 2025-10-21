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

use crate::arch::zone;
use crate::consts::MAX_ZONE_NUM;
use crate::memory::{GuestPhysAddr, MemoryRegion};
use crate::zone::Zone;
use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Once;

#[cfg(feature = "amd_iommu")]
mod amd_iommu;
#[cfg(feature = "arm_smmu")]
mod arm_smmu;
#[cfg(feature = "intel_vtd")]
mod intel_vtd;
#[cfg(feature = "riscv_iommu")]
mod riscv_iommu;

/// IOMMU trait defining the required methods for diffent IOMMU implementations
#[rustfmt::skip]
trait Iommu {
    /// Initialize the IOMMU hardware and data structures
    fn initialize(&self, iommu_base: usize, iommu_size: usize);
    /// Add a device with the given VMID and device ID to the IOMMU
    fn add_device_share_s2pt(&self, zone_id: usize, device_id: usize);
    /// Add a device with exclusive stage 2 page table mappings
    fn add_device_exclusive_s2pt(&self, zone_id: usize, device_id: usize, regions: Vec<MemoryRegion<GuestPhysAddr>>);
    /// Flush all IOMMU translation caches, including device directory and page table translation caches
    fn iommu_flush_all(&self);
    /// Flush stage 2 page table caches for a specific VMID
    fn iommu_flush_s2pt_cache(&self, zone_id: usize);
    /// Flush device directory caches for a specific device ID
    fn iommu_flush_dev_dir_cache(&self, device_id: usize);
    /// Handle IOMMU-related interrupts
    fn interrupt_handler(&self, irq_id: usize);
    /// Initialize the Virtual IOMMU for the Zone
    fn viommu_init(&self, zone_id: usize);
    /// Virtual IOMMU MMIO handler for the Zone
    fn viommu_mmio_handler(&self, zone: &mut Zone, viommu_base: usize, viommu_size: usize);
}

/// Dummy IOMMU implementation for systems without IOMMU support
struct DummyIommu;

#[rustfmt::skip]
impl Iommu for DummyIommu {
    fn initialize(&self, iommu_base: usize, iommu_size: usize) {
        info!("No IOMMU implementation available, skipping initialization, base: {:#x}, size: {:#x}", iommu_base, iommu_size);
    }
    fn add_device_share_s2pt(&self, zone_id: usize, device_id: usize) {
        info!("No IOMMU implementation available, cannot add device id {} for VMID {}", device_id, zone_id);
    }
    fn add_device_exclusive_s2pt(&self, zone_id: usize, device_id: usize, _regions: Vec<MemoryRegion<GuestPhysAddr>>) {
        info!("No IOMMU implementation available, cannot add device id {} for VMID {} with exclusive S2PT", device_id, zone_id);
    }
    fn iommu_flush_all(&self) {
        info!("No IOMMU implementation available, cannot flush all");
    }
    fn iommu_flush_s2pt_cache(&self, zone_id: usize) {
        info!("No IOMMU implementation available, cannot flush S2PT IOTLB for VMID {}", zone_id);
    }
    fn iommu_flush_dev_dir_cache(&self, device_id: usize) {
        info!("No IOMMU implementation available, cannot flush DDT for device id {}", device_id);
    }
    fn interrupt_handler(&self, irq_id: usize) {
        info!("No IOMMU implementation available, cannot handle interrupt id {}", irq_id);
    }
    fn viommu_init(&self, zone_id: usize) {
        info!("No IOMMU implementation available, cannot initialize VIOMMU for Zone id {}", zone_id);
    }
    fn viommu_mmio_handler(&self, zone: &mut Zone, _viommu_base: usize, _viommu_size: usize) {
        info!("No IOMMU implementation available, cannot handle VIOMMU MMIO for Zone id {}", zone.id);
    }
}

/// Global IOMMU implementation instance
/// Note: concret implementations don't contain any mutable global state, only common interfaces define in the trait.
static IOMMU_IMPL: Once<Box<dyn Iommu + Sync + Send>> = Once::new();

// Dispatch to the appropriate IOMMU implementation based on hardware support
fn iommu_impl_init() -> Box<dyn Iommu + Sync + Send> {
    #[cfg(feature = "intel_vtd")]
    return Box::new(intel_vtd::IntelVtdIommu);

    #[cfg(feature = "amd_iommu")]
    return Box::new(amd_iommu::AmdIommu);

    #[cfg(feature = "riscv_iommu")]
    return Box::new(riscv_iommu::RiscvIommu);

    #[cfg(feature = "arm_smmu")]
    return Box::new(arm_smmu::ArmSmmuIommu);

    // Default return DummyIommu if no IOMMU support
    #[cfg(not(any(
        feature = "intel_vtd",
        feature = "amd_iommu",
        feature = "riscv_iommu",
        feature = "arm_smmu",
    )))]
    return Box::new(DummyIommu);
}

/// Get the global IOMMU implementation instance
/// Note: IOMMU_IMPL is immutable after initialization
fn iommu_impl() -> &'static dyn Iommu {
    IOMMU_IMPL.call_once(|| iommu_impl_init()).as_ref()
}

/// Public interface for IOMMU initialization and device addition
/// It is only called during hypervisor initialization by master cpu
pub fn iommu_init(iommu_base: usize, iommu_size: usize) {
    iommu_impl().initialize(iommu_base, iommu_size);
}

/// Public interface for adding a device to the IOMMU
/// It can be call during VM creation, the concret implementation need
pub fn iommu_add_device(zone_id: usize, did: usize) {
    if zone_id >= MAX_ZONE_NUM {
        warn!(
            "Invalid zone id {} for adding device id {} to IOMMU",
            zone_id, did
        );
        return;
    }
    iommu_impl().add_device_share_s2pt(zone_id, did);
}

/// Public interface for adding a device with exclusive S2PT mappings to the IOMMU
pub fn iommu_add_device_exclusive_s2pt(
    zone_id: usize,
    sid: usize,
    regions: Vec<MemoryRegion<GuestPhysAddr>>,
) {
    if zone_id >= MAX_ZONE_NUM {
        warn!(
            "Invalid zone id {} for adding device id {} to IOMMU with exclusive S2PT",
            zone_id, sid
        );
        return;
    }
    iommu_impl().add_device_exclusive_s2pt(zone_id, sid, regions);
}

/// Public interface for flushing all IOMMU caches
pub fn iommu_flush_all() {
    iommu_impl().iommu_flush_all();
}

/// Public interface for flushing S2PT cache for a specific VMID
pub fn iommu_flush_s2pt_cache(zone_id: usize) {
    if zone_id >= MAX_ZONE_NUM {
        warn!("Invalid zone id {} for flushing S2PT IOTLB", zone_id);
        return;
    }
    iommu_impl().iommu_flush_s2pt_cache(zone_id);
}

/// Public interface for flushing device directory cache for a specific device ID
pub fn iommu_flush_dev_dir_cache(device_id: usize) {
    iommu_impl().iommu_flush_dev_dir_cache(device_id);
}

/// Public interface for IOMMU interrupt handling
pub fn iommu_interrupt_handler(irq_id: usize) {
    iommu_impl().interrupt_handler(irq_id);
}

/// Public interface for Zone's viommu initialization
impl Zone {
    // Called only once during Zone creation
    pub fn viommu_init(&self) {
        iommu_impl().viommu_init(self.id);
    }
    // Called only once during Zone creation
    pub fn viommu_mmio_init(&mut self, viommu_base: usize, viommu_size: usize) {
        iommu_impl().viommu_mmio_handler(self, viommu_base, viommu_size);
    }
}
