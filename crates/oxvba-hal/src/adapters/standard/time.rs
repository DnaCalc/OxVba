use crate::{error::HalResult, model::CapabilityId, traits::TimeLocaleHal};
use oxvba_runtime::Variant;
use oxvba_runtime::ymd_to_serial;

use super::StandardHostServices;

const DETERMINISTIC_DATE_SERIAL: f64 = 46_082.0;
const DETERMINISTIC_SECONDS: f64 = 45_296.0;
const VBA_EPOCH_OFFSET_DAYS: f64 = 25_569.0;

fn deterministic_date_variant() -> Variant {
    Variant::from_date_f64(DETERMINISTIC_DATE_SERIAL)
}

fn deterministic_time_variant() -> Variant {
    Variant::from_date_f64(DETERMINISTIC_SECONDS / 86_400.0)
}

fn deterministic_timer_variant() -> Variant {
    Variant::from_f32(DETERMINISTIC_SECONDS as f32)
}

#[derive(Debug, Clone, Copy)]
struct ClockComponents {
    date_serial: f64,
    seconds_today: f64,
}

fn utc_system_time_to_vba_serial(time: std::time::SystemTime) -> f64 {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64() / 86_400.0 + VBA_EPOCH_OFFSET_DAYS,
        Err(e) => VBA_EPOCH_OFFSET_DAYS - e.duration().as_secs_f64() / 86_400.0,
    }
}

fn clock_components_from_vba_serial(serial: f64) -> ClockComponents {
    let date_serial = serial.floor();
    ClockComponents {
        date_serial,
        seconds_today: (serial - date_serial) * 86_400.0,
    }
}

#[cfg(any(test, not(target_os = "windows")))]
fn utc_clock_components() -> ClockComponents {
    clock_components_from_vba_serial(utc_system_time_to_vba_serial(std::time::SystemTime::now()))
}

#[cfg(target_os = "windows")]
pub(super) fn system_time_to_local_vba_serial(time: std::time::SystemTime) -> f64 {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::Storage::FileSystem::FileTimeToLocalFileTime;
    use windows_sys::Win32::System::Time::FileTimeToSystemTime;

    const WINDOWS_TICKS_FROM_UNIX_EPOCH: i128 = 11_644_473_600_i128 * 10_000_000;

    let unix_relative_ticks = match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => {
            duration.as_secs() as i128 * 10_000_000 + i128::from(duration.subsec_nanos() / 100)
        }
        Err(err) => {
            let duration = err.duration();
            -(duration.as_secs() as i128 * 10_000_000 + i128::from(duration.subsec_nanos() / 100))
        }
    };
    let ticks = WINDOWS_TICKS_FROM_UNIX_EPOCH + unix_relative_ticks;
    let Ok(ticks) = u64::try_from(ticks) else {
        return utc_system_time_to_vba_serial(time);
    };
    let utc_filetime = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut local_filetime = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let local_ok = unsafe { FileTimeToLocalFileTime(&utc_filetime, &mut local_filetime) };
    if local_ok == 0 {
        return utc_system_time_to_vba_serial(time);
    }
    let mut local = SYSTEMTIME {
        wYear: 0,
        wMonth: 0,
        wDayOfWeek: 0,
        wDay: 0,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };
    let system_ok = unsafe { FileTimeToSystemTime(&local_filetime, &mut local) };
    if system_ok == 0 {
        return utc_system_time_to_vba_serial(time);
    }
    ymd_to_serial(
        i64::from(local.wYear),
        i64::from(local.wMonth),
        i64::from(local.wDay),
    ) + (f64::from(local.wHour) * 3_600.0
        + f64::from(local.wMinute) * 60.0
        + f64::from(local.wSecond)
        + f64::from(local.wMilliseconds) / 1_000.0)
        / 86_400.0
}

#[cfg(unix)]
pub(super) fn system_time_to_local_vba_serial(time: std::time::SystemTime) -> f64 {
    let Ok(duration) = time.duration_since(std::time::UNIX_EPOCH) else {
        return utc_system_time_to_vba_serial(time);
    };
    let seconds = duration.as_secs() as libc::time_t;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    let local = unsafe {
        if libc::localtime_r(&seconds, local.as_mut_ptr()).is_null() {
            return utc_system_time_to_vba_serial(time);
        }
        local.assume_init()
    };
    ymd_to_serial(
        i64::from(local.tm_year + 1900),
        i64::from(local.tm_mon + 1),
        i64::from(local.tm_mday),
    ) + (f64::from(local.tm_hour) * 3_600.0
        + f64::from(local.tm_min) * 60.0
        + f64::from(local.tm_sec)
        + f64::from(duration.subsec_nanos()) / 1_000_000_000.0)
        / 86_400.0
}

