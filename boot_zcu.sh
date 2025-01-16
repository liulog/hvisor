export CROSS_COMPILE=riscv64-unknown-linux-gnu-

# hvisor
make all LOG=info ARCH=riscv64 FEATURES=zcu102

# cd ~/hypervisor/opensbi-1.5.1
# make clean
# make -j8 ARCH=riscv PLATFORM=generic FW_PAYLOAD_PATH=/home/jingyu/hypervisor/xiangshan/riscv-linux-devel/arch/riscv/boot/Image \
    # FW_FDT_PATH=/home/jingyu/hypervisor/xiangshan/opensbi-devel/kmh-v2-1core.dtb

# make -j8 ARCH=riscv PLATFORM=generic FW_PAYLOAD_PATH=/home/jingyu/hypervisor/hvisor-1core/target/riscv64gc-unknown-none-elf/debug/hvisor.bin
# make distclean
# make -j8 ARCH=riscv PLATFORM=generic FW_PAYLOAD_PATH=/home/jingyu/linux-6.9/arch/riscv/boot/Image FW_FDT_PATH=/home/jingyu/hypervisor/hvisor-1core/images/riscv64/devicetree/kmh.dtb

qemu-system-riscv64 -nographic \
    -M virt -smp 4 -m 512M \
    -kernel ~/hypervisor/hvisor-1core/target/riscv64gc-unknown-none-elf/debug/hvisor.bin \
    # -bios ~/hypervisor/opensbi-1.5.1/build/platform/generic/firmware/fw_payload.bin
