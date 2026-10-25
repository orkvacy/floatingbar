use serde::Serialize;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BatteryInfo {
    pub has_battery: bool,
    /// None when Windows reports the level as unknown.
    pub percent: Option<u8>,
    pub plugged: bool,
    pub charging: bool,
}

pub fn read() -> BatteryInfo {
    let mut s = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut s) }.is_err() {
        return BatteryInfo { has_battery: false, percent: None, plugged: true, charging: false };
    }
    BatteryInfo {
        has_battery: s.BatteryFlag & 128 == 0,
        percent: (s.BatteryLifePercent <= 100).then_some(s.BatteryLifePercent),
        plugged: s.ACLineStatus == 1,
        charging: s.BatteryFlag & 8 != 0,
    }
}
