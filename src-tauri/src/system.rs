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

#[cfg(target_os = "linux")]
pub fn total_memory_mb() -> u64 {
    // `MemTotal:       16337608 kB` in /proc/meminfo.
    let Ok(content) = std::fs::read_to_string("/proc/meminfo") else {
        return 0;
    };
    parse_meminfo_total_mb(&content)
}

#[cfg(any(target_os = "linux", test))]
fn parse_meminfo_total_mb(meminfo: &str) -> u64 {
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u64 = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            return kb / 1024;
        }
    }
    0
}

#[cfg(target_os = "macos")]
pub fn total_memory_mb() -> u64 {
    // sysctl hw.memsize — total physical RAM in bytes.
    extern "C" {
        fn sysctlbyname(
            name: *const std::os::raw::c_char,
            oldp: *mut std::os::raw::c_void,
            oldlenp: *mut usize,
            newp: *const std::os::raw::c_void,
            newlen: usize,
        ) -> std::os::raw::c_int;
    }
    let mut size: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    let ok = unsafe {
        sysctlbyname(
            c"hw.memsize".as_ptr(),
            &mut size as *mut u64 as *mut _,
            &mut len,
            std::ptr::null(),
            0,
        )
    };
    if ok == 0 {
        size / 1024 / 1024
    } else {
        0
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub fn total_memory_mb() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo_parsing() {
        let sample = "MemTotal:       16337608 kB\nMemFree:         1078348 kB\n";
        assert_eq!(parse_meminfo_total_mb(sample), 15954);
        assert_eq!(parse_meminfo_total_mb("garbage"), 0);
        assert_eq!(parse_meminfo_total_mb("MemTotal: notanumber kB"), 0);
    }
}
