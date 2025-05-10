use futures::{Stream, StreamExt, future};
use serde::Deserialize;
use std::{collections::HashMap, time::Duration};
use sysinfo::System;
use tokio::sync::mpsc;
use wmi::{COMLibrary, FilterValue, WMIConnection};

use crate::{errors::Error, errors::Result, tracker::OsEvent, utils::LocalSpawner};

const D2R_PROCESS: &str = "D2R.exe";

#[derive(Deserialize)]
#[serde(rename = "Win32_Process")]
#[serde(rename_all = "PascalCase")]
struct WMIProcess {
    process_id: u32,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename = "__InstanceCreationEvent")]
#[serde(rename_all = "PascalCase")]
struct WMIProcessCreationEvent {
    target_instance: WMIProcess,
}

#[derive(Deserialize)]
#[serde(rename = "__InstanceDeletionEvent")]
#[serde(rename_all = "PascalCase")]
struct WMIProcessDeletionEvent {
    target_instance: WMIProcess,
}

struct WMIProcessTracker {
    wmi_connection: WMIConnection,
}

impl WMIProcessTracker {
    fn new() -> Result<Self> {
        println!("New WMIProcessTracker");
        let com_library = COMLibrary::new()?;
        let wmi_connection = WMIConnection::new(com_library)?;
        Ok(Self { wmi_connection })
    }

    fn wmi_filtered_notification<T>(&self, duration: Duration) -> Result<impl Stream<Item = T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let wmi_filters = HashMap::from([(
            String::from("TargetInstance"),
            FilterValue::is_a::<WMIProcess>()?,
        )]);

        let stream = self
            .wmi_connection
            .async_filtered_notification::<T>(&wmi_filters, Some(duration))?;

        Ok(stream
            .filter(|p| future::ready(p.is_ok()))
            .map(|p| p.unwrap()))
    }

    async fn process_creation_stream(&self, duration: Duration) -> Result<impl Stream<Item = u32>> {
        let stream = self.wmi_filtered_notification::<WMIProcessCreationEvent>(duration)?;

        Ok(stream
            .map(|p| p.target_instance)
            .filter(|p| future::ready(p.name == D2R_PROCESS))
            .map(|p| p.process_id))
    }

    async fn process_deletion_stream(&self, duration: Duration) -> Result<impl Stream<Item = u32>> {
        let stream = self.wmi_filtered_notification::<WMIProcessDeletionEvent>(duration)?;

        Ok(stream
            .map(|p| p.target_instance)
            .filter(|p| future::ready(p.name == D2R_PROCESS))
            .map(|p| p.process_id))
    }
}

pub struct ProcessTracker {}

impl ProcessTracker {
    pub async fn new(send: mpsc::Sender<OsEvent>, duration: Duration) -> Result<()> {
        {
            let system = System::new_all();

            for process in system.processes_by_exact_name(D2R_PROCESS.as_ref()) {
                send.send(OsEvent::ProcessCreation(process.pid().as_u32()))
                    .await?;
            }
        }

        LocalSpawner::new("process tracker", move || {
            let send = send.clone();

            async move {
                let process_tracker = WMIProcessTracker::new()?;

                tokio::select! {
                    result = async {
                        let mut stream = process_tracker.process_creation_stream(duration).await?;

                        while let Some(pid) = stream.next().await {
                            send.send(OsEvent::ProcessCreation(pid)).await?;
                        }

                        Ok::<(), Error>(())
                    } => result,

                    result = async {
                        let mut stream = process_tracker.process_deletion_stream(duration).await?;

                        while let Some(pid) = stream.next().await {
                            send.send(OsEvent::ProcessDeletion(pid)).await?;
                        }

                        Ok::<(), Error>(())
                    } => result,
                }
            }
        })
    }
}
