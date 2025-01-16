use crate::{
    config::*,
    device::virtio_trampoline::{mmio_virtio_handler, VIRTIO_BRIDGE},
    error::HvResult,
    memory::{
        addr::align_up, GuestPhysAddr, HostPhysAddr, MemFlags, MemoryRegion,
    },
    percpu::get_cpu_data,
    zone::Zone,
};
impl Zone {
    pub fn pt_init( &mut self, mem_regions: &[HvConfigMemoryRegion],
    ) -> HvResult {
        for mem_region in mem_regions.iter() {
            let mut flags = MemFlags::READ | MemFlags::WRITE | MemFlags::EXECUTE;
            if mem_region.mem_type == MEM_TYPE_IO {
                flags |= MemFlags::IO;
            }
            match mem_region.mem_type {
                MEM_TYPE_RAM | MEM_TYPE_IO => {
                    self.gpm.insert(MemoryRegion::new_with_offset_mapper(
                        mem_region.virtual_start as GuestPhysAddr,
                        mem_region.physical_start as HostPhysAddr,
                        mem_region.size as _,
                        flags,
                    ))?
                }
                MEM_TYPE_VIRTIO => {
                    self.mmio_region_register(
                        mem_region.physical_start as _,
                        mem_region.size as _,
                        mmio_virtio_handler,
                        mem_region.physical_start as _,
                    );
                }
                _ => {
                    panic!("Unsupported memory type: {}", mem_region.mem_type)
                }
            }
        }

        info!("VM stage 2 memory set: {:#x?}", self.gpm);

        // unsafe{ info!("{:#x?}",self.gpm.page_table_query(0x81000000)); }
        // unsafe{ info!("{:#x?}",self.gpm.page_table_query(0x82000000)); }

        Ok(())
    }
    pub fn mmio_init(&mut self, hv_config: &HvArchZoneConfig) {
        //TODO
    }
    pub fn irq_bitmap_init(&mut self, irqs: &[u32]) {
        for irq in irqs {
            self.insert_irq_to_bitmap(*irq);
        }
        for (index, &word) in self.irq_bitmap.iter().enumerate() {
            for bit_position in 0..32 {
                if word & (1 << bit_position) != 0 {
                    let interrupt_number = index * 32 + bit_position;
                    info!(
                        "Found interrupt in Zone {} irq_bitmap: {}",
                        self.id, interrupt_number
                    );
                }
            }
        }
    }

    fn insert_irq_to_bitmap(&mut self, irq: u32) {
        assert!(irq < 1024); // 1024 is the maximum number of interrupts supported by GICv3 (GICD_TYPER.ITLinesNumber)
        let irq_index = irq / 32;
        let irq_bit = irq % 32;
        self.irq_bitmap[irq_index as usize] |= 1 << irq_bit;
    }

    pub fn isa_init(&mut self, fdt: &fdt::Fdt) {
        let cpu_set = self.cpu_set;
        cpu_set.iter().for_each(|cpuid| {
            let cpu_data = get_cpu_data(cpuid);
            let cpu_isa = fdt
                .cpus()
                .find(|cpu| cpu.ids().all().next().unwrap() == cpuid)
                .unwrap()
                .properties()
                .find(|p| p.name == "riscv,isa")
                .unwrap();
            if cpu_isa.as_str().unwrap().contains("sstc") {
                println!("cpu{} support sstc", cpuid);
                cpu_data.arch_cpu.sstc = true;
            }
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct HvArchZoneConfig {
    pub plic_base: usize,
    pub plic_size: usize,
}