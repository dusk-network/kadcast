// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod plain_encoder;
mod raptorq;

pub(crate) use self::raptorq::RaptorQDecoder as TransportDecoder;
pub(crate) use self::raptorq::RaptorQEncoder as TransportEncoder;

pub type TransportEncoderConfig =
    <self::TransportEncoder as BaseConfigurable>::TConf;
pub type TransportDecoderConfig =
    <self::TransportDecoder as BaseConfigurable>::TConf;
use crate::encoding::message::Message;
use async_trait::async_trait;
use tokio::io;

pub trait BaseConfigurable {
    type TConf;
    fn default_configuration() -> Self::TConf;
}

pub trait Configurable: BaseConfigurable {
    fn configure(conf: &Self::TConf) -> Self;
}

#[async_trait]
pub trait AsyncConfigurable: BaseConfigurable + Sized {
    async fn configure(conf: &Self::TConf) -> io::Result<Self>;
}

pub(crate) trait Encoder: BaseConfigurable {
    fn encode(&self, msg: Message) -> Vec<Message>;
}

pub(crate) trait Decoder: BaseConfigurable {
    fn decode(&mut self, chunk: Message) -> Option<Message>;
}
