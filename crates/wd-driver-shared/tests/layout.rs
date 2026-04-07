use wd_driver_shared::{DEVICE_NAME, DOS_DEVICE_NAME, IOCTL_OPEN, IOCTL_RECV, IOCTL_SEND};

#[test]
fn device_names_and_ioctl_values_are_stable() {
    assert_eq!(DEVICE_NAME, r"\\Device\\WdRust");
    assert_eq!(DOS_DEVICE_NAME, r"\\DosDevices\\WdRust");
    assert_eq!(IOCTL_OPEN, 0x8000_2000);
    assert_eq!(IOCTL_RECV, 0x8000_2004);
    assert_eq!(IOCTL_SEND, 0x8000_2008);
}
