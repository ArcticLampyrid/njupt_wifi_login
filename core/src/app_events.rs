use tokio::task::AbortHandle;

pub trait AppEvents {
    fn on_started(&self);
    fn on_stopping(&self);
    fn on_stopped(&self);
    fn register_abort_handle(&mut self, handle: AbortHandle);
}

pub struct DefaultAppEvents;
impl AppEvents for DefaultAppEvents {
    fn on_started(&self) {
        // Do nothing.
    }
    fn on_stopping(&self) {
        // Do nothing.
    }
    fn on_stopped(&self) {
        // Do nothing.
    }
    fn register_abort_handle(&mut self, _handle: AbortHandle) {
        // Do nothing.
    }
}
