mod net;
mod prelude;
mod sound_streams;

use iroh::{PublicKey, SecretKey};
pub use net::{Command, Key};
use tokio::sync::mpsc::Sender;

use crate::prelude::*;
use cpal::{Device, Stream};
use net::NetState;
use sound_streams::SoundState;

pub struct ShambleState {
    sound_state: SoundState,
    net_state: NetState,
}

impl ShambleState {
    pub fn new(pk: &SecretKey) -> Self {
        Self {
            sound_state: SoundState::default(),
            net_state: NetState::with_pk(pk),
        }
    }

    pub fn command(&self) -> Sender<Command> {
        self.net_state.cmd.clone()
    }
}
