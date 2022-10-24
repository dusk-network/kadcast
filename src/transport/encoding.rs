// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod plain_encoder;
mod raptorq;

use std::io;

pub(crate) use self::raptorq::RaptorQDecoder as TransportDecoder;
pub(crate) use self::raptorq::RaptorQEncoder as TransportEncoder;
use crate::encoding::message::Message;

pub type TransportEncoderConfig =
    <self::TransportEncoder as Configurable>::TConf;
pub type TransportDecoderConfig =
    <self::TransportDecoder as Configurable>::TConf;

pub trait Configurable {
    type TConf;
    fn default_configuration() -> Self::TConf;
    fn configure(conf: &Self::TConf) -> Self;
}

pub(crate) trait Encoder: Configurable {
    fn encode(&self, msg: Message) -> io::Result<Vec<Message>>;
}

pub(crate) trait Decoder: Configurable {
    fn decode(&mut self, chunk: Message) -> io::Result<Option<Message>>;
}
