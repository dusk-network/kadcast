// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::encoding::payload::BroadcastPayload;

use super::Encoder;

pub(crate) struct PlainEncoder {}

impl Encoder for PlainEncoder {
    fn encode<'msg>(&self, msg: &'msg [u8]) -> Vec<&'msg [u8]> {
        vec![msg]
    }

    fn decode(&self, chunk: BroadcastPayload) -> Option<BroadcastPayload> {
        Some(chunk)
    }
}
