use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use crate::prelude::*;
use crate::sound_streams::EncodedBuffer;
use futures_lite::future::Boxed as BoxedFuture;
use iroh::{
    endpoint::Connection,
    protocol::{ProtocolHandler, Router},
    Endpoint, NodeAddr, PublicKey, SecretKey,
};
use serde::{Deserialize, Serialize};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{channel, Receiver, Sender},
};

// maybe a newtype for private key to avoid leaking it?
pub type Key = [u8; 32];

const ALPN: &str = "shamble/room/0";

pub enum Command {
    ConnectTo(PublicKey),
    SendMsg(NetMsg),
}
pub type Order = u64;

#[derive(Serialize, Deserialize, Debug)]
pub enum NetMsg {
    /// Someone new joined
    Introduce([u8; 32]),
    MyName(String),
    // TODO: just use the reliable channel
    /// in case name didn't arrive
    AskForName,
    Voice((Order, Vec<u8>)),
}

pub struct NetState {
    pub cmd: Sender<Command>,
}

impl NetState {
    pub(crate) fn with_pk(pk: &SecretKey) -> NetState {
        let (cmd_sender, mut msg_receiver) = start_actor(pk);

        // TODO: send this to proper channels
        tokio::spawn(async move {
            while let Some(msg) = msg_receiver.recv().await {
                info!(?msg);
            }
        });

        NetState { cmd: cmd_sender }
    }
}

fn greet_and_listen(connection: Connection, msg_sender: Sender<NetMsg>) {
    let c = connection.clone();
    tokio::spawn(async move {
        while let Ok(d) = c.read_datagram().await {
            if let Ok(m) = bincode::deserialize(&d) {
                if let Err(e) = msg_sender.try_send(m) {
                    warn!(?e, "net msg receiver queue full");
                }
            }
        }
    });

    let name = std::env::args()
        .nth(1)
        .unwrap_or("noname".into())
        .chars()
        .take(100)
        .collect();
    let name = bincode::serialize(&NetMsg::MyName(name)).unwrap();
    if let Err(e) = connection.send_datagram(name.into()) {
        error!(?e, "Sending my name");
    }
}

/// assumes tokio runtime installed
pub fn start_actor(pk: &SecretKey) -> (Sender<Command>, Receiver<NetMsg>) {
    let (cmd_sender, mut cmd_receiver) = channel(10);
    let (msg_sender, msg_receiver) = channel(100);
    let pk = pk.clone();
    let connections = SharedConnections::default();

    tokio::spawn(async move {
        let endpoint = Endpoint::builder()
            .secret_key(pk.clone())
            .discovery_n0()
            .discovery_local_network()
            .bind()
            .await
            .unwrap();

        let router = Router::builder(endpoint)
            .accept(
                ALPN,
                ShambleProtocolHandler {
                    msg: msg_sender.clone(),
                    connections: connections.clone(),
                },
            )
            .spawn()
            .await
            .unwrap();

        // spawn cmd listener
        let msg_sender = msg_sender.clone();
        tokio::spawn(async move {
            info!("Cmd listener started");
            loop {
                let cmd = match cmd_receiver.recv().await {
                    Some(c) => c,
                    _ => {
                        info!("Channel dropped, quitting cmd received task");
                        break;
                    }
                };
                info!("Received message");

                match cmd {
                    Command::ConnectTo(addr) => {
                        info!(?addr, "Connecting");
                        let connection =
                            match router.endpoint().connect(addr, ALPN.as_bytes()).await {
                                Ok(c) => c,
                                Err(e) => {
                                    error!(?e, "connecting");
                                    continue;
                                }
                            };
                        info!(addr=?connection.remote_address(), "Connected");
                        // FIXME: timeout
                        // let (serd, recv) = match connection.open_bi().await {
                        //     Ok((send ,recv)) => (send,recv),
                        //     Err(e) => {error!(?e, "sending bi"); continue;}
                        // };
                        //
                        greet_and_listen(connection.clone(), msg_sender.clone());

                        connections
                            .lock()
                            .unwrap()
                            .insert(connection.stable_id(), connection.clone());

                        let debug_conn = connections
                            .lock()
                            .unwrap()
                            .iter()
                            .map(|(k, v)| (*k, v.remote_address()))
                            .collect::<Vec<_>>();
                        info!(?debug_conn);
                    }
                    Command::SendMsg(net_msg) => {
                        todo!("Share list of connections and broadcast the message");
                    }
                }
            }
            router.shutdown().await.unwrap();
        });
    });
    (cmd_sender, msg_receiver)
}

type SharedConnections = Arc<Mutex<HashMap<usize, Connection>>>;

#[derive(Debug)]
struct ShambleProtocolHandler {
    msg: Sender<NetMsg>,
    connections: SharedConnections,
}

impl ProtocolHandler for ShambleProtocolHandler {
    fn accept(&self, conn: iroh::endpoint::Connecting) -> BoxedFuture<anyhow::Result<()>> {
        let conns = self.connections.clone();
        let msg_sender = self.msg.clone();
        Box::pin(async move {
            let connection = conn.await?;

            info!(addr=?connection.remote_address(), "Incoming connection");
            greet_and_listen(connection.clone(), msg_sender);
            conns
                .lock()
                .unwrap()
                .insert(connection.stable_id(), connection.clone());

            let debug_conn = conns
                .lock()
                .unwrap()
                .iter()
                .map(|(k, v)| (*k, v.remote_address()))
                .collect::<Vec<_>>();
            info!(?debug_conn);
            Ok(())
        })
    }
}
