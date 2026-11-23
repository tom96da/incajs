// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The development protocol: reads newline-delimited JSON-RPC on stdin and
//! answers on stdout. A `reload` evaluates the new entry as a pending load
//! that swaps into the window when it settles. Any other notification goes
//! to the loading app while a load is pending, and to the window's app
//! otherwise, as `__inca_dev__.receive(method, paramsJson)`; see
//! [`wire`] for the wire format this speaks. With the `automation` feature, `click`, `snapshot`
//! and `waitIdle` run in the host and are not relayed.
//!
//! The first entry starts as a [`crate::app::Loading`] and the window opens once its
//! top-level `await` settles. An entry still pending after [`serve::LOAD_TIMEOUT`]
//! opens the window at the size the config gives, else the mounted content's
//! size, else the default size, unless the app reported a failed load.

#[cfg(any(test, feature = "automation"))]
mod automation;
mod globals;
mod reload;
mod serve;
mod startup;
mod stdio;
mod wire;

pub(crate) use globals::{DevSend, LoadSignal, call_dev_receive, install_dev, install_load_signal};
pub(crate) use serve::serve_dev_protocol;
pub(crate) use startup::fail_startup;
#[cfg(test)]
pub(crate) use stdio::Writer;
pub(crate) use stdio::{
    SharedWriter, StdoutWriter, dev_send, install_panic_hook, reporter_for, send, stdin_lines,
};
pub(crate) use wire::ErrorCode;
