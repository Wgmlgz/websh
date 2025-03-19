use std::fmt::Debug;
use std::sync::Arc;
use std::{
    borrow::Borrow,
    collections::HashSet,
    fmt::Display,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use anyhow::{anyhow, Context};
use driver_ipc::{Dimen, DriverClient, EventCommand, Id, Mode, Monitor, RefreshRate};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
#[derive(Debug, Clone)]
pub struct VirtualDisplayManager {
    client: Arc<Mutex<DriverClient>>,
    managed_displays: Arc<Mutex<Vec<Id>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisplayCommand {
    Create {
        name: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
        refresh_rate: Option<u32>,
    },
    Update {
        id: Id,
        width: Option<u32>,
        height: Option<u32>,
        refresh_rate: Option<u32>,
    },
    Disable {
        id: Id,
    },
    Enable {
        id: Id,
    },
    Remove {
        id: Id,
    },
    List,
}

impl VirtualDisplayManager {
    pub async fn new() -> Result<Self> {
        let mut client = DriverClient::new().await?;

        // Initialize with existing monitors
        let existing_ids = client.monitors().into_iter().map(|m| m.id).collect();

        dbg!(&existing_ids);
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            managed_displays: Arc::new(Mutex::new(existing_ids)),
        })
    }

    pub async fn handle_command(&self, command: DisplayCommand) -> Result<()> {
        match command {
            DisplayCommand::Create {
                name,
                width,
                height,
                refresh_rate,
            } => {
                let width = width.unwrap_or(1920);
                let height = height.unwrap_or(1080);
                let refresh_rate = refresh_rate.unwrap_or(60);
                self.create_display(name, width, height, refresh_rate)
                    .await?;
                Ok(())
            }
            DisplayCommand::Update {
                id,
                width,
                height,
                refresh_rate,
            } => self.update_display(id, width, height, refresh_rate).await,
            DisplayCommand::Disable { id } => self.set_display_enabled(id, false).await,
            DisplayCommand::Enable { id } => self.set_display_enabled(id, true).await,
            DisplayCommand::Remove { id } => self.remove_display(id).await,
            DisplayCommand::List => self.list_displays().await,
        }
    }

    pub async fn set_display_enabled(&self, id: Id, enabled: bool) -> Result<()> {
        let mut client = self.client.lock().await;
        client.set_enabled_query(&[id.to_string().as_str()], enabled)?;
        client.persist()?;
        client.notify().await?;
        Ok(())
    }

    pub async fn remove_display(&self, id: Id) -> Result<()> {
        let mut client = self.client.lock().await;
        let mut managed = self.managed_displays.lock().await;

        client.remove_query(&[id.to_string()])?;
        client.persist()?;
        client.notify().await?;

        managed.retain(|&i| i != id);
        Ok(())
    }

    // pub fn get_active_displays(&self) -> Result<Vec<Monitor>> {
    //     let client = self.client.lock().await;
    //     Ok(client
    //         .monitors()
    //         .into_iter()
    //         .filter(|m| m.enabled)
    //         .collect())
    // }

    pub async fn create_display(
        &self,
        name: Option<String>,
        width: u32,
        height: u32,
        refresh_rate: u32,
    ) -> Result<Id> {
        let mut client = self.client.lock().await;
        let mut managed = self.managed_displays.lock().await;

        let id = client
            .new_id(None)
            .ok_or_else(|| anyhow!("Failed to generate monitor ID"))?;

        let monitor = Monitor {
            id,
            name,
            enabled: true,
            modes: vec![Mode {
                width,
                height,
                refresh_rates: vec![refresh_rate],
            }],
        };

        client.add(monitor)?;
        client.persist()?;
        client.notify().await?;

        managed.push(id);
        Ok(id)
    }

    // updates and activates found display or creates new one
    pub async fn update_display(
        &self,
        id: Id,
        width: Option<u32>,
        height: Option<u32>,
        refresh_rate: Option<u32>,
    ) -> Result<()> {
        dbg!("huh??");

        let res = {
            let mut client = self.client.lock().await;
            client.find_monitor_mut_query(&id.to_string(), |monitor| {
                monitor.enabled = true;
                if let Some(current_mode) = monitor.modes.first_mut() {
                    *current_mode = Mode {
                        width: width.unwrap_or(current_mode.width),
                        height: height.unwrap_or(current_mode.height),
                        refresh_rates: vec![refresh_rate
                            .unwrap_or(*current_mode.refresh_rates.first().unwrap_or(&60))],
                    };
                    // Keep only one mode
                    monitor.modes.truncate(1);
                }
            })
        };
        dbg!("huh2??");

        if let None = res {
            self.create_display(
                None,
                width.unwrap_or(1920),
                height.unwrap_or(1080),
                refresh_rate.unwrap_or(60),
            )
            .await?;
        };
        dbg!("huh3??");

        let mut client = self.client.lock().await;
        client.persist()?;
        client.notify().await?;
        dbg!("huh4??");

        Ok(())
    }

    async fn list_displays(&self) -> Result<()> {
        let client = self.client.lock().await;
        let monitors = client.monitors();

        println!("Managed Displays:");
        for monitor in monitors {
            let mode = monitor
                .modes
                .first()
                .map(|m| {
                    format!(
                        "{}x{}@{}Hz",
                        m.width,
                        m.height,
                        m.refresh_rates.first().unwrap_or(&0)
                    )
                })
                .unwrap_or_else(|| "No modes".to_string());

            println!(
                "[{}] {} - {} ({})",
                if monitor.enabled { "✓" } else { " " },
                monitor.id,
                monitor.name.as_deref().unwrap_or("Unnamed"),
                mode
            );
        }
        Ok(())
    }

    pub async fn exit(&self) -> Result<()> {
        let mut client = self.client.lock().await;

        // Disable all monitors
        let monitors = client.monitors();
        let ids: Vec<String> = monitors.iter().map(|m| m.id.to_string()).collect();

        dbg!("huh?");
        let _ = client.set_enabled_query(&ids, false);
        let _ = client.persist();
        let _ = client.notify().await?;
        Ok(())
    }
}

// pub struct VirtualDisplayManagerWrapper {
//     command_tx: mpsc::UnboundedSender<DisplayCommand>,
// }

// impl VirtualDisplayManagerWrapper {
//     pub async fn new() -> Result<Self> {
//         let (command_tx, mut command_rx) = mpsc::unbounded_channel();

//         // Spawn a dedicated handler thread
//         tokio::spawn(async move {
//             let client = VirtualDisplayManager::new().await.unwrap();

//             while let Some(command) = command_rx.recv().await {
//                 client.handle_command(command).await;
//             }
//         });

//         Ok(Self { command_tx })
//     }
// }

impl Drop for VirtualDisplayManager {
    fn drop(&mut self) {}
}

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct StartVideoMsg {
    pub display: i32,
}
