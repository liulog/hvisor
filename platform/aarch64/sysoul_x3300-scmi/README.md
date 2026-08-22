# sysoul_x3300-scmi — RK3588 Linux Multi-Zone (NPU + GPU + VOP/HDMI)

## Overview

The `sysoul_x3300-scmi` platform is the **Linux / VirtIO-SCMI variant** of
the `sysoul_x3300` platform (the original Android configuration stays in
`platform/aarch64/sysoul_x3300`). It targets the same **Rockchip RK3588**
SoC board with a multi-zone Linux deployment:

- **Zone 0 (root)**: runs the root Linux, the `hvisor.ko` driver and
  `hvisor-tool`, which provide the VirtIO-SCMI back-end, virtio console,
  block and network devices for the non-root zones.
- **Zone 1**: dedicated to the **NPU** (Cortex-A76 cores 6, 7).
- **Zone 2**: dedicated to the **GPU + VOP + HDMI display** (cores 4, 5).

## Zone Layout

| Zone | CPUs | Devices | SCMI resources |
|------|------|---------|----------------|
| **Zone 1 (npu)** | 6, 7 | RKNPU, virtio-blk/net/console | clocks 0-7, resets 0-5, power 0-2 |
| **Zone 2 (gpu)** | 4, 5 | Mali GPU, VOP2, HDMI0, HD-PHY, VOP IOMMU | clocks 8-11, power 3 |

## Configurations (`configs/`)

| File | Purpose |
|------|---------|
| `zones-npu-gpu-virtio.json` | Launch Zone 1 (NPU) and Zone 2 (GPU) simultaneously |
| `zone1-linux-npu.json` | NPU zone (Zone 1) |
| `zone2-linux-gpu-hdmi.json` | GPU + VOP + HDMI display zone (Zone 2, 2 GB RAM) |

## Device Trees (`image/dts/`)

| File | Purpose |
|------|---------|
| `sysoul_iron.dts` | Board-level device tree (used by u-boot and the root zone) |
| `zone0.dts` | Root zone; declares the `hvisor_virtio_device` node listing the SCMI-exposed clocks/resets/power domains |
| `zone1-linux-npu.dts` | NPU zone: NPU + reserved DMA pool |
| `zone2-linux-gpu-hdmi.dts` | GPU + display zone: GPU, VOP2, HDMI0, hdmiphy, VOP IOMMU |

## Zone 1: NPU Passthrough

| Resource | Details |
|----------|---------|
| MMIO | `0xfdab0000` + `0xfdac0000` + `0xfdad0000` (3 × 64 KB) |
| IRQs (INTID) | 0x8e, 0x8f, 0x90 |
| SCMI | clocks 0-7, resets 0-5, power domains 0-2 |
| DMA | Reserved memory pool at `0x38000000` (64 MB, shared-dma-pool, no-IOMMU mode) |

The NPU uses a `shared-dma-pool` reserved region instead of the
`rockchip,iommu-v2` IOMMU.

## Zone 2: GPU + VOP + HDMI Passthrough

| Resource | Details |
|----------|---------|
| GPU MMIO | `0xfb000000` (2 MB), GPU GRF `0xfd5a0000`, PVTM `0xfdb30000` |
| GPU IRQs (INTID) | JOB=0x7e, MMU=0x7d, GPU=0x7c |
| GPU SCMI | clocks 8-11, power domain 3 |
| RAM | 2 GB at `0x58000000` – `0xd8000000` (IOVAs == PAs via `dma-ranges`) |
| VOP2 | `0xfdd90000`, VOP IOMMU `0xfdd97e00` (passthrough, translated) |
| HDMI | HDMI0 TX `0xfde80000`, HD-PHY `0xfed60000`, plus sys/ioc GRF |

The display pipeline (VOP2 + HDMI0 + hdptx-phy) is fully passed through and
**translated by the VOP IOMMU**, with the display clocks (including
`hdmi0_phy_pll`) delivered through VirtIO-SCMI. The zone2 DTS keeps
`hdmiphy0` disabled on the root side so the clock is not touched by zone0.

### Display notes (verified on hardware)

- The vop2 `plane-mask` of the display zone is `0x3cf` (all 12 windows
  assigned to VP0). u-boot leaves stale VOP windows enabled (e.g. Esmart1/3
  pointing at its logo buffer); the vop2 driver only disables windows that
  belong to the driven VP's plane mask, and only when the mode changes.
  Assigning every window to VP0 lets any mode switch disable all of them,
  otherwise stale windows keep faulting through the IOMMU and the screen
  stays green.
- Do **not** set `iommu.passthrough=1` for this zone: the VOP IOMMU must
  translate the front buffers, and passthrough leaves the iommu group
  domain in a state that breaks re-attach after a mode switch.
- glmark2 runs fullscreen at 4K60 directly after boot (no pre-mode-switch
  needed): `glmark2-es2-drm`.

## Build & Deploy

```bash
make all BID=aarch64/sysoul_x3300-scmi
```

Compile the device trees with `dtc` (see `image/dts/Makefile`), then boot
via u-boot: load `hvisor.bin` with `bootm`, which starts the root zone;
the zones are launched by `hvisor-tool` on the root zone using the JSON
configs above (adjust `kernel_filepath` / `dtb_filepath` to the deployed
file names).

## Verification

- **Zone 1 (NPU)**: check `rknpu` probe in the zone kernel and run an NPU
  workload (e.g. rknn-toolkit-lite).
- **Zone 2 (GPU/display)**: `glmark2-es2-drm` should report 4K60 FPS with
  no `Page fault` / `POST_BUF_EMPTY` in dmesg; `modetest -M rockchip -s
  <connector>@<crtc>:<mode>` mode switches should complete without
  `-EBUSY` or `rk_iommu_force_reset` timeouts.
