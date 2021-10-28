// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::encoding::message::Message;

use super::Encoder;

pub(crate) struct PlainEncoder {}

impl Encoder for PlainEncoder {
    fn encode<'msg>(&self, msg: Message) -> Vec<Message> {
        vec![msg]
    }

    fn decode(&mut self, chunk: Message) -> Option<Message> {
        if let Message::Broadcast(header, payload) = chunk {
            Some(Message::Broadcast(header, payload))
        } else {
            Some(chunk)
        }
    }
}
