use std::{ffi::c_void, ptr};
use windows::Win32::{
    Foundation::{BOOLEAN, HANDLE},
    NetworkManagement::IpHelper::{CancelMibChangeNotify2, NotifyNetworkConnectivityHintChange},
    Networking::WinSock::NL_NETWORK_CONNECTIVITY_HINT,
};

#[must_use]
pub struct NetworkConnectivityHintChangedHandle<'a, F>
where
    F: Fn(NL_NETWORK_CONNECTIVITY_HINT) + Sync,
{
    _func: &'a F,
    handle: HANDLE,
}
impl<'a, F> NetworkConnectivityHintChangedHandle<'a, F>
where
    F: Fn(NL_NETWORK_CONNECTIVITY_HINT) + Sync,
{
    pub fn register(func: &'a F, initial_notification: bool) -> windows::core::Result<Self> {
        let mut handle = HANDLE::default();
        unsafe {
            NotifyNetworkConnectivityHintChange(
                Some(Self::on_network_connectivity_hint_changed),
                Some(func as *const F as *const c_void),
                BOOLEAN::from(initial_notification),
                ptr::addr_of_mut!(handle),
            )
            .ok()?;
        }
        Ok(Self {
            _func: func,
            handle,
        })
    }
    unsafe extern "system" fn on_network_connectivity_hint_changed(
        caller_context: *const c_void,
        connectivity_hint: NL_NETWORK_CONNECTIVITY_HINT,
    ) {
        let caller_context: &F = &*(caller_context as *const F);
        caller_context(connectivity_hint);
    }
}

impl<'a, F> Drop for NetworkConnectivityHintChangedHandle<'a, F>
where
    F: Fn(NL_NETWORK_CONNECTIVITY_HINT) + Sync,
{
    fn drop(&mut self) {
        unsafe {
            let _ = CancelMibChangeNotify2(self.handle);
        }
    }
}
