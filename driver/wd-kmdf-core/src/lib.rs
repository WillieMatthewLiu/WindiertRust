#![no_std]

use core::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleState {
    Opening,
    Running,
    RecvShutdown,
    SendShutdown,
    Closing,
    Closed,
}

impl HandleState {
    pub const fn opening() -> Self {
        Self::Opening
    }

    pub fn mark_running(&mut self) -> Result<(), &'static str> {
        if matches!(self, Self::Opening) {
            *self = Self::Running;
            Ok(())
        } else {
            Err("mark_running requires Opening state")
        }
    }

    pub fn shutdown_recv(&mut self) -> Result<(), &'static str> {
        if matches!(self, Self::Running) {
            *self = Self::RecvShutdown;
            Ok(())
        } else {
            Err("shutdown_recv requires Running state")
        }
    }

    pub fn shutdown_send(&mut self) -> Result<(), &'static str> {
        if matches!(self, Self::RecvShutdown) {
            *self = Self::SendShutdown;
            Ok(())
        } else {
            Err("shutdown_send requires RecvShutdown state")
        }
    }

    pub fn close(&mut self) -> Result<(), &'static str> {
        if !matches!(self, Self::SendShutdown) {
            return Err("close requires SendShutdown state");
        }

        *self = Self::Closing;
        *self = Self::Closed;
        Ok(())
    }

    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlueIoStatus {
    Success = 0,
    UnsupportedIoctl = 1,
    DecodeOpen = 2,
    OutputTooSmall = 3,
    QueueEmpty = 4,
    RecvDisabled = 5,
    SendDisabled = 6,
    InvalidState = 7,
    NetworkRuntime = 8,
    InvalidPointer = 9,
    InvalidHandle = 10,
    InvalidLayer = 11,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlueIoResult {
    pub status: GlueIoStatus,
    pub bytes_written: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReinjectionToken(u64);

impl ReinjectionToken {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReinjectionError {
    UnknownToken,
}

impl Display for ReinjectionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownToken => write!(f, "unknown reinjection token"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FixedReinjectionTable<const N: usize> {
    next_token: u64,
    write_index: usize,
    slots: [Option<(ReinjectionToken, u64)>; N],
}

impl<const N: usize> FixedReinjectionTable<N> {
    pub const fn new() -> Self {
        Self {
            next_token: 1,
            write_index: 0,
            slots: [None; N],
        }
    }

    pub fn issue_for_network_packet(&mut self, packet_id: u64) -> ReinjectionToken {
        let token = ReinjectionToken::new(self.next_token);
        self.next_token = self.next_token.saturating_add(1);

        if N != 0 {
            self.slots[self.write_index] = Some((token, packet_id));
            self.write_index = (self.write_index + 1) % N;
        }

        token
    }

    pub fn consume(&mut self, token: ReinjectionToken) -> Result<u64, ReinjectionError> {
        for slot in &mut self.slots {
            if let Some((stored, packet_id)) = slot {
                if *stored == token {
                    let packet_id = *packet_id;
                    *slot = None;
                    return Ok(packet_id);
                }
            }
        }

        Err(ReinjectionError::UnknownToken)
    }

    pub fn consume_raw(&mut self, token: u64) -> Result<u64, ReinjectionError> {
        self.consume(ReinjectionToken::new(token))
    }
}

impl<const N: usize> Default for FixedReinjectionTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteRingError {
    FrameTooLarge,
    OutputTooSmall { required: usize, provided: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedPacketError {
    PacketTooLarge { required: usize, capacity: usize },
}

impl Display for FixedPacketError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PacketTooLarge { required, capacity } => {
                write!(
                    f,
                    "packet exceeds fixed storage: required {required} bytes but capacity is {capacity}"
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPacket<const BYTES: usize> {
    storage: [u8; BYTES],
    len: usize,
}

impl<const BYTES: usize> FixedPacket<BYTES> {
    pub const fn new() -> Self {
        Self {
            storage: [0; BYTES],
            len: 0,
        }
    }

    pub fn copy_from_slice(input: &[u8]) -> Result<Self, FixedPacketError> {
        if input.len() > BYTES {
            return Err(FixedPacketError::PacketTooLarge {
                required: input.len(),
                capacity: BYTES,
            });
        }

        let mut packet = Self::new();
        packet.storage[..input.len()].copy_from_slice(input);
        packet.len = input.len();
        Ok(packet)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.storage[..self.len]
    }

    pub const fn len(&self) -> usize {
        self.len
    }
}

impl<const BYTES: usize> Default for FixedPacket<BYTES> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ByteRing<const SLOTS: usize, const BYTES: usize> {
    storage: [[u8; BYTES]; SLOTS],
    lengths: [usize; SLOTS],
    head: usize,
    len: usize,
}

impl<const SLOTS: usize, const BYTES: usize> ByteRing<SLOTS, BYTES> {
    pub const fn new() -> Self {
        Self {
            storage: [[0; BYTES]; SLOTS],
            lengths: [0; SLOTS],
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, frame: &[u8]) -> Result<(), ByteRingError> {
        if frame.len() > BYTES {
            return Err(ByteRingError::FrameTooLarge);
        }
        if SLOTS == 0 {
            return Ok(());
        }

        let index = if self.len < SLOTS {
            (self.head + self.len) % SLOTS
        } else {
            let current = self.head;
            self.head = (self.head + 1) % SLOTS;
            current
        };

        self.storage[index][..frame.len()].copy_from_slice(frame);
        self.lengths[index] = frame.len();
        if self.len < SLOTS {
            self.len += 1;
        }
        Ok(())
    }

    pub fn pop_into(&mut self, output: &mut [u8]) -> Result<Option<usize>, ByteRingError> {
        if self.len == 0 || SLOTS == 0 {
            return Ok(None);
        }

        let index = self.head;
        let frame_len = self.lengths[index];
        if frame_len > output.len() {
            return Err(ByteRingError::OutputTooSmall {
                required: frame_len,
                provided: output.len(),
            });
        }

        output[..frame_len].copy_from_slice(&self.storage[index][..frame_len]);
        self.lengths[index] = 0;
        self.head = (self.head + 1) % SLOTS;
        self.len -= 1;
        Ok(Some(frame_len))
    }

    pub fn drop_oldest(&mut self) -> bool {
        if self.len == 0 || SLOTS == 0 {
            return false;
        }

        let index = self.head;
        self.lengths[index] = 0;
        self.head = (self.head + 1) % SLOTS;
        self.len -= 1;
        true
    }

    pub fn clear(&mut self) {
        while self.drop_oldest() {}
    }

    pub const fn len(&self) -> usize {
        self.len
    }
}

impl<const SLOTS: usize, const BYTES: usize> Default for ByteRing<SLOTS, BYTES> {
    fn default() -> Self {
        Self::new()
    }
}
