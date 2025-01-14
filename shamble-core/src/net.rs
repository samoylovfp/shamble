use std::{
    collections::HashSet,
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

pub enum NetMsg {
    /// Someone new joined
    Introduce([u8; 32]),
    Voice((u64, EncodedBuffer)),
}

pub struct NetState {
    netmsg: Receiver<NetMsg>,
    pub cmd: Sender<Command>,
}

impl NetState {
    pub(crate) fn with_pk(pk: &SecretKey) -> NetState {
        let (cmd_sender, msg_receiver) = start_actor(pk);
        NetState {
            netmsg: msg_receiver,
            cmd: cmd_sender,
        }
    }
}

/// assumes tokio runtime installed
pub fn start_actor(pk: &SecretKey) -> (Sender<Command>, Receiver<NetMsg>) {
    let (cmd_sender, mut cmd_receiver) = channel(10);
    let (msg_sender, msg_receiver) = channel(100);
    let pk = pk.clone();
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
                    msg: msg_sender,
                    connections: Default::default(),
                },
            )
            .spawn()
            .await
            .unwrap();

        // spawn cmd listener
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

#[derive(Debug)]
struct ShambleProtocolHandler {
    msg: Sender<NetMsg>,
    connections: Arc<Mutex<HashSet<Connection>>>,
}

impl ProtocolHandler for ShambleProtocolHandler {
    fn accept(&self, conn: iroh::endpoint::Connecting) -> BoxedFuture<anyhow::Result<()>> {
        Box::pin(async move {
            let connection = conn.await?;

            info!(addr=?connection.remote_address(), "Incoming connection");
            // TODO: add connection to a list of live connections
            Ok(())
        })
    }
}
