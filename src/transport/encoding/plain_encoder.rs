// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::HashMap;

use crate::encoding::message::Message;

use super::{BaseConfigurable, Configurable, Decoder, Encoder};

pub(crate) struct PlainEncoder {}

impl BaseConfigurable for PlainEncoder {
    type TConf = HashMap<String, String>;
    fn default_configuration() -> Self::TConf {
        HashMap::new()
    }
}

impl Configurable for PlainEncoder {
    fn configure(_: &Self::TConf) -> Self {
        PlainEncoder {}
    }
}

impl Encoder for PlainEncoder {
    fn encode<'msg>(&self, msg: Message) -> Vec<Message> {
        vec![msg]
    }
}

impl Decoder for PlainEncoder {
    fn decode(&mut self, chunk: Message) -> Option<Message> {
        if let Message::Broadcast(header, payload) = chunk {
            Some(Message::Broadcast(header, payload))
        } else {
            Some(chunk)
        }
    }
}
