use crate::{arch::zone::HvArchZoneConfig, config::*};

// PLIC
pub const PLIC_BASE: usize = 0x3c000000;
pub const PLIC_MAX_IRQ: usize = 96;
pub const PLIC_GLOBAL_SIZE: usize = 0x200000;
pub const PLIC_TOTAL_SIZE: usize = 0x400000;
pub const PLIC_MAX_CONTEXT: usize = 64;
pub const PLIC_PRIORITY_BASE: usize = 0x0000;
pub const PLIC_PENDING_BASE: usize = 0x1000;
pub const PLIC_ENABLE_BASE: usize = 0x2000;

// ROOT ZONE 相关的约定 (这里是 virtual addr)
pub const ROOT_ZONE_DTB_ADDR: u64 = 0x80E00000;
pub const ROOT_ZONE_KERNEL_ADDR: u64 = 0x81000000;
pub const ROOT_ZONE_ENTRY: u64 = 0x81000000;
pub const ROOT_ZONE_CPUS: u64 = 1 << 0;

pub const ROOT_ZONE_NAME: &str = "root-linux";

// 0x80E00000
// root linux's dtb
#[link_section = ".dtb1"]
#[used]
pub static GUEST1_DTB: [u8; include_bytes!("/home/jingyu/hypervisor/hvisor-1core/images/riscv64/devicetree/kmh_v2_1core.dtb").len()] =
    *include_bytes!("/home/jingyu/hypervisor/hvisor-1core/images/riscv64/devicetree/kmh_v2_1core.dtb");

// root linux's image
// #[link_section = ".img1"]
// #[used]
// pub static GUEST1: [u8; include_bytes!("/home/jingyu/hypervisor/linuxloader/linux.bin").len()] =
//     *include_bytes!("/home/jingyu/hypervisor/linuxloader/linux.bin");

#[link_section = ".img1"]
#[used]
pub static GUEST1: [u8; include_bytes!("/home/jingyu/hypervisor/xiangshan/riscv-linux-devel/arch/riscv/boot/Image").len()] =
    *include_bytes!("/home/jingyu/hypervisor/xiangshan/riscv-linux-devel/arch/riscv/boot/Image");

// #[link_section = ".img1"]
// #[used]
// pub static GUEST1: [u8; include_bytes!("/home/jingyu/hypervisor/hvisor-1core/Image-unixbench").len()] =
//     *include_bytes!("/home/jingyu/hypervisor/hvisor-1core/Image-unixbench");

// #[link_section = ".img1"]
// #[used]
// pub static GUEST1: [u8; include_bytes!("/home/jingyu/hyperbench-riscv-rs/target/riscv64gc-unknown-none-elf/debug/hyperbench-riscv-rs.bin").len()] =
//     *include_bytes!("/home/jingyu/hyperbench-riscv-rs/target/riscv64gc-unknown-none-elf/debug/hyperbench-riscv-rs.bin");

// /home/jingyu/hyperbench-riscv-rs/hyperbench-riscv-rs/target/riscv64gc-unknown-none-elf/debug/hyperbench-riscv-rs.bin

// 0x81000000
// #[link_section = ".img1"]
// #[used]
// pub static GUEST1: [u8; include_bytes!("/home/jingyu/backup/rt-thread/bsp/qemu-virt64-riscv/rtthread.bin").len()] =
    // *include_bytes!("/home/jingyu/backup/rt-thread/bsp/qemu-virt64-riscv/rtthread.bin");


// ROOT ZONE 的内存配置
pub const ROOT_ZONE_MEMORY_REGIONS: [HvConfigMemoryRegion; 2] = [
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x80E00000,     // 物理位置
        virtual_start: 0x80E00000,      // 对于 bench 可以这样限制
        size: 0x7F200000,               // 和 kmh 保持一致, 这部分对比 rocket，设置成一样的配置
    }, // ram
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x310B0000,
        virtual_start: 0x310B0000,
        size: 0x10000,
    }, // serial
];

pub const ROOT_ZONE_IRQS: [u32; 1] = [40];

// ROOT ZONE 的关于 PLIC 的配置
pub const ROOT_ARCH_ZONE_CONFIG: HvArchZoneConfig = HvArchZoneConfig {
    plic_base: 0x3c000000,
    plic_size: 0x1000000,
};
