QEMU := qemu-system-riscv64


FSIMG1 := $(image_dir)/virtdisk/rootfs1.ext4
FSIMG2 := $(image_dir)/virtdisk/rootfs-busybox.qcow2
# HVISOR ENTRY
HVISOR_ENTRY_PA := 0x80200000
zone0_kernel := $(image_dir)/kernel/Image
zone0_dtb    := $(image_dir)/dts/zone0.dtb
# zone1_kernel := $(image_dir)/kernel/Image
# zone1_dtb    := $(image_dir)/devicetree/linux.dtb

QEMU_ARGS := -machine virt,aclint=on,iommu-sys=on
QEMU_ARGS += -bios default
QEMU_ARGS += -cpu rv64
QEMU_ARGS += -smp 4
QEMU_ARGS += -m 4G
QEMU_ARGS += -nographic

QEMU_ARGS += -kernel $(hvisor_bin)
QEMU_ARGS += -device loader,file="$(zone0_kernel)",addr=0x90000000,force-raw=on
QEMU_ARGS += -device loader,file="$(zone0_dtb)",addr=0x8f000000,force-raw=on
# QEMU_ARGS += -device loader,file="$(zone1_kernel)",addr=0x84000000,force-raw=on
# QEMU_ARGS += -device loader,file="$(zone1_dtb)",addr=0x83000000,force-raw=on

QEMU_ARGS += -drive if=none,file=$(FSIMG1),id=X10008000,format=raw
# QEMU_ARGS += -device virtio-blk-device,drive=X10008000,bus=virtio-mmio-bus.7
QEMU_ARGS += -device virtio-blk-pci,drive=X10008000,iommu_platform=on,disable-legacy=on,bus=pcie.0,addr=01.0
QEMU_ARGS += -device virtio-serial-device,bus=virtio-mmio-bus.6 -chardev pty,id=X10007000 -device virtconsole,chardev=X10007000 -s -S
QEMU_ARGS += -drive if=none,file=$(FSIMG2),id=X10006000,format=qcow2
# QEMU_ARGS += -device virtio-blk-device,drive=X10006000,bus=virtio-mmio-bus.5
QEMU_ARGS += -device virtio-blk-pci,drive=X10006000,iommu_platform=on,disable-legacy=on,bus=pcie.0,addr=02.0

ifeq ($(IOMMU_TRACE), 1)
  QEMU_ARGS += -d trace:riscv_iommu_*  # -D hvisor.log
endif

$(hvisor_bin): elf
	$(OBJCOPY) $(hvisor_elf) --strip-all -O binary $@