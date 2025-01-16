export CROSS_COMPILE=riscv64-unknown-linux-gnu-

# hvisor
make all LOG=info ARCH=riscv64 FEATURES=kmh_v2_1core

cd ~/hypervisor/opensbi-1.5.1
# ./bo  make clean
# make -j8 ARCH=riscv PLATFORM=generic FW_PAYLOAD_PATH=/home/jingyu/hypervisor/xiangshan/riscv-linux-devel/arch/riscv/boot/Image FW_FDT_PATH=/home/jingyu/hypervisor/xiangshan/opensbi-devel/kmh-v2-1core.dtb

make clean
make -j8 ARCH=riscv PLATFORM=generic FW_PAYLOAD_PATH=/home/jingyu/hypervisor/hvisor-1core/target/riscv64gc-unknown-none-elf/debug/hvisor.bin FW_FDT_PATH=/home/jingyu/hypervisor/xiangshan/opensbi-devel/kmh-v2-1core.dtb FW_PAYLOAD_FDT_ADDR=0xBFE00000
# make distclean
# make -j8 ARCH=riscv PLATFORM=generic FW_PAYLOAD_PATH=/home/jingyu/hypervisor/xiangshan/riscv-linux-devel/arch/riscv/boot/Image FW_FDT_PATH=/home/jingyu/hypervisor/xiangshan/opensbi-devel/kmh-v2-1core.dtb FW_PAYLOAD_FDT_ADDR=0xBFE00000

/home/jingyu/hypervisor/xiangshan/qemu-devel/build/qemu-system-riscv64 -nographic \
    -M bosc-kmh -smp 1 -m 2G \
    -bios ~/hypervisor/opensbi-1.5.1/build/platform/generic/firmware/fw_payload.bin                         # -d mmu,int -D qemu.log


# cd ~/hypervisor/opensbi-1.5.1
# # make distclean
# make -j8 ARCH=riscv PLATFORM=generic FW_PAYLOAD_PATH=/home/jingyu/hypervisor/hvisor-1core/target/riscv64gc-unknown-none-elf/debug/hvisor.bin FW_FDT_PATH=../xiangshan/opensbi-devel/kmh-v2-1core.dtb

# /home/jingyu/hypervisor/xiangshan/qemu-devel/build/qemu-system-riscv64 -nographic \
#     -M bosc-kmh -smp 1 -m 2G \
#     -bios ~/hypervisor/opensbi-1.5.1/build/platform/generic/firmware/fw_payload.bin # -d mmu,int -D qemu.log

# linux
# cd ~/hypervisor/xiangshan/opensbi-devel
# make distclean
# make ARCH=riscv PLATFORM=generic FW_PAYLOAD_PATH=/home/jingyu/linux-6.9/arch/riscv/boot/Image # FW_FDT_PATH=./kmh-v2-1core.dtb 

# ../qemu-devel/build/qemu-system-riscv64 -nographic \
#     -M virt -smp 1 -m 2G \
#     -bios ~/hypervisor/xiangshan/opensbi-devel/build/platform/generic/firmware/fw_payload.bin

# 注意: 
#    启动命令中不能使用 -cpu rv64, 因为cpu的定义已经放在在cpu-qom.h里面: TYPE_RISCV_CPU_BOSC_KMH
#    如果启动命令包含 -cpu rv64, 那么QEMU 就用默认的TYPE_RISCV_CPU_BASE64启动, 而不是cpu-qom.h中定义的昆明湖CPU.