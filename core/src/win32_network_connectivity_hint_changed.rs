#![cfg(target_os = "windows")]
use std::{ffi::c_void, ptr};
use windows::Win32::{
    Foundation::{BOOLEAN, HANDLE},
    NetworkManagement::IpHelper::{CancelMibChangeNotify2, NotifyNetworkConnectivityHintChange},
    Networking::WinSock::NL_NETWORK_CONNECTIVITY_HINT,
};

#[must_use]
pub struct NetworkConnectivityHintChangedHandle<'a> {
    handle: HANDLE,
    _func: Box<dyn Fn(NL_NETWORK_CONNECTIVITY_HINT) + Sync + 'a>,
}
impl<'a> NetworkConnectivityHintChangedHandle<'a> {
    pub fn register<F>(func: F, initial_notification: bool) -> windows::core::Result<Self>
    where
        F: Fn(NL_NETWORK_CONNECTIVITY_HINT) + Sync + 'a,
    {
        let func = Box::new(func);
        let mut handle = HANDLE::default();
        unsafe {
            NotifyNetworkConnectivityHintChange(
                Some(Self::callback::<F>),
                Some(&*func as *const F as *const c_void),
                BOOLEAN::from(initial_notification),
                ptr::addr_of_mut!(handle),
            )
            .ok()?;
        }
        Ok(Self {
            handle,
            _func: func,
        })
    }
    unsafe extern "system" fn callback<F>(
        caller_context: *const c_void,
        connectivity_hint: NL_NETWORK_CONNECTIVITY_HINT,
    ) where
        F: Fn(NL_NETWORK_CONNECTIVITY_HINT) + Sync,
    {
        let caller_context: &F = &*(caller_context as *const F);
        caller_context(connectivity_hint);
    }
}

impl<'a> Drop for NetworkConnectivityHintChangedHandle<'a> {
    fn drop(&mut self) {
        unsafe {
            let _ = CancelMibChangeNotify2(self.handle);
        }
    }
}
