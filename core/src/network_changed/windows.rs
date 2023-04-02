use std::{ffi::c_void, ptr};

use anyhow::Result;
use tokio::sync::mpsc::{self, error::TrySendError};
use tracing::trace;
use windows::Win32::{
    Foundation::HANDLE,
    NetworkManagement::IpHelper::{CancelMibChangeNotify2, NotifyNetworkConnectivityHintChange},
    Networking::WinSock::{
        NetworkConnectivityLevelHintConstrainedInternetAccess,
        NetworkConnectivityLevelHintLocalAccess, NL_NETWORK_CONNECTIVITY_HINT,
    },
};

#[must_use]
pub struct NetworkChangedListener {
    handle: HANDLE,
    _tx: Box<mpsc::Sender<()>>,
}

impl NetworkChangedListener {
    pub fn listen() -> Result<(Self, mpsc::Receiver<()>)> {
        let mut handle = HANDLE::default();
        let (tx, rx) = mpsc::channel(1);
        let _ = tx.try_send(());
        let tx = Box::into_raw(Box::new(tx));
        let tx = unsafe {
            NotifyNetworkConnectivityHintChange(
                Some(Self::callback),
                Some(tx as *const c_void),
                true,
                ptr::addr_of_mut!(handle),
            )?;
            Box::from_raw(tx)
        };
        Ok((Self { handle, _tx: tx }, rx))
    }

    unsafe extern "system" fn callback(
        callercontext: *const c_void,
        connectivityhint: NL_NETWORK_CONNECTIVITY_HINT,
    ) {
        let connectivity_level = connectivityhint.ConnectivityLevel;
        trace!(connectivity_level = connectivity_level.0);

        #[allow(non_upper_case_globals)]
        if let NetworkConnectivityLevelHintConstrainedInternetAccess
        | NetworkConnectivityLevelHintLocalAccess = connectivity_level
        {
            let tx: &mpsc::Sender<()> = &*(callercontext as *const mpsc::Sender<()>);
            if let Err(TrySendError::Closed(_)) = tx.try_send(()) {
                // rx is dropped
                return;
            }
        }
    }
}

impl Drop for NetworkChangedListener {
    fn drop(&mut self) {
        unsafe {
            let _ = CancelMibChangeNotify2(self.handle);
        }
    }
}
