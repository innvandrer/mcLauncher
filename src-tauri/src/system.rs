//! Lightweight host-system probes. Currently just total physical memory, used
//! to suggest a sensible default heap size. Implemented with a tiny inline FFI
//! to avoid pulling in a system-info crate.

/// Total physical RAM in megabytes. Returns 0 if it can't be determined.
#[cfg(windows)]
pub fn total_memory_mb() -> u64 {
    #[repr(C)]
    struct MemoryStatusEx {
        dw_length: u32,
        dw_memory_load: u32,
        ull_total_phys: u64,
        ull_avail_phys: u64,
        ull_total_page_file: u64,
        ull_avail_page_file: u64,
        ull_total_virtual: u64,
        ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }

    let mut status = MemoryStatusEx {
        dw_length: std::mem::size_of::<MemoryStatusEx>() as u32,
        dw_memory_load: 0,
        ull_total_phys: 0,
        ull_avail_phys: 0,
        ull_total_page_file: 0,
        ull_avail_page_file: 0,
        ull_total_virtual: 0,
        ull_avail_virtual: 0,
        ull_avail_extended_virtual: 0,
    };

    unsafe {
        if GlobalMemoryStatusEx(&mut status) != 0 {
            return status.ull_total_phys / 1024 / 1024;
        }
    }
    0
}

#[cfg(not(windows))]
pub fn total_memory_mb() -> u64 {
    0
}
