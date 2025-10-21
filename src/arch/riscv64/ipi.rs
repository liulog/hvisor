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
//
use crate::consts::{IPI_EVENT_HART_RESUME, IPI_EVENT_HART_SUSPEND};
use crate::consts::{IPI_EVENT_SEND_IPI, IPI_EVENT_UPDATE_HART_LINE};
use crate::percpu::this_cpu_data;
use sbi_rt::HartMask;
use sbi_rt::SbiRet;
use crate::arch::cpu::CpuState;
use riscv::asm::{wfi, fence};
use crate::percpu::CpuSet;

// arch_send_event
pub fn arch_send_event(cpu_id: u64, _sgi_num: u64) {
    debug!("arch_send_event: cpu_id: {}", cpu_id);
    #[cfg(feature = "aclint")]
    crate::device::irqchip::aclint::aclint_send_ipi(cpu_id as usize);
    #[cfg(not(feature = "aclint"))]
    {
        let sbi_ret: SbiRet = sbi_rt::send_ipi(HartMask::from_mask_base(1 << cpu_id, 0));
        if sbi_ret.is_err() {
            error!("arch_send_event: send_ipi failed: {:?}", sbi_ret);
        }
    }
}

/// Handle send_ipi event.
fn arch_ipi_handler() {
    unsafe {
        riscv_h::register::hvip::set_vssip();
    }
}

/// Handle hart suspend event.
fn arch_hart_suspend() {
    this_cpu_data().arch_cpu.state = CpuState::Suspended;
    // temporarily disable timer interrupt
    unsafe {
        riscv::register::sie::clear_stimer();
    }
}

/// Handle hart resume event.
fn arch_hart_resume() {
    this_cpu_data().arch_cpu.state = CpuState::Started;
    // re-enable timer interrupt
    unsafe {
        riscv::register::sie::set_stimer();
    }
}

/// Wait for other cpus in the cpu set to suspend.
pub fn wait_for_other_cpus_suspend(cpu_set: CpuSet) {
    let this_cpu_id = crate::arch::cpu::this_cpu_id();
    for target_cpu_id in cpu_set.iter() {
        if target_cpu_id == this_cpu_id {
            continue;
        }
        // Wait for the cpu to suspend.
        while crate::percpu::get_cpu_data(target_cpu_id).arch_cpu.state != CpuState::Suspended {
            fence();
        }
    }
}

pub fn arch_check_events(event: Option<usize>) {
    match event {
        #[cfg(feature = "plic")]
        Some(IPI_EVENT_UPDATE_HART_LINE) => {
            use crate::device::irqchip::plic::update_hart_line;
            update_hart_line();
        }
        Some(IPI_EVENT_SEND_IPI) => {
            use crate::arch::riscv64::ipi::arch_ipi_handler;
            arch_ipi_handler();
        }
        Some(IPI_EVENT_HART_SUSPEND) => {
            arch_hart_suspend();
        }
        Some(IPI_EVENT_HART_RESUME) => {
            arch_hart_resume();
        }
        _ => {
            panic!("arch_check_events: unhandled event: {:?}", event);
        }
    }
}

pub fn arch_prepare_send_event(cpu_id: usize, ipi_int_id: usize, event_id: usize) {
    debug!("risc-v arch_prepare_send_event: do nothing now.")
}
