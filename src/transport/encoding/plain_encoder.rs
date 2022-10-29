// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io;

use super::{Configurable, Decoder, Encoder};
use crate::encoding::message::Message;

pub struct PlainEncoder {}

impl Configurable for PlainEncoder {
    type TConf = ();
    fn default_configuration() -> Self::TConf {}
    fn configure(_: &Self::TConf) -> Self {
        PlainEncoder {}
    }
}

impl Encoder for PlainEncoder {
    fn encode<'msg>(&self, msg: Message) -> io::Result<Vec<Message>> {
        Ok(vec![msg])
    }
}

impl Decoder for PlainEncoder {
    fn decode(&mut self, chunk: Message) -> io::Result<Option<Message>> {
        Ok(Some(chunk))
    }
}
