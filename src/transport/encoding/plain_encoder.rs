// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::collections::HashMap;

use crate::encoding::message::Message;

use super::{Configurable, Decoder, Encoder};

pub(crate) struct PlainEncoder {}

impl Configurable for PlainEncoder {
    fn configure(_: &HashMap<String, String>) -> Self {
        PlainEncoder {}
    }
    fn default_configuration() -> HashMap<String, String> {
        HashMap::new()
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
