// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod plain_encoder;
mod raptorq;

use std::collections::HashMap;

pub(crate) use self::raptorq::RaptorQDecoder;
pub(crate) use self::raptorq::RaptorQEncoder;

use crate::encoding::message::Message;

pub(crate) trait Configurable {
    fn configure(conf: HashMap<String, String>) -> Self;
}

pub(crate) trait Encoder: Configurable {
    fn encode(&self, msg: Message) -> Vec<Message>;
}

pub(crate) trait Decoder: Configurable {
    fn decode(&mut self, chunk: Message) -> Option<Message>;
}