#[cfg(not(any(target_os = "windows", unix)))]
pub(super) fn system_time_to_local_vba_serial(time: std::time::SystemTime) -> f64 {
    utc_system_time_to_vba_serial(time)
}

fn local_clock_components() -> ClockComponents {
    clock_components_from_vba_serial(system_time_to_local_vba_serial(std::time::SystemTime::now()))
}

impl TimeLocaleHal for StandardHostServices {
    fn date_serial_now_variant(&self) -> HalResult<Variant> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "date_serial_now"));
        }
        if self.native_time_enabled() {
            return Ok(Variant::from_date_f64(
                local_clock_components().date_serial.floor(),
            ));
        }
        Ok(deterministic_date_variant())
    }

    fn time_serial_now_variant(&self) -> HalResult<Variant> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "time_serial_now"));
        }
        if self.native_time_enabled() {
            return Ok(Variant::from_date_f64(
                local_clock_components().seconds_today / 86_400.0,
            ));
        }
        Ok(deterministic_time_variant())
    }

    fn timer_ticks_variant(&self) -> HalResult<Variant> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "timer_ticks"));
        }
        if self.native_time_enabled() {
            return Ok(Variant::from_f32(
                local_clock_components().seconds_today as f32,
            ));
        }
        Ok(deterministic_timer_variant())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HalProfileId, HostPolicy};

    fn native_profile() -> HalProfileId {
        if cfg!(target_os = "windows") {
            HalProfileId::Windows
        } else if cfg!(target_os = "linux") {
            HalProfileId::Linux
        } else {
            HalProfileId::Null
        }
    }

    #[test]
    fn deterministic_time_values_stay_stable() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.date_serial_now_variant().expect("date"),
            deterministic_date_variant()
        );
        assert_eq!(
            host.time_serial_now_variant().expect("time"),
            deterministic_time_variant()
        );
        assert_eq!(
            host.timer_ticks_variant().expect("timer"),
            deterministic_timer_variant()
        );
    }

    fn seconds_apart_mod_day(lhs: f64, rhs: f64) -> f64 {
        let delta = (lhs - rhs).abs();
        delta.min(86_400.0 - delta)
    }

    #[test]
    fn native_time_uses_local_clock_components() {
        let host = StandardHostServices::new(native_profile(), HostPolicy::interactive_dev());
        if !host.native_time_enabled() {
            return;
        }

        let local_before = local_clock_components();
        let utc = utc_clock_components();
        let observed_date = host
            .date_serial_now_variant()
            .expect("date")
            .as_date_f64()
            .expect("date serial");
        let observed_time = host
            .time_serial_now_variant()
            .expect("time")
            .as_date_f64()
            .expect("time serial")
            * 86_400.0;
        let observed_timer = host
            .timer_ticks_variant()
            .expect("timer")
            .as_f32()
            .expect("timer") as f64;
        let local_after = local_clock_components();

        assert!(
            observed_date == local_before.date_serial.floor()
                || observed_date == local_after.date_serial.floor()
        );
        assert!(
            seconds_apart_mod_day(observed_time, local_before.seconds_today) < 2.0
                || seconds_apart_mod_day(observed_time, local_after.seconds_today) < 2.0
        );
        assert!(
            seconds_apart_mod_day(observed_timer, local_before.seconds_today) < 2.0
                || seconds_apart_mod_day(observed_timer, local_after.seconds_today) < 2.0
        );

        let offset_seconds = ((local_after.date_serial - utc.date_serial) * 86_400.0
            + local_after.seconds_today
            - utc.seconds_today)
            .abs();
        if offset_seconds > 60.0 {
            let observed_serial = observed_date + observed_time / 86_400.0;
            let utc_serial = utc.date_serial.floor() + utc.seconds_today / 86_400.0;
            assert!(
                ((observed_serial - utc_serial) * 86_400.0).abs() > 60.0,
                "non-UTC local clock must not report UTC wall-clock serial"
            );
        }
    }
}
