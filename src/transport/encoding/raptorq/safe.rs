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

use std::convert::TryFrom;

use raptorq::ObjectTransmissionInformation;

use super::TRANSMISSION_INFO_SIZE;
use super::encoder::{MAX_MTU, MIN_MTU};

// K'_max as defined in section 5.1.2
const MAX_SOURCE_SYMBOLS_PER_BLOCK: u32 = 56403;

#[derive(Debug, Clone)]
pub(crate) struct SafeObjectTransmissionInformation {
    pub inner: ObjectTransmissionInformation,
    pub max_blocks: usize,
}

#[derive(Debug, Clone)]
pub enum TransmissionInformationError {
    SourceBlocksZero,
    SymbolSizeZero,
    SymbolSizeGreaterThanMTU,
    SymbolSizeLessThanMTU,
    SymbolSizeNotAligned,
    InvalidSubBlocks,
    TooManySourceBlocks,
    TransferLengthZero,
    TransferLengthExceeded,
    TooManySourceSymbols,
}

impl TryFrom<&[u8; TRANSMISSION_INFO_SIZE]>
    for SafeObjectTransmissionInformation
{
    type Error = TransmissionInformationError;

    fn try_from(
        value: &[u8; TRANSMISSION_INFO_SIZE],
    ) -> Result<Self, Self::Error> {
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

        // Encoders never use symbols below the minimum MTU. Rejecting them
        // bounds solver dimensions by `max_udp_len / MIN_MTU`.
        if config.symbol_size() < MIN_MTU {
            return Err(TransmissionInformationError::SymbolSizeLessThanMTU);
        }

        if config.symbol_alignment() != 8 || config.symbol_size() % 8 != 0 {
            return Err(TransmissionInformationError::SymbolSizeNotAligned);
        }

        if config.sub_blocks() == 0
            || config.sub_blocks() > config.symbol_size() / 8
        {
            return Err(TransmissionInformationError::InvalidSubBlocks);
        }

        let kt = u32::try_from(
            config
                .transfer_length()
                .div_ceil(u64::from(config.symbol_size())),
        )
        .map_err(|_| TransmissionInformationError::TooManySourceSymbols)?;

        if kt < u32::from(config.source_blocks()) {
            return Err(TransmissionInformationError::TooManySourceBlocks);
        }

        let (kl, _, zl, zs) = raptorq::partition(kt, config.source_blocks());

        match kl <= MAX_SOURCE_SYMBOLS_PER_BLOCK {
            true => Ok(SafeObjectTransmissionInformation {
                inner: config,
                max_blocks: (zl + zs) as usize,
            }),
            false => Err(TransmissionInformationError::TooManySourceSymbols),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(
        transfer_length: u64,
        symbol_size: u16,
    ) -> Result<SafeObjectTransmissionInformation, TransmissionInformationError>
    {
        let info = ObjectTransmissionInformation::with_defaults(
            transfer_length,
            symbol_size,
        );
        SafeObjectTransmissionInformation::try_from(&info.serialize())
    }

    #[test]
    fn symbol_size_bounds_source_symbols() {
        assert!(matches!(
            validate(1, MIN_MTU - 8),
            Err(TransmissionInformationError::SymbolSizeLessThanMTU)
        ));
        // A block with more than 8_192 source symbols remains valid. Decoder
        // admission still depends on both configured memory budgets.
        let info = validate(20 * 1_024 * 1_024, MIN_MTU).unwrap();
        assert_eq!(info.max_blocks, 1);
    }
}
