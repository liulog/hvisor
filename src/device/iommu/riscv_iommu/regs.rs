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

#![allow(unused)]

/// Capabilities register fields
pub const RV_IOMMU_CAPS_VERSION_MASK: u64 = 0xFF << 0;
pub const RV_IOMMU_CAPS_SV32_BIT: u64 = 0x1 << 8;
pub const RV_IOMMU_CAPS_SV39_BIT: u64 = 0x1 << 9;
pub const RV_IOMMU_CAPS_SV48_BIT: u64 = 0x1 << 10;
pub const RV_IOMMU_CAPS_SV57_BIT: u64 = 0x1 << 11;
pub const RV_IOMMU_CAPS_SVPBMT_BIT: u64 = 0x1 << 15;
pub const RV_IOMMU_CAPS_SV32X4_BIT: u64 = 0x1 << 16;
pub const RV_IOMMU_CAPS_SV39X4_BIT: u64 = 0x1 << 17;
pub const RV_IOMMU_CAPS_SV48X4_BIT: u64 = 0x1 << 18;
pub const RV_IOMMU_CAPS_SV57X4_BIT: u64 = 0x1 << 19;
pub const RV_IOMMU_CAPS_AMO_MRIF_BIT: u64 = 0x1 << 21;
pub const RV_IOMMU_CAPS_MSI_FLAT_BIT: u64 = 0x1 << 22;
pub const RV_IOMMU_CAPS_MSI_MRIF_BIT: u64 = 0x1 << 23;
pub const RV_IOMMU_CAPS_AMO_HWAD_BIT: u64 = 0x1 << 24;
pub const RV_IOMMU_CAPS_ATS_BIT: u64 = 0x1 << 25;
pub const RV_IOMMU_CAPS_T2GPA_BIT: u64 = 0x1 << 26;
pub const RV_IOMMU_CAPS_END_BIT: u64 = 0x1 << 27;
pub const RV_IOMMU_CAPS_IGS_MASK: u64 = 0x3 << 28;
pub const RV_IOMMU_CAPS_HPM_BIT: u64 = 0x1 << 30;
pub const RV_IOMMU_CAPS_DBG_BIT: u64 = 0x1 << 31;
pub const RV_IOMMU_CAPS_PAS_MASK: u64 = 0x3F << 32;
pub const RV_IOMMU_CAPS_PD8_BIT: u64 = 0x1 << 38;
pub const RV_IOMMU_CAPS_PD17_BIT: u64 = 0x1 << 39;
pub const RV_IOMMU_CAPS_PD20_BIT: u64 = 0x1 << 40;

pub const RV_IOMMU_SUPPORTED_VERSION: u64 = 0x10;
pub const RV_IOMMU_IGS_MSI: u64 = 0;
pub const RV_IOMMU_IGS_WSI: u64 = 1;
pub const RV_IOMMU_IGS_BOTH: u64 = 2;

/// Features control register fields
pub const RV_IOMMU_FCTL_DEFAULT: u32 = 0x1 << 1;
pub const RV_IOMMU_FCTL_BE_BIT: u32 = 0x1 << 0;
pub const RV_IOMMU_FCTL_WSI_BIT: u32 = 0x1 << 1;
pub const RV_IOMMU_FCTL_GXL_BIT: u32 = 0x1 << 2;

/// Device-directory-table pointer fields
pub const RV_IOMMU_DDTP_MODE_MASK: usize = 0xF;
pub const RV_IOMMU_DDTP_BUSY_BIT: usize = 0x1 << 4;
pub const RV_IOMMU_DDTP_PPN_MASK: usize = 0xFFF_FFFF_FFFF << 10; // [53:10] PPN

/// Iommu Mode
#[derive(Debug, PartialEq, Eq)]
pub enum IommuMode {
    Off = 0x0,     // No inbound memory transactions are allowed
    Bare = 0x1,    // No translation or protection
    Ddt1Lvl = 0x2, // One-level device-directory-table
    Ddt2Lvl = 0x3, // Two-level device-directory-table
    Ddt3Lvl = 0x4, // Three-level device-directory-table
}

impl TryFrom<usize> for IommuMode {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::Bare),
            2 => Ok(Self::Ddt1Lvl),
            3 => Ok(Self::Ddt2Lvl),
            4 => Ok(Self::Ddt3Lvl),
            _ => Err(()),
        }
    }
}

/// DDT entry. fsc register fields
pub const RV_IOMMU_FSC_MODE_MASK: u64 = 0xF << 60;
pub const RV_IOMMU_FSC_PPN_MASK: u64 = 0xFFF_FFFF_FFFF;

