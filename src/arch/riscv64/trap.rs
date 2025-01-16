use super::cpu::ArchCpu;
use crate::arch::csr::read_csr;
use crate::arch::csr::*;
use crate::arch::sbi::sbi_vs_handler;
use crate::device::irqchip::plic::{host_plic, vplic_global_emul_handler, vplic_hart_emul_handler};
use crate::event::check_events;
use crate::memory::{GuestPhysAddr, HostPhysAddr};
#[cfg(all(feature = "platform_qemu", target_arch = "riscv64"))]
use crate::platform::qemu_riscv64::*;
#[cfg(all(feature = "kmh_v2_1core", target_arch = "riscv64"))]
use crate::platform::kmh_v2_1core::*;
#[cfg(all(feature = "zcu102", target_arch = "riscv64"))]
use crate::platform::zcu102::*;
use core::arch::{asm, global_asm};
use riscv::register::mtvec::TrapMode;
use riscv::register::stvec;
use riscv::register::{hvip, sie};
use riscv_decode::Instruction;
use crate::percpu::this_zone;
use crate::arch::riscv64::s2pt::DescriptorAttr;
use crate::vmexitinfo;
extern "C" {
    fn _hyp_trap_vector();
}
global_asm!(include_str!("trap.S"),
sync_exception_handler=sym sync_exception_handler,
interrupts_arch_handle=sym interrupts_arch_handle);
#[allow(non_snake_case)]
pub mod ExceptionType {
    pub const ECALL_VU: usize = 8;
    pub const ECALL_VS: usize = 10;
    pub const LOAD_GUEST_PAGE_FAULT: usize = 21;
    pub const STORE_GUEST_PAGE_FAULT: usize = 23;
}
#[allow(non_snake_case)]
pub mod InterruptType {
    pub const SSI: usize = 1;
    pub const STI: usize = 5;
    pub const SEI: usize = 9;
}
pub fn install_trap_vector() {
    unsafe {
        // Set the trap vector.
        stvec::write(_hyp_trap_vector as usize, TrapMode::Direct);
    }
}
pub fn sync_exception_handler(current_cpu: &mut ArchCpu) {
    trace!("current_cpu: stack{:#x}", current_cpu.stack_top);
    let trap_code = read_csr!(CSR_SCAUSE);
    trace!("CSR_SCAUSE: {}", trap_code);
    if (read_csr!(CSR_HSTATUS) & (1 << 7)) == 0 {
        // HSTATUS_SPV
        error!("exception from HS mode");
        info!("trap_code: {:#x}", trap_code);
        info!("HSTATUS: {:#x}", read_csr!(CSR_HSTATUS));
        unreachable!();
    }
    let trap_value = read_csr!(CSR_HTVAL);
    trace!("CSR_HTVAL: {:#x}", trap_value);
    let trap_ins = read_csr!(CSR_HTINST);
    trace!("CSR_HTINST: {:#x}", trap_ins);
    let trap_pc = read_csr!(CSR_SEPC);
    trace!("CSR_SEPC: {:#x}", trap_pc);
    trace!("PC{:#x}", current_cpu.sepc);
    match trap_code {
        ExceptionType::ECALL_VU => {
            error!("ECALL_VU");
        }
        ExceptionType::ECALL_VS => {
            vmexitinfo::increment_ecall_global();
            trace!("ECALL_VS");
            sbi_vs_handler(current_cpu);
            current_cpu.sepc += 4;
        }
        ExceptionType::LOAD_GUEST_PAGE_FAULT => {
            vmexitinfo::increment_load_page_fault_global();
            trace!("LOAD_GUEST_PAGE_FAULT");
            guest_page_fault_handler(current_cpu);
        }
        ExceptionType::STORE_GUEST_PAGE_FAULT => {
            vmexitinfo::increment_store_page_fault_global();
            debug!("STORE_GUEST_PAGE_FAULT");
            guest_page_fault_handler(current_cpu);
        }
        // 20 =>{
            // debug!("INSTRCTION_GUEST_PAGE_FAULT");
            // info!("flag: {}", DescriptorAttr::from_bits_truncate((DescriptorAttr::ACCESSED | DescriptorAttr::WRITABLE | DescriptorAttr::READABLE | DescriptorAttr::USER | DescriptorAttr::EXECUTABLE)));
            // unsafe {
            //     let r = this_zone().write().gpm.page_table_update(current_cpu.sepc);
            // }
        // }
        _ => {
            // let value = read_csr!(CSR_HGATP);
            // info!("CSR_HGATP: {:#x}", value);

            warn!(
                "CPU {} trap {},sepc: {:#x}",
                current_cpu.cpuid, trap_code, current_cpu.sepc
            );
            warn!("trap info: {} {:#x} {:#x}", trap_code, trap_value, trap_ins);
            let raw_inst = read_inst(trap_pc);
            let inst = riscv_decode::decode(raw_inst);
            warn!("trap ins: {:#x}  {:?}", raw_inst, inst);
            // current_cpu.sepc += 4;
            error!("unhandled trap");
            unreachable!();
            // current_cpu.idle();                     // 去执行 idle
        }
    }
}
pub fn guest_page_fault_handler(current_cpu: &mut ArchCpu) {
    vmexitinfo::increment_page_fault_global();
    // info!("guest_page_fault_handler!");
    let addr: HostPhysAddr = read_csr!(CSR_HTVAL) << 2;
    trace!("guest page fault at {:#x}", addr);
    let host_plic_base = host_plic().read().base;
    let mut ins_size: usize = 0;
    //TODO: get plic addr range from dtb or vpliv object
    if addr >= host_plic_base && addr < host_plic_base + PLIC_TOTAL_SIZE {
        trace!("PLIC access");
        let mut inst: u32 = read_csr!(CSR_HTINST) as u32;
        if inst == 0 {
            let inst_addr: GuestPhysAddr = current_cpu.sepc;
            //load real ins from guest memmory
            inst = read_inst(inst_addr);
            ins_size = if inst & 0x3 == 3 { 4 } else { 2 };
        } else if inst == 0x3020 || inst == 0x3000 {
            // TODO: we should reinject this in the guest as a fault access
            error!("fault on 1st stage page table walk");
        } else {
            // If htinst is valid and is not a pseudo instructon make sure
            // the opcode is valid even if it was a compressed instruction,
            // but before save the real instruction size.
            ins_size = if (inst) & 0x2 == 0 { 2 } else { 4 };
            inst = inst | 0b10;
            // error!("unhandled guest page fault at {:#x}", addr);
            // panic!("inst{:#x}", inst);
        }
        //TODO: decode inst to real instruction
        let (_len, inst) = decode_inst(inst);
        if let Some(inst) = inst {
            if addr >= host_plic_base + PLIC_GLOBAL_SIZE {
                vplic_hart_emul_handler(current_cpu, addr, inst);
            } else {
                vplic_global_emul_handler(current_cpu, addr, inst);
            }
            current_cpu.sepc += ins_size;
        } else {
            error!("Invalid instruction at {:#x}", current_cpu.sepc);
            panic!();
        }
    } else {
        panic!("CPU {} unmaped memmory at {:#x}", current_cpu.cpuid, addr);
    }
}
fn read_inst(addr: GuestPhysAddr) -> u32 {
    let mut ins: u32;
    if addr & 0b1 != 0 {
        error!("trying to read guest unaligned instruction");
    }
    //
    //  Read 16 bits at a time to make sure the access is aligned. If the instruction is not
    //  compressed, read the following 16-bits.
    //
    ins = hlvxhu(addr) as u32;
    if (ins & 0b11) == 3 {
        ins |= (hlvxhu(addr + 2) as u32) << 16;
    }
    ins
}
fn hlvxhu(addr: GuestPhysAddr) -> u64 {
    let mut value: u64;
    unsafe {
        asm!(
            ".insn r 0x73, 0x4, 0x32, {0}, {1}, x3",
            out(reg) value,
            in(reg) addr,
        );
    }
    value
}
/// decode risc-v instruction, return (inst len, inst)
fn decode_inst(inst: u32) -> (usize, Option<Instruction>) {
    let i1 = inst as u16;
    let len = riscv_decode::instruction_length(i1);
    let inst = match len {
        2 => i1 as u32,
        4 => inst as u32,
        _ => unreachable!(),
    };
    (len, riscv_decode::decode(inst).ok())
}
/// handle external interrupt
pub fn interrupts_arch_handle(current_cpu: &mut ArchCpu) {
    vmexitinfo::increment_interrupt_global();
    trace!("interrupts_arch_handle @CPU{}", current_cpu.cpuid);
    let trap_code: usize;
    trap_code = read_csr!(CSR_SCAUSE);
    trace!("CSR_SCAUSE: {:#x}", trap_code);
    match trap_code & 0xfff {
        InterruptType::STI => {
            vmexitinfo::increment_timer_interrupt_global();
            // trace!("STI on CPU{}", current_cpu.cpuid);
            // info!("hvip{:#x}", read_csr!(CSR_HVIP));
            unsafe {
                hvip::set_vstip();      // 设置 VSTIP
                sie::clear_stimer();    // 暂时关闭使能 sie 的中断使能位, 不继续产生中断
            }
            // info!("hvip{:#x}", read_csr!(CSR_HVIP));
            // info!("sip{:#x}", read_csr!(CSR_SIP));
            // info!("sie {:#x}", read_csr!(CSR_SIE));
        }
        InterruptType::SSI => {
            vmexitinfo::increment_soft_interrupt_global();
            trace!("SSI on CPU {}", current_cpu.cpuid);
            handle_ssi(current_cpu);
        }
        InterruptType::SEI => {
            vmexitinfo::increment_external_interrupt_global();
            debug!("SEI on CPU {}", current_cpu.cpuid);
            handle_eirq(current_cpu)
        }
        _ => {
            error!(
                "unhandled trap {:#x},sepc: {:#x}",
                trap_code, current_cpu.sepc
            );
            unreachable!();
        }
    }
}

