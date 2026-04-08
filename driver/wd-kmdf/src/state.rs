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

        *self = {
            Self::Closing
        };

        if matches!(self, Self::Closing) {
            *self = Self::Closed;
        }
        Ok(())
    }

    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}
