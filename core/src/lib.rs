mod actions;
mod core;
mod qr;
mod state;
mod updates;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use flume::{Receiver, Sender};

pub use actions::AppAction;
pub use qr::*;
pub use state::*;
pub use updates::*;

use crate::core::AppCore;

uniffi::setup_scaffolding!();

#[uniffi::export(callback_interface)]
pub trait AppReconciler: Send + Sync + 'static {
    fn reconcile(&self, update: AppUpdate);
}

#[derive(uniffi::Object)]
pub struct FfiApp {
    core_tx: Sender<CoreMsg>,
    update_rx: Receiver<AppUpdate>,
    listening: AtomicBool,
    shared_state: Arc<RwLock<AppState>>,
}

#[uniffi::export]
impl FfiApp {
    #[uniffi::constructor]
    pub fn new(data_dir: String, _keychain_group: String, _app_version: String) -> Arc<Self> {
        let (update_tx, update_rx) = flume::unbounded();
        let (core_tx, core_rx) = flume::unbounded();
        let shared_state = Arc::new(RwLock::new(AppState::empty()));

        let core_tx_for_thread = core_tx.clone();
        let shared_for_thread = shared_state.clone();
        thread::spawn(move || {
            let mut core = AppCore::new(update_tx, core_tx_for_thread, data_dir, shared_for_thread);
            while let Ok(msg) = core_rx.recv() {
                if !core.handle_message(msg) {
                    break;
                }
            }
        });

        Arc::new(Self {
            core_tx,
            update_rx,
            listening: AtomicBool::new(false),
            shared_state,
        })
    }

    pub fn state(&self) -> AppState {
        match self.shared_state.read() {
            Ok(slot) => slot.clone(),
            Err(poison) => poison.into_inner().clone(),
        }
    }

    pub fn dispatch(&self, action: AppAction) {
        let _ = self.core_tx.send(CoreMsg::Action(action));
    }

    pub fn export_support_bundle_json(&self) -> String {
        let (reply_tx, reply_rx) = flume::bounded(1);
        if self
            .core_tx
            .send(CoreMsg::ExportSupportBundle(reply_tx))
            .is_err()
        {
            return "{}".to_string();
        }
        reply_rx.recv().unwrap_or_else(|_| "{}".to_string())
    }

    pub fn shutdown(&self) {
        let (reply_tx, reply_rx) = flume::bounded(1);
        if self.core_tx.send(CoreMsg::Shutdown(Some(reply_tx))).is_err() {
            return;
        }
        let _ = reply_rx.recv();
    }

    pub fn listen_for_updates(&self, reconciler: Box<dyn AppReconciler>) {
        if self
            .listening
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let update_rx = self.update_rx.clone();
        thread::spawn(move || {
            while let Ok(update) = update_rx.recv() {
                reconciler.reconcile(update);
            }
        });
    }
}

#[cfg(test)]
impl FfiApp {
    pub(crate) fn shutdown_blocking(&self) {
        let (reply_tx, reply_rx) = flume::bounded(1);
        let _ = self.core_tx.send(CoreMsg::Shutdown(Some(reply_tx)));
        let _ = reply_rx.recv_timeout(std::time::Duration::from_secs(5));
    }
}

impl Drop for FfiApp {
    fn drop(&mut self) {
        let _ = self.core_tx.send(CoreMsg::Shutdown(None));
    }
}

#[uniffi::export]
pub fn normalize_peer_input(input: String) -> String {
    crate::core::normalize_peer_input_for_display(&input)
}

#[uniffi::export]
pub fn is_valid_peer_input(input: String) -> bool {
    crate::core::parse_peer_input(&input).is_ok()
}

#[uniffi::export]
pub fn build_summary() -> String {
    crate::core::build_summary()
}

#[uniffi::export]
pub fn relay_set_id() -> String {
    crate::core::relay_set_id().to_string()
}

#[uniffi::export]
pub fn is_trusted_test_build() -> bool {
    crate::core::trusted_test_build_flag()
}
