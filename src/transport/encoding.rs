// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod plain_encoder;
mod raptorq_encoder;
pub(crate) use plain_encoder::PlainEncoder;
pub(crate) use raptorq_encoder::RaptorQEncoder;

use crate::encoding::{message::Message};
pub(crate) trait Encoder {
    fn encode(&self, msg: Message) -> Vec<Message>;

    fn decode(&self, chunk: Message) -> Option<Message>;
}