#[derive(Debug, PartialEq, Eq)]
pub enum IosatpMode {
    Bare = 0x0,
    Sv39 = 0x8,
    Sv48 = 0x9,
    Sv57 = 0xA,
}

impl TryFrom<usize> for IosatpMode {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0x0 => Ok(Self::Bare),
            0x8 => Ok(Self::Sv39),
            0x9 => Ok(Self::Sv48),
            0xA => Ok(Self::Sv57),
            _ => Err(()),
        }
    }
}

/// Device-context fields
pub const RV_IOMMU_DC_TC_VALID_BIT: u64 = 1;
pub const RV_IOMMU_DC_TC_EN_ATS_BIT: u64 = 1 << 1;
pub const RV_IOMMU_DC_TC_EN_PRI_BIT: u64 = 1 << 2;
pub const RV_IOMMU_DC_TC_T2GPA_BIT: u64 = 1 << 3;
pub const RV_IOMMU_DC_TC_DTF_BIT: u64 = 1 << 4;
pub const RV_IOMMU_DC_TC_PDTV_BIT: u64 = 1 << 5;
pub const RV_IOMMU_DC_TC_PRPR_BIT: u64 = 1 << 6;
pub const RV_IOMMU_DC_TC_GADE_BIT: u64 = 1 << 7;
pub const RV_IOMMU_DC_TC_SADE_BIT: u64 = 1 << 8;
pub const RV_IOMMU_DC_TC_DPE_BIT: u64 = 1 << 9;
pub const RV_IOMMU_DC_TC_SBE_BIT: u64 = 1 << 10;
pub const RV_IOMMU_DC_TC_SXL_BIT: u64 = 1 << 11;
pub const RV_IOMMU_DC_IOHGATP_MODE_MASK: u64 = 0xF << 60;
pub const RV_IOMMU_DC_IOHGATP_GSCID_MASK: u64 = 0xFFFF << 44;
pub const RV_IOMMU_DC_IOHGATP_PPN_MASK: u64 = 0xFFF_FFFF_FFFF;

/// Icvec register fields
pub const RV_IOMMU_ICVEC_CIV_MASK: u64 = 0xF;
pub const RV_IOMMU_ICVEC_FIV_MASK: u64 = 0xF << 4;
pub const RV_IOMMU_ICVEC_PMIV_MASK: u64 = 0xF << 8;
pub const RV_IOMMU_ICVEC_PIV_MASK: u64 = 0xF << 12;
pub const RV_IOMMU_ICVEC_MASK: u64 = RV_IOMMU_ICVEC_CIV_MASK | RV_IOMMU_ICVEC_FIV_MASK;

/// Interrupt pending status register fields
pub const RV_IOMMU_IPSR_CIP_BIT: u32 = 1;
pub const RV_IOMMU_IPSR_FIP_BIT: u32 = 1 << 1;
pub const RV_IOMMU_IPSR_PMIP_BIT: u32 = 1 << 2;
pub const RV_IOMMU_IPSR_PIP_BIT: u32 = 1 << 3;
pub const RV_IOMMU_IPSR_CLEAR: u32 =
    RV_IOMMU_IPSR_CIP_BIT | RV_IOMMU_IPSR_FIP_BIT | RV_IOMMU_IPSR_PMIP_BIT | RV_IOMMU_IPSR_PIP_BIT;

/// In-memory Queue common fields
pub const RV_IOMMU_XQCSR_XQEN_BIT: u32 = 1 << 0;
pub const RV_IOMMU_XQCSR_XIE_BIT: u32 = 1 << 1;
pub const RV_IOMMU_FQCSR_XQMF_BIT: u32 = 1 << 8;
pub const RV_IOMMU_XQCSR_XQON_BIT: u32 = 1 << 16;
pub const RV_IOMMU_XQCSR_BUSY_BIT: u32 = 1 << 17;
/// Command queue CSR fields
pub const RV_IOMMU_CQCSR_CMD_TO_BIT: u32 = 1 << 9;
pub const RV_IOMMU_CQCSR_CMD_ILL_BIT: u32 = 1 << 10;
pub const RV_IOMMU_CQCSR_CMD_FENCE_WIP_BIT: u32 = 1 << 11;
/// Fault queue CSR fields
pub const RV_IOMMU_FQCSR_FQOF_BIT: u32 = 1 << 9;
/// Page-request queue CSR fields
pub const RV_IOMMU_PQCSR_PQOF_BIT: u32 = 1 << 9;
