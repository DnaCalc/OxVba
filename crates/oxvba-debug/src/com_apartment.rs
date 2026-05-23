use crate::config::DebugComApartment;

/// Worker COM apartment guard placeholder.
#[derive(Debug)]
pub struct DebugComApartmentGuard {
    apartment: DebugComApartment,
}

impl DebugComApartmentGuard {
    pub fn initialize(apartment: DebugComApartment) -> Result<Self, String> {
        Ok(Self { apartment })
    }

    pub fn apartment(&self) -> DebugComApartment {
        self.apartment
    }
}
