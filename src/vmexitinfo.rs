// use spin::RwLock;
// use alloc::sync::Arc;
use spin::Mutex;
use crate::hypercall::HyperCallResult;

/// A structure to track VM exit statistics.
#[derive(Debug, Default)]
struct VmExitInfo {
    /// Number of exits caused by interrupts.
    interrupt_exits: u64,
    timer_interrupt: u64,
    soft_interrupt: u64,
    external_interrupt: u64,
    /// Number of exits caused by ECALL instructions.
    ecall_exits: u64,
    hsm_ecall: u64,
    hvisor_ecall: u64,
    timer_ecall: u64,
    // 除了以上三种 ecall, 其余的 ecall 都是直接调用 sbi 接口
    /// Number of exits caused by page faults.
    page_fault_exits: u64,
    load_page_fault: u64,
    store_page_fault: u64,
}

impl VmExitInfo {
    /// Create a new `VmExitInfo` instance with all counts set to zero.
    fn new() -> Self {
        Self {
            interrupt_exits: 0,
            timer_interrupt: 0,
            soft_interrupt: 0,
            external_interrupt: 0,
            ecall_exits: 0,
            hsm_ecall: 0,
            hvisor_ecall: 0,
            timer_ecall: 0,
            page_fault_exits: 0,
            load_page_fault: 0,
            store_page_fault: 0,
        }
    }
    /// Increment the interrupt exit count.
    fn increment_interrupt(&mut self) {
        self.interrupt_exits += 1;
    }

    fn increment_timer_interrupt(&mut self) {
        self.timer_interrupt += 1;
    }

    fn increment_soft_interrupt(&mut self) {
        self.soft_interrupt += 1;
    }

    fn increment_external_interrupt(&mut self) {
        self.external_interrupt += 1;
    }

    /// Increment the ECALL exit count.
    fn increment_ecall(&mut self) {
        self.ecall_exits += 1;
    }

    fn increment_hsm_ecall(&mut self) {
        self.hsm_ecall += 1;
    }

    fn increment_hvisor_ecall(&mut self) {
        self.hvisor_ecall += 1;
    }

    fn increment_timer_ecall(&mut self) {
        self.timer_ecall += 1;
    }

    /// Increment the page fault exit count.
    fn increment_page_fault(&mut self) {
        self.page_fault_exits += 1;
    }

    fn increment_load_page_fault(&mut self) {
        self.load_page_fault += 1;
    }

    fn increment_store_page_fault(&mut self) {
        self.store_page_fault += 1;
    }

    /// Clear all counters.
    fn clear(&mut self) {
        self.interrupt_exits = 0;
        self.timer_interrupt = 0;
        self.soft_interrupt = 0;
        self.external_interrupt = 0;
        self.ecall_exits = 0;
        self.hsm_ecall = 0;
        self.hvisor_ecall = 0;
        self.timer_ecall = 0;
        self.page_fault_exits = 0;
        self.load_page_fault = 0;
        self.store_page_fault = 0;
    }

    /// Print a summary of the exit statistics.
    fn print_summary(&self) {
        println!("VM exit summary:");
        println!("  Interrupt exits: {}", self.interrupt_exits);
        println!("    Timer interrupt: {}", self.timer_interrupt);
        println!("    Soft interrupt: {}", self.soft_interrupt);
        println!("    External interrupt: {}", self.external_interrupt);
        println!("  ECALL exits: {}", self.ecall_exits);
        println!("    HSM ECALL: {}", self.hsm_ecall);
        println!("    HVISOR ECALL: {}", self.hvisor_ecall);
        println!("    Timer ECALL: {}", self.timer_ecall);
        println!("  Page fault exits: {}", self.page_fault_exits);
        println!("    Load page fault: {}", self.load_page_fault);
        println!("    Store page fault: {}", self.store_page_fault);
    }
}

lazy_static! {
    static ref GLOBAL_VM_EXIT_INFO: Mutex<VmExitInfo> = {
        let mut info =  VmExitInfo::new();
        Mutex::new(info)
    };
}

/// Increment the interrupt exit count in the global variable.
pub fn increment_interrupt_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_interrupt();
}

pub fn increment_timer_interrupt_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_timer_interrupt();
}

pub fn increment_soft_interrupt_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_soft_interrupt();
}

pub fn increment_external_interrupt_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_external_interrupt();
}

/// Increment the ECALL exit count in the global variable.
pub fn increment_ecall_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_ecall();
}

pub fn increment_hsm_ecall_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_hsm_ecall();
}

pub fn increment_hvisor_ecall_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_hvisor_ecall();
}

pub fn increment_timer_ecall_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_timer_ecall();
}

/// Increment the page fault exit count in the global variable.
pub fn increment_page_fault_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_page_fault();
}

pub fn increment_load_page_fault_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_load_page_fault();
}

pub fn increment_store_page_fault_global() {
    GLOBAL_VM_EXIT_INFO.lock().increment_store_page_fault();
}

/// Print the summary of the global VM exit statistics.
pub fn print_global_summary() -> HyperCallResult {
    GLOBAL_VM_EXIT_INFO.lock().print_summary();
    HyperCallResult::Ok(0)
}

/// Print the summary of the global VM exit statistics.
pub fn clear_global_summary() -> HyperCallResult {
    GLOBAL_VM_EXIT_INFO.lock().clear();
    HyperCallResult::Ok(0)
}