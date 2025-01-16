use crate::{arch::zone::HvArchZoneConfig, config::*};

pub const PLIC_BASE: usize = 0xc000000;
pub const PLIC_MAX_IRQ: usize = 1024;
pub const PLIC_GLOBAL_SIZE: usize = 0x200000;
pub const PLIC_TOTAL_SIZE: usize = 0x400000;
pub const PLIC_MAX_CONTEXT: usize = 64;
pub const PLIC_PRIORITY_BASE: usize = 0x0000;
pub const PLIC_PENDING_BASE: usize = 0x1000;
pub const PLIC_ENABLE_BASE: usize = 0x2000;

pub const ROOT_ZONE_DTB_ADDR: u64 = 0x80f00000;
pub const ROOT_ZONE_KERNEL_ADDR: u64 = 0x81000000;
pub const ROOT_ZONE_ENTRY: u64 = 0x81000000;
pub const ROOT_ZONE_CPUS: u64 = 1 << 0;

pub const ROOT_ZONE_NAME: &str = "root-linux";

// root linux's dtb
#[link_section = ".dtb1"]
#[used]
pub static GUEST1_DTB: [u8; include_bytes!("/home/jingyu/hypervisor/hvisor-1core/images/riscv64/devicetree/linux-1core.dtb").len()] =
    *include_bytes!("/home/jingyu/hypervisor/hvisor-1core/images/riscv64/devicetree/linux-1core.dtb");

// root linux's image
#[link_section = ".img1"]
#[used]
pub static GUEST1: [u8; include_bytes!("/home/jingyu/linux-6.9/arch/riscv/boot/Image").len()] =
    *include_bytes!("/home/jingyu/linux-6.9/arch/riscv/boot/Image");

// ROOT ZONE 的内存配置
pub const ROOT_ZONE_MEMORY_REGIONS: [HvConfigMemoryRegion; 2] = [
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x80f00000,
        virtual_start: 0x80f00000,
        size: 0x7f100000,
    }, // ram
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x10000000,
        virtual_start: 0x10000000,
        size: 0x1000,
    }, // serial
];

pub const ROOT_ZONE_IRQS: [u32; 1] = [10];    // riscv 版本, 这个暂时不影响

pub const ROOT_ARCH_ZONE_CONFIG: HvArchZoneConfig = HvArchZoneConfig {
    plic_base: 0xc000000,
    plic_size: 0x4000000,
};
