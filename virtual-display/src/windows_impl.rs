use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use std::{
    borrow::Borrow,
    collections::HashSet,
    fmt::Display,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use driver_ipc::{Dimen, DriverClient, EventCommand, Id, Mode, Monitor, RefreshRate};

pub struct VirtualDisplayState {}

pub struct VirtualDisplay {
    // state: Arc<Mutex<VirtualDisplayState>>,
    client: Arc<Mutex<DriverClient>>,
}

impl VirtualDisplay {
    pub async fn new(display_name: &str) -> Result<Self> {
        let mut client = DriverClient::new().await?;

        let monitors = [Monitor {
            id: 0,
            name: Some(display_name.to_owned()),
            enabled: true,
            modes: vec![Mode {
                height: 500,
                width: 500,
                refresh_rates: vec![60],
            }],
        }];

        client.set_monitors(&monitors)?;
        client.persist()?;
        client.notify().await?;
        let client = Arc::new(Mutex::new(client));
        Ok(Self { client })
    }
}
