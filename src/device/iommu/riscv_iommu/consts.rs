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

#![allow(dead_code)]

use crate::consts::PAGE_SIZE;

use super::regs::IommuMode;

/// This driver's global configuration
pub const IOMMU_MODE: usize = IommuMode::Ddt1Lvl as _;
pub const IOMMU_CQ_PAGE_NUM: u32 = 1;
pub const IOMMU_CQ_ENTRY_SIZE: u32 = 16;
pub const IOMMU_CQ_NUM_ENTRIES: u32 = IOMMU_CQ_PAGE_NUM * PAGE_SIZE as u32 / IOMMU_CQ_ENTRY_SIZE;
pub const IOMMU_FQ_PAGE_NUM: usize = 1;
pub const IOMMU_PQ_PAGE_NUM: usize = 1;
