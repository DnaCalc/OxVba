use crate::config::DebugComApartment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugWorkerApartmentKind {
    Sta,
    Mta,
    None,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugWorkerApartmentReport {
    pub configured: DebugComApartment,
    pub observed: DebugWorkerApartmentKind,
    pub initialized_by_worker: bool,
}

#[derive(Debug)]
pub struct DebugComApartmentGuard {
    apartment: DebugComApartment,
    initialized: bool,
}

impl DebugComApartmentGuard {
    pub fn initialize(apartment: DebugComApartment) -> Result<Self, String> {
        initialize_platform(apartment).map(|initialized| Self {
            apartment,
            initialized,
        })
    }

    pub fn apartment(&self) -> DebugComApartment {
        self.apartment
    }

    pub fn report(&self) -> DebugWorkerApartmentReport {
        DebugWorkerApartmentReport {
            configured: self.apartment,
            observed: observed_platform_apartment(),
            initialized_by_worker: self.initialized,
        }
    }
}

impl Drop for DebugComApartmentGuard {
    fn drop(&mut self) {
        uninitialize_platform(self.initialized);
    }
}

#[cfg(target_os = "windows")]
fn initialize_platform(apartment: DebugComApartment) -> Result<bool, String> {
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoInitializeEx,
    };
    let coinit = match apartment {
        DebugComApartment::None => return Ok(false),
        DebugComApartment::Sta => COINIT_APARTMENTTHREADED,
        DebugComApartment::Mta => COINIT_MULTITHREADED,
    };
    let hr = unsafe { CoInitializeEx(std::ptr::null_mut(), coinit as u32) };
    if hr < 0 {
        Err(format!("CoInitializeEx failed: 0x{hr:08x}"))
    } else {
        Ok(true)
    }
}

#[cfg(not(target_os = "windows"))]
fn initialize_platform(apartment: DebugComApartment) -> Result<bool, String> {
    match apartment {
        DebugComApartment::None | DebugComApartment::Sta | DebugComApartment::Mta => Ok(false),
    }
}

#[cfg(target_os = "windows")]
fn uninitialize_platform(initialized: bool) {
    if initialized {
        unsafe { windows_sys::Win32::System::Com::CoUninitialize() };
    }
}

#[cfg(not(target_os = "windows"))]
fn uninitialize_platform(_initialized: bool) {}

#[cfg(target_os = "windows")]
fn observed_platform_apartment() -> DebugWorkerApartmentKind {
    use windows_sys::Win32::System::Com::{
        APTTYPE, APTTYPE_MAINSTA, APTTYPE_MTA, APTTYPE_STA, APTTYPEQUALIFIER, CoGetApartmentType,
    };
    let mut apt_type: APTTYPE = 0;
    let mut qualifier: APTTYPEQUALIFIER = 0;
    let hr = unsafe { CoGetApartmentType(&mut apt_type, &mut qualifier) };
    if hr < 0 {
        return DebugWorkerApartmentKind::None;
    }
    match apt_type {
        APTTYPE_STA | APTTYPE_MAINSTA => DebugWorkerApartmentKind::Sta,
        APTTYPE_MTA => DebugWorkerApartmentKind::Mta,
        _ => DebugWorkerApartmentKind::Unknown,
    }
}

#[cfg(not(target_os = "windows"))]
fn observed_platform_apartment() -> DebugWorkerApartmentKind {
    DebugWorkerApartmentKind::None
}
