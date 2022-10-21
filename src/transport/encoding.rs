// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

#[cfg(not(feature = "raptorq"))]
mod plain_encoder;

#[cfg(not(feature = "raptorq"))]
pub(crate) use plain_encoder::PlainEncoder as TransportDecoder;

#[cfg(not(feature = "raptorq"))]
pub(crate) use plain_encoder::PlainEncoder as TransportEncoder;

#[cfg(feature = "raptorq")]
mod raptorq;

#[cfg(feature = "raptorq")]
pub(crate) use self::raptorq::RaptorQDecoder as TransportDecoder;

#[cfg(feature = "raptorq")]
pub(crate) use self::raptorq::RaptorQEncoder as TransportEncoder;

use crate::encoding::message::Message;
use std::io;

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
