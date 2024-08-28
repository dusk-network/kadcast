// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/*
 * The original implementation of this file can be found at:
 * https://github.com/cberner/raptorq
 *
 * The original work is licensed under the Apache License, Version 2.0.
 *
 * Modifications made:
 * - Add implementation for a safe ObjectTransmissionInformation
 *   deserialization that checks parameter in order to prevent panic at
 *   runtime
 *
 * The following is a notice for the original file:
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::convert::{TryFrom, TryInto};

use raptorq::ObjectTransmissionInformation;

use super::{encoder::MAX_MTU, TRANSMISSION_INFO_SIZE};

// This should eventually become <https://doc.rust-lang.org/std/primitive.u64.html#method.div_ceil>
// when it gets stabilized, and this function should be removed.
// (1) the result is known to not overflow u32 from elsewhere;
// (2) `denom` is known to not be `0` from elsewhere.
// TODO this is definitely not always the case! Let's do something about it.
fn int_div_ceil(num: u64, denom: u64) -> u32 {
    if num % denom == 0 {
        (num / denom) as u32
    } else {
        (num / denom + 1) as u32
    }
}

// K'_max as defined in section 5.1.2
const MAX_SOURCE_SYMBOLS_PER_BLOCK: u32 = 56403;

#[derive(Debug, Clone)]
pub(crate) struct SafeObjectTransmissionInformation {
    pub inner: ObjectTransmissionInformation,
    pub max_blocks: usize,
}

#[derive(Debug, Clone)]
pub enum TransmissionInformationError {
    InvalidSize,
    SourceBlocksZero,
    SymbolSizeZero,
    SymbolSizeGreaterThanMTU,
    SymbolSizeNotAligned,
    TransferLengthZero,
    TransferLengthExceeded,
    TooManySourceSymbols,
}

impl TryFrom<&[u8]> for SafeObjectTransmissionInformation {
    type Error = TransmissionInformationError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let value: &[u8; TRANSMISSION_INFO_SIZE] = value
            .try_into()
            .map_err(|_| TransmissionInformationError::InvalidSize)?;
        let config = ObjectTransmissionInformation::deserialize(value);

        if config.source_blocks() == 0 {
            return Err(TransmissionInformationError::SourceBlocksZero);
        }

        if config.symbol_size() == 0 {
            return Err(TransmissionInformationError::SymbolSizeZero);
        }

        if config.transfer_length() == 0 {
            return Err(TransmissionInformationError::TransferLengthZero);
        }

        if config.symbol_size() > MAX_MTU {
            return Err(TransmissionInformationError::SymbolSizeGreaterThanMTU);
        }

        if config.symbol_alignment() != 8 {
            return Err(TransmissionInformationError::SymbolSizeNotAligned);
        }

        let kt =
            int_div_ceil(config.transfer_length(), config.symbol_size() as u64);

        let (kl, _, zl, zs) = raptorq::partition(kt, config.source_blocks());

        let block_length = u64::from(kl) * u64::from(config.symbol_size());
        let source_block_symbols =
            int_div_ceil(block_length, config.symbol_size() as u64);

        match source_block_symbols <= MAX_SOURCE_SYMBOLS_PER_BLOCK {
            true => Ok(SafeObjectTransmissionInformation {
                inner: config,
                max_blocks: (zl + zs) as usize,
            }),
            false => Err(TransmissionInformationError::TooManySourceSymbols),
        }
    }
}
