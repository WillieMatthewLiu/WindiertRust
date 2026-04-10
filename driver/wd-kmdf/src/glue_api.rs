use wd_proto::Layer;
use wd_kmdf_core::{GlueIoResult, GlueIoStatus};

use crate::{RuntimeIoctlDispatcher, RuntimeIoctlError};

#[derive(Debug, Clone)]
pub struct RuntimeGlueApi {
    dispatcher: RuntimeIoctlDispatcher,
}

impl RuntimeGlueApi {
    pub fn new(queue_capacity: usize) -> Self {
        Self {
            dispatcher: RuntimeIoctlDispatcher::new(queue_capacity),
        }
    }

    pub fn queue_network_event(
        &mut self,
        layer: Layer,
        packet_id: u64,
        packet: &[u8],
    ) -> Result<(), RuntimeIoctlError> {
        self.dispatcher.queue_network_event(layer, packet_id, packet)
    }

    pub fn device_control(&mut self, ioctl: u32, input: &[u8], output: &mut [u8]) -> GlueIoResult {
        match self.dispatcher.dispatch_into(ioctl, input, output) {
            Ok(bytes_written) => GlueIoResult {
                status: GlueIoStatus::Success,
                bytes_written: bytes_written as u32,
            },
            Err(err) => GlueIoResult {
                status: map_ioctl_error(err),
                bytes_written: 0,
            },
        }
    }

    pub unsafe fn device_control_raw(
        &mut self,
        ioctl: u32,
        input_ptr: *const u8,
        input_len: usize,
        output_ptr: *mut u8,
        output_len: usize,
    ) -> GlueIoResult {
        if input_len > 0 && input_ptr.is_null() {
            return GlueIoResult {
                status: GlueIoStatus::InvalidPointer,
                bytes_written: 0,
            };
        }
        if output_len > 0 && output_ptr.is_null() {
            return GlueIoResult {
                status: GlueIoStatus::InvalidPointer,
                bytes_written: 0,
            };
        }

        let input = if input_len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(input_ptr, input_len) }
        };
        let output = if output_len == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(output_ptr, output_len) }
        };

        self.device_control(ioctl, input, output)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wd_runtime_glue_create(queue_capacity: usize) -> *mut RuntimeGlueApi {
    Box::into_raw(Box::new(RuntimeGlueApi::new(queue_capacity)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wd_runtime_glue_destroy(handle: *mut RuntimeGlueApi) {
    if handle.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wd_runtime_glue_device_control(
    handle: *mut RuntimeGlueApi,
    ioctl: u32,
    input_ptr: *const u8,
    input_len: usize,
    output_ptr: *mut u8,
    output_len: usize,
) -> GlueIoResult {
    let Some(glue) = (unsafe { handle.as_mut() }) else {
        return GlueIoResult {
            status: GlueIoStatus::InvalidHandle,
            bytes_written: 0,
        };
    };

    unsafe { glue.device_control_raw(ioctl, input_ptr, input_len, output_ptr, output_len) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wd_runtime_glue_queue_network_event(
    handle: *mut RuntimeGlueApi,
    layer_wire: u8,
    packet_id: u64,
    packet_ptr: *const u8,
    packet_len: usize,
) -> GlueIoResult {
    let Some(glue) = (unsafe { handle.as_mut() }) else {
        return GlueIoResult {
            status: GlueIoStatus::InvalidHandle,
            bytes_written: 0,
        };
    };
    if packet_len > 0 && packet_ptr.is_null() {
        return GlueIoResult {
            status: GlueIoStatus::InvalidPointer,
            bytes_written: 0,
        };
    }
    let Some(layer) = Layer::from_wire(layer_wire) else {
        return GlueIoResult {
            status: GlueIoStatus::InvalidLayer,
            bytes_written: 0,
        };
    };
    let packet = if packet_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(packet_ptr, packet_len) }
    };

    match glue.queue_network_event(layer, packet_id, packet) {
        Ok(()) => GlueIoResult {
            status: GlueIoStatus::Success,
            bytes_written: 0,
        },
        Err(err) => GlueIoResult {
            status: map_ioctl_error(err),
            bytes_written: 0,
        },
    }
}

fn map_ioctl_error(err: RuntimeIoctlError) -> GlueIoStatus {
    match err {
        RuntimeIoctlError::UnsupportedIoctl(_) => GlueIoStatus::UnsupportedIoctl,
        RuntimeIoctlError::DecodeOpen(_) => GlueIoStatus::DecodeOpen,
        RuntimeIoctlError::OutputTooSmall { .. } => GlueIoStatus::OutputTooSmall,
        RuntimeIoctlError::Device(device_err) => match device_err {
            crate::RuntimeDeviceError::QueueEmpty => GlueIoStatus::QueueEmpty,
            crate::RuntimeDeviceError::RecvDisabled => GlueIoStatus::RecvDisabled,
            crate::RuntimeDeviceError::SendDisabled => GlueIoStatus::SendDisabled,
            crate::RuntimeDeviceError::InvalidState(_) => GlueIoStatus::InvalidState,
            crate::RuntimeDeviceError::FilterCompile(_) => GlueIoStatus::DecodeOpen,
            crate::RuntimeDeviceError::EncodeInto(_) => GlueIoStatus::NetworkRuntime,
            crate::RuntimeDeviceError::QueueStorage(_) => GlueIoStatus::NetworkRuntime,
            crate::RuntimeDeviceError::NetworkRuntime(_) => GlueIoStatus::NetworkRuntime,
        },
    }
}
