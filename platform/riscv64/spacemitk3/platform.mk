
BOOT_PATH        := $(image_dir)/../

HVISOR_RAW_BIN_ABS := $(abspath $(hvisor_bin).tmp)

$(hvisor_bin): elf
	@if ! command -v mkimage > /dev/null; then \
		sudo apt-get install -y u-boot-tools; \
	fi
	$(OBJCOPY) $(hvisor_elf) --strip-all -O binary $(hvisor_bin).tmp
	cp $(BOOT_PATH)hvisor.its $(BOOT_PATH)hvisor.its.tmp
	sed -i 's|hvisor.bin.tmp|$(HVISOR_RAW_BIN_ABS)|g' $(BOOT_PATH)hvisor.its.tmp
	mkimage -f $(BOOT_PATH)hvisor.its.tmp $(hvisor_bin)
	rm -f $(hvisor_bin).tmp $(BOOT_PATH)hvisor.its.tmp

# Pack RISC-V Linux Image + test.dtb into a FIT for U-Boot bootm.
# Usage:
#   make ARCH=riscv64 BOARD=spacemitk3 linux-test-fit \
#       LINUX_IMAGE=/path/to/Image
# Load addresses match board.rs ROOT_ZONE_KERNEL_ADDR / ROOT_ZONE_DTB_ADDR.
LINUX_IMAGE      ?= $(image_dir)/kernel/Image
LINUX_TEST_DTB   := $(abspath $(image_dir)/dts/test.dtb)
LINUX_TEST_FIT   := $(BOOT_PATH)linux-test.bin
LINUX_TEST_ITS   := $(BOOT_PATH)linux-test.its
LINUX_TEST_ITS_TMP := $(BOOT_PATH)linux-test.its.tmp

.PHONY: linux-test-dtb linux-test-fit linux-test-cp

linux-test-dtb:
	$(MAKE) -C $(image_dir)/dts test.dtb

linux-test-fit: linux-test-dtb
	@if ! command -v mkimage > /dev/null; then \
		sudo apt-get install -y u-boot-tools; \
	fi
	@if [ ! -f "$(LINUX_IMAGE)" ]; then \
		echo "Error: LINUX_IMAGE not found: $(LINUX_IMAGE)"; \
		echo "Set LINUX_IMAGE= to your RISC-V Linux Image path."; \
		exit 1; \
	fi
	@if [ ! -f "$(LINUX_TEST_DTB)" ]; then \
		echo "Error: test.dtb not found: $(LINUX_TEST_DTB)"; \
		exit 1; \
	fi
	cp $(LINUX_TEST_ITS) $(LINUX_TEST_ITS_TMP)
	sed -i 's|__LINUX_IMAGE__|$(abspath $(LINUX_IMAGE))|g' $(LINUX_TEST_ITS_TMP)
	sed -i 's|__LINUX_DTB__|$(LINUX_TEST_DTB)|g' $(LINUX_TEST_ITS_TMP)
	mkimage -f $(LINUX_TEST_ITS_TMP) $(LINUX_TEST_FIT)
	rm -f $(LINUX_TEST_ITS_TMP)
	@echo "Generated FIT: $(abspath $(LINUX_TEST_FIT))"
	@echo "  kernel: $(abspath $(LINUX_IMAGE))"
	@echo "  fdt:    $(LINUX_TEST_DTB)"

linux-test-cp: linux-test-fit
	cp $(LINUX_TEST_FIT) ~/tftp
