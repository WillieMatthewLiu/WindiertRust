#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceAvailability {
    Present,
    Missing,
}

pub fn default_device_path() -> &'static str {
    r"\\.\WdRust"
}