/// handle interrupt request(current only external interrupt)
pub fn handle_eirq(current_cpu: &mut ArchCpu) {
    // TODO: handle other irq
    // check external interrupt && handle
    // sifive plic: context0=>cpu0,M mode,context1=>cpu0,S mode...
    let context_id = 2 * current_cpu.cpuid + 1;
    let host_plic = host_plic();
    let claim_and_complete_addr =
        host_plic.read().base + PLIC_GLOBAL_SIZE + 0x1000 * context_id + 0x4;
    let irq = unsafe { core::ptr::read_volatile(claim_and_complete_addr as *const u32) };
    debug!(
        "CPU{} get external irq{}@{:#x}",
        current_cpu.cpuid, irq, claim_and_complete_addr
    );
    host_plic.write().claim_complete[context_id] = irq;
    // set external interrupt pending, which trigger guest interrupt
    unsafe { hvip::set_vseip() };
}

pub fn handle_ssi(current_cpu: &mut ArchCpu) {
    trace!("handle_ssi");
    let sip = read_csr!(CSR_SIP);
    trace!("CPU{} sip: {:#x}", current_cpu.cpuid, sip);
    clear_csr!(CSR_SIP, 1 << 1);
    let sip2 = read_csr!(CSR_SIP);
    trace!("CPU{} sip*: {:#x}", current_cpu.cpuid, sip2);

    trace!("hvip: {:#x}", read_csr!(CSR_HVIP));
    set_csr!(CSR_HVIP, 1 << 2);
    check_events();
}
