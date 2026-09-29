use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use super::process::Pid;

#[derive(Debug, Clone)]
pub enum IpcMessage {
    Signal {
        kind: SignalKind,
    },
    JobAssigned {
        job_id: String,
        payload: serde_json::Value,
    },
    JobCancelled {
        job_id: String,
    },
    CapabilityGranted {
        kind: String,
        resource: String,
    },
    CapabilityRevoked {
        kind: String,
    },
    DeviceEvent {
        device_id: String,
        event: String,
    },
    HeartbeatRequest,
    Custom {
        tag: String,
        payload: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalKind {
    Terminate,
    Suspend,
    Resume,
    Reload,
}

pub struct MessageChannel {
    pub pid: Pid,
    sender: mpsc::Sender<IpcMessage>,
    receiver: Arc<Mutex<mpsc::Receiver<IpcMessage>>>,
}

impl MessageChannel {
    fn new(pid: Pid, capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        Self {
            pid,
            sender: tx,
            receiver: Arc::new(Mutex::new(rx)),
        }
    }

    pub fn sender(&self) -> mpsc::Sender<IpcMessage> {
        self.sender.clone()
    }

    pub async fn recv(&self) -> Option<IpcMessage> {
        self.receiver.lock().unwrap().recv().await
    }
}

pub struct SovereignIPC {
    channels: Mutex<HashMap<Pid, mpsc::Sender<IpcMessage>>>,
}

impl SovereignIPC {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            channels: Mutex::new(HashMap::new()),
        })
    }

    pub fn register(&self, pid: Pid, capacity: usize) -> MessageChannel {
        let ch = MessageChannel::new(pid, capacity);
        self.channels.lock().unwrap().insert(pid, ch.sender());
        ch
    }

    pub fn unregister(&self, pid: Pid) {
        self.channels.lock().unwrap().remove(&pid);
    }

    pub async fn send(&self, pid: Pid, msg: IpcMessage) -> bool {
        let tx = {
            let map = self.channels.lock().unwrap();
            map.get(&pid).cloned()
        };
        match tx {
            Some(tx) => tx.send(msg).await.is_ok(),
            None => false,
        }
    }

    pub async fn broadcast(&self, msg: IpcMessage) -> usize {
        let senders: Vec<_> = {
            let map = self.channels.lock().unwrap();
            map.values().cloned().collect()
        };
        let mut sent = 0usize;
        for tx in senders {
            if tx.send(msg.clone()).await.is_ok() {
                sent += 1;
            }
        }
        sent
    }
}

impl Default for SovereignIPC {
    fn default() -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
        }
    }
}
