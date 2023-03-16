// Copyright 2015-2020 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! ABI decoder.

#[cfg(not(feature = "std"))]
use crate::no_std_prelude::*;
use crate::{Error, ParamType, Token, Word};

#[derive(Debug)]
struct DecodeResult {
    token: Token,
    new_offset: usize,
}

fn as_usize(slice: &Word) -> Result<usize, Error> {
    if !slice[..28].iter().all(|x| *x == 0) {
        return Err(Error::InvalidData);
    }

    let result = ((slice[28] as usize) << 24)
        + ((slice[29] as usize) << 16)
        + ((slice[30] as usize) << 8)
        + (slice[31] as usize);

    Ok(result)
}

fn check_bool(slice: Word) -> Result<(), Error> {
    check_zeroes(&slice[..31])?;
    Ok(())
}

fn decode_impl(
    types: &[ParamType],
    data: &[u8],
    validate: bool,
) -> Result<(Vec<Token>, usize), Error> {
    let is_empty_bytes_valid_encoding = types.iter().all(|t| t.is_empty_bytes_valid_encoding());
    if !is_empty_bytes_valid_encoding && data.is_empty() {
        return Err(Error::InvalidName(
            "please ensure the contract and method you're calling exist! \
			 failed to decode empty bytes. if you're using jsonrpc this is \
			 likely due to jsonrpc returning `0x` in case contract or method \
			 don't exist"
                .into(),
        ));
    }

    let mut tokens = vec![];
    let mut offset = 0;

    for param in types {
        let res = decode_param(param, data, offset, validate)?;
        offset = res.new_offset;
        tokens.push(res.token);
    }
    if validate && offset != data.len() {
        return Err(Error::InvalidData);
    }

    Ok((tokens, offset))
}

/// Decodes ABI compliant vector of bytes into vector of tokens described by types param.
/// Checks, that decoded data is exact as input provided
pub fn decode_validate(types: &[ParamType], data: &[u8]) -> Result<Vec<Token>, Error> {
    decode_impl(types, data, true).map(|(tokens, _)| tokens)
}

/// Decodes ABI compliant vector of bytes into vector of tokens described by types param.
pub fn decode(types: &[ParamType], data: &[u8]) -> Result<Vec<Token>, Error> {
    decode_impl(types, data, false).map(|(tokens, _)| tokens)
}

fn peek(data: &[u8], offset: usize, len: usize) -> Result<&[u8], Error> {
    if offset + len > data.len() {
        Err(Error::InvalidData)
    } else {
        Ok(&data[offset..(offset + len)])
    }
}

fn peek_32_bytes(data: &[u8], offset: usize) -> Result<Word, Error> {
    peek(data, offset, 32).map(|x| {
        let mut out = Word::default();
        out.as_fixed_bytes_mut().copy_from_slice(&x[0..32]);
        out
    })
}

fn round_up_nearest_multiple(value: usize, padding: usize) -> usize {
    (value + padding - 1) / padding * padding
}

fn check_fixed_bytes(word: Word, len: usize) -> Result<(), Error> {
    if word == Word::default() {
        return Ok(());
    }
    match len {
        0 => Err(Error::InvalidData),
        1..=31 => check_zeroes(&word[len..]),
        32 => Ok(()),
        33.. => Err(Error::InvalidData),
        _ => unreachable!(),
    }
}

fn take_bytes(data: &[u8], offset: usize, len: usize, validate: bool) -> Result<Vec<u8>, Error> {
    if validate {
        let padded_len = round_up_nearest_multiple(len, 32);
        if offset + padded_len > data.len() {
            return Err(Error::InvalidData);
        }
        check_zeroes(&data[(offset + len)..(offset + padded_len)])?;
    } else if offset + len > data.len() {
        return Err(Error::InvalidData);
    }
    Ok(data[offset..(offset + len)].to_vec())
}

fn check_zeroes(data: &[u8]) -> Result<(), Error> {
    if data.iter().all(|b| *b == 0) {
        Ok(())
    } else {
        Err(Error::InvalidData)
    }
}

fn decode_param(
    param: &ParamType,
    data: &[u8],
    offset: usize,
    validate: bool,
) -> Result<DecodeResult, Error> {
    match *param {
        ParamType::Address => {
            let slice = peek_32_bytes(data, offset)?;
            if validate {
                check_zeroes(&slice[..12])?;
            }
            let result = DecodeResult {
                token: Token::Word(slice),
                new_offset: offset + 32,
            };
            Ok(result)
        }
        ParamType::Int(_) => {
            let slice = peek_32_bytes(data, offset)?;
            let result = DecodeResult {
                token: Token::Word(slice),
                new_offset: offset + 32,
            };
            Ok(result)
        }
        ParamType::Uint(_) => {
            let slice = peek_32_bytes(data, offset)?;
            let result = DecodeResult {
                token: Token::Word(slice),
                new_offset: offset + 32,
            };
            Ok(result)
        }
        ParamType::Bool => {
            let word = peek_32_bytes(data, offset)?;
            check_bool(word)?;
            let result = DecodeResult {
                token: Token::Word(word),
                new_offset: offset + 32,
            };
            Ok(result)
        }
        ParamType::FixedBytes(len) => {
            // FixedBytes is anything from bytes1 to bytes32. These values
            // are padded with trailing zeros to fill 32 bytes.
            let word = peek_32_bytes(data, offset)?;
            check_fixed_bytes(word, len)?;

            let result = DecodeResult {
                token: Token::Word(word),
                new_offset: offset + 32,
            };
            Ok(result)
        }
        ParamType::Bytes => {
            let dynamic_offset = as_usize(&peek_32_bytes(data, offset)?)?;
            let len = as_usize(&peek_32_bytes(data, dynamic_offset)?)?;
            let bytes = take_bytes(data, dynamic_offset + 32, len, validate)?;
            let result = DecodeResult {
                token: Token::PackedSeq(bytes),
                new_offset: offset + 32,
            };
            Ok(result)
        }
        ParamType::String => {
            let dynamic_offset = as_usize(&peek_32_bytes(data, offset)?)?;
            let len = as_usize(&peek_32_bytes(data, dynamic_offset)?)?;
            let bytes = take_bytes(data, dynamic_offset + 32, len, validate)?;
            let result = DecodeResult {
                // NOTE: We're decoding strings using lossy UTF-8 decoding to
                // prevent invalid strings written into contracts by either users or
                // Solidity bugs from causing graph-node to fail decoding event
                // data.
                token: Token::PackedSeq(bytes),
                new_offset: offset + 32,
            };
            Ok(result)
        }
        ParamType::Array(ref t) => {
            let len_offset = as_usize(&peek_32_bytes(data, offset)?)?;
            let len = as_usize(&peek_32_bytes(data, len_offset)?)?;

            let tail_offset = len_offset + 32;
            let tail = &data[tail_offset..];

            let mut tokens = vec![];
            let mut new_offset = 0;

            for _ in 0..len {
                let res = decode_param(t, tail, new_offset, validate)?;
                new_offset = res.new_offset;
                tokens.push(res.token);
            }

            let result = DecodeResult {
                token: Token::DynSeq(tokens),
                new_offset: offset + 32,
            };

            Ok(result)
        }
        ParamType::FixedArray(ref t, len) => {
            let is_dynamic = param.is_dynamic();

            let (tail, mut new_offset) = if is_dynamic {
                let offset = as_usize(&peek_32_bytes(data, offset)?)?;
                if offset > data.len() {
                    return Err(Error::InvalidData);
                }
                (&data[offset..], 0)
            } else {
                (data, offset)
            };

            let mut tokens = vec![];

            for _ in 0..len {
                let res = decode_param(t, tail, new_offset, validate)?;
                new_offset = res.new_offset;
                tokens.push(res.token);
            }

            let result = DecodeResult {
                token: Token::FixedSeq(tokens),
                new_offset: if is_dynamic { offset + 32 } else { new_offset },
            };

            Ok(result)
        }
        ParamType::Tuple(ref t) => {
            let is_dynamic = param.is_dynamic();

            // The first element in a dynamic Tuple is an offset to the Tuple's data
            // For a static Tuple the data begins right away
            let (tail, mut new_offset) = if is_dynamic {
                let offset = as_usize(&peek_32_bytes(data, offset)?)?;
                if offset > data.len() {
                    return Err(Error::InvalidData);
                }
                (&data[offset..], 0)
            } else {
                (data, offset)
            };

            let len = t.len();
            let mut tokens = Vec::with_capacity(len);
            for param in t {
                let res = decode_param(param, tail, new_offset, validate)?;
                new_offset = res.new_offset;
                tokens.push(res.token);
            }

            // The returned new_offset depends on whether the Tuple is dynamic
            // dynamic Tuple -> follows the prefixed Tuple data offset element
            // static Tuple  -> follows the last data element
            let result = DecodeResult {
                token: Token::FixedSeq(tokens),
                new_offset: if is_dynamic { offset + 32 } else { new_offset },
            };

            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use ethers_primitives::{B160, B256};
    use hex_literal::hex;

    #[cfg(not(feature = "std"))]
    use crate::no_std_prelude::*;
    use crate::{decode, decode_validate, util::pad_u32, ParamType, Token, Tokenize};

    #[test]
    fn decode_from_empty_byte_slice() {
        // these can NOT be decoded from empty byte slice
        assert!(decode(&[ParamType::Address], &[]).is_err());
        assert!(decode(&[ParamType::Bytes], &[]).is_err());
        assert!(decode(&[ParamType::Int(0)], &[]).is_err());
        assert!(decode(&[ParamType::Int(1)], &[]).is_err());
        assert!(decode(&[ParamType::Int(0)], &[]).is_err());
        assert!(decode(&[ParamType::Int(1)], &[]).is_err());
        assert!(decode(&[ParamType::Bool], &[]).is_err());
        assert!(decode(&[ParamType::String], &[]).is_err());
        assert!(decode(&[ParamType::Array(Box::new(ParamType::Bool))], &[]).is_err());
        assert!(decode(&[ParamType::FixedBytes(1)], &[]).is_err());
        assert!(decode(&[ParamType::FixedArray(Box::new(ParamType::Bool), 1)], &[]).is_err());

        assert!(decode(&[ParamType::FixedBytes(0)], &[]).is_err());
        // these are the only ones that can be decoded from empty byte slice
        assert!(decode(&[ParamType::FixedArray(Box::new(ParamType::Bool), 0)], &[]).is_ok());
    }

    #[test]
    fn decode_static_tuple_of_addresses_and_uints() {
        let encoded = hex!(
            "
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
			1111111111111111111111111111111111111111111111111111111111111111
		"
        );
        let address1 = B160([0x11u8; 20]).to_token();
        let address2 = B160([0x22u8; 20]).to_token();
        let uint = Token::Word([0x11u8; 32].into());
        let tuple = Token::FixedSeq(vec![address1, address2, uint]);
        let expected = vec![tuple];
        let decoded = decode(
            &[ParamType::Tuple(vec![
                ParamType::Address,
                ParamType::Address,
                ParamType::Uint(32),
            ])],
            &encoded,
        )
        .unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decode_dynamic_tuple() {
        let encoded = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000040
			0000000000000000000000000000000000000000000000000000000000000080
			0000000000000000000000000000000000000000000000000000000000000009
			6761766f66796f726b0000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000009
			6761766f66796f726b0000000000000000000000000000000000000000000000
		"
        );
        let string1 = Token::PackedSeq(b"gavofyork".to_vec());
        let string2 = Token::PackedSeq(b"gavofyork".to_vec());
        let tuple = Token::FixedSeq(vec![string1, string2]);
        let decoded = decode(
            &[ParamType::Tuple(vec![ParamType::String, ParamType::String])],
            &encoded,
        )
        .unwrap();
        let expected = vec![tuple];
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decode_nested_tuple() {
        let encoded = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000080
			0000000000000000000000000000000000000000000000000000000000000001
			00000000000000000000000000000000000000000000000000000000000000c0
			0000000000000000000000000000000000000000000000000000000000000100
			0000000000000000000000000000000000000000000000000000000000000004
			7465737400000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000006
			6379626f72670000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000060
			00000000000000000000000000000000000000000000000000000000000000a0
			00000000000000000000000000000000000000000000000000000000000000e0
			0000000000000000000000000000000000000000000000000000000000000005
			6e69676874000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000003
			6461790000000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000040
			0000000000000000000000000000000000000000000000000000000000000080
			0000000000000000000000000000000000000000000000000000000000000004
			7765656500000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000008
			66756e7465737473000000000000000000000000000000000000000000000000
		"
        );
        let string1 = Token::PackedSeq(b"test".to_vec());
        let string2 = Token::PackedSeq(b"cyborg".to_vec());
        let string3 = Token::PackedSeq(b"night".to_vec());
        let string4 = Token::PackedSeq(b"day".to_vec());
        let string5 = Token::PackedSeq(b"weee".to_vec());
        let string6 = Token::PackedSeq(b"funtests".to_vec());
        let bool = true.to_token();
        let deep_tuple = Token::FixedSeq(vec![string5, string6]);
        let inner_tuple = Token::FixedSeq(vec![string3, string4, deep_tuple]);
        let outer_tuple = Token::FixedSeq(vec![string1, bool, string2, inner_tuple]);
        let expected = vec![outer_tuple];
        let decoded = decode(
            &[ParamType::Tuple(vec![
                ParamType::String,
                ParamType::Bool,
                ParamType::String,
                ParamType::Tuple(vec![
                    ParamType::String,
                    ParamType::String,
                    ParamType::Tuple(vec![ParamType::String, ParamType::String]),
                ]),
            ])],
            &encoded,
        )
        .unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decode_complex_tuple_of_dynamic_and_static_types() {
        let encoded = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			1111111111111111111111111111111111111111111111111111111111111111
			0000000000000000000000000000000000000000000000000000000000000080
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
			0000000000000000000000000000000000000000000000000000000000000009
			6761766f66796f726b0000000000000000000000000000000000000000000000
		"
        );
        let uint = Token::Word([0x11u8; 32].into());
        let string = Token::PackedSeq(b"gavofyork".to_vec());
        let address1 = B160([0x11u8; 20]).to_token();
        let address2 = B160([0x22u8; 20]).to_token();
        let tuple = Token::FixedSeq(vec![uint, string, address1, address2]);
        let expected = vec![tuple];
        let decoded = decode(
            &[ParamType::Tuple(vec![
                ParamType::Uint(32),
                ParamType::String,
                ParamType::Address,
                ParamType::Address,
            ])],
            &encoded,
        )
        .unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decode_params_containing_dynamic_tuple() {
        let encoded = hex!(
            "
			0000000000000000000000002222222222222222222222222222222222222222
			00000000000000000000000000000000000000000000000000000000000000a0
			0000000000000000000000003333333333333333333333333333333333333333
			0000000000000000000000004444444444444444444444444444444444444444
			0000000000000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000001
			0000000000000000000000000000000000000000000000000000000000000060
			00000000000000000000000000000000000000000000000000000000000000a0
			0000000000000000000000000000000000000000000000000000000000000009
			7370616365736869700000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000006
			6379626f72670000000000000000000000000000000000000000000000000000
		"
        );
        let address1 = B160([0x22u8; 20]).to_token();
        let bool1 = true.to_token();
        let string1 = Token::PackedSeq(b"spaceship".to_vec());
        let string2 = Token::PackedSeq(b"cyborg".to_vec());
        let tuple = Token::FixedSeq(vec![bool1, string1, string2]);
        let address2 = B160([0x33u8; 20]).to_token();
        let address3 = B160([0x44u8; 20]).to_token();
        let bool2 = false.to_token();
        let expected = vec![address1, tuple, address2, address3, bool2];
        let decoded = decode(
            &[
                ParamType::Address,
                ParamType::Tuple(vec![ParamType::Bool, ParamType::String, ParamType::String]),
                ParamType::Address,
                ParamType::Address,
                ParamType::Bool,
            ],
            &encoded,
        )
        .unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decode_params_containing_static_tuple() {
        let encoded = hex!(
            "
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
			0000000000000000000000000000000000000000000000000000000000000001
			0000000000000000000000000000000000000000000000000000000000000000
			0000000000000000000000003333333333333333333333333333333333333333
			0000000000000000000000004444444444444444444444444444444444444444
		"
        );
        let address1 = B160([0x11u8; 20]).to_token();
        let address2 = B160([0x22u8; 20]).to_token();
        let bool1 = true.to_token();
        let bool2 = false.to_token();
        let tuple = Token::FixedSeq(vec![address2, bool1, bool2]);
        let address3 = B160([0x33u8; 20]).to_token();
        let address4 = B160([0x44u8; 20]).to_token();

        let expected = vec![address1, tuple, address3, address4];
        let decoded = decode(
            &[
                ParamType::Address,
                ParamType::Tuple(vec![ParamType::Address, ParamType::Bool, ParamType::Bool]),
                ParamType::Address,
                ParamType::Address,
            ],
            &encoded,
        )
        .unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn decode_data_with_size_that_is_not_a_multiple_of_32() {
        let encoded = hex!(
            "
            0000000000000000000000000000000000000000000000000000000000000000
            00000000000000000000000000000000000000000000000000000000000000a0
            0000000000000000000000000000000000000000000000000000000000000152
            0000000000000000000000000000000000000000000000000000000000000001
            000000000000000000000000000000000000000000000000000000000054840d
            0000000000000000000000000000000000000000000000000000000000000092
            3132323033393637623533326130633134633938306235616566666231373034
            3862646661656632633239336139353039663038656233633662306635663866
            3039343265376239636337366361353163636132366365353436393230343438
            6533303866646136383730623565326165313261323430396439343264653432
            3831313350373230703330667073313678390000000000000000000000000000
            0000000000000000000000000000000000103933633731376537633061363531
            3761
        "
        );

        assert_eq!(
			decode(
				&[
					ParamType::Uint(256),
					ParamType::String,
					ParamType::String,
					ParamType::Uint(256),
					ParamType::Uint(256),
				],
				&encoded,
			).unwrap(),
			&[
				Token::Word(pad_u32(0)),
				Token::PackedSeq(b"12203967b532a0c14c980b5aeffb17048bdfaef2c293a9509f08eb3c6b0f5f8f0942e7b9cc76ca51cca26ce546920448e308fda6870b5e2ae12a2409d942de428113P720p30fps16x9".to_vec()),
				Token::PackedSeq(b"93c717e7c0a6517a".to_vec()),
				Token::Word(pad_u32(1)),
				Token::Word(pad_u32(5538829))
			]
		);
    }

    #[test]
    fn decode_after_fixed_bytes_with_less_than_32_bytes() {
        let encoded = hex!(
            "
			0000000000000000000000008497afefdc5ac170a664a231f6efb25526ef813f
			0000000000000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000080
			000000000000000000000000000000000000000000000000000000000000000a
			3078303030303030314600000000000000000000000000000000000000000000
		"
        );

        assert_eq!(
            decode(
                &[
                    ParamType::Address,
                    ParamType::FixedBytes(32),
                    ParamType::FixedBytes(4),
                    ParamType::String,
                ],
                &encoded,
            )
            .unwrap(),
            &[
                B160(hex!("8497afefdc5ac170a664a231f6efb25526ef813f")).to_token(),
                Token::Word(B256([0u8; 32])),
                Token::Word(B256([0u8; 32])),
                Token::PackedSeq("0x0000001F".into()),
            ]
        )
    }

    #[test]
    fn decode_broken_utf8() {
        let encoded = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000004
			e4b88de500000000000000000000000000000000000000000000000000000000
        "
        );

        assert_eq!(
            decode(&[ParamType::String], &encoded).unwrap(),
            &[Token::PackedSeq([0xe4, 0xb8, 0x8d, 0xe5].to_vec())]
        );
    }

    #[test]
    fn decode_corrupted_dynamic_array() {
        // line 1 at 0x00 =   0: tail offset of array
        // line 2 at 0x20 =  32: length of array
        // line 3 at 0x40 =  64: first word
        // line 4 at 0x60 =  96: second word
        let encoded = hex!(
            "
		0000000000000000000000000000000000000000000000000000000000000020
		00000000000000000000000000000000000000000000000000000000ffffffff
		0000000000000000000000000000000000000000000000000000000000000001
		0000000000000000000000000000000000000000000000000000000000000002
        "
        );

        assert!(decode(&[ParamType::Array(Box::new(ParamType::Uint(32)))], &encoded).is_err());
    }

    #[test]
    fn decode_verify_addresses() {
        let input = hex!(
            "
		0000000000000000000000000000000000000000000000000000000000012345
		0000000000000000000000000000000000000000000000000000000000054321
		"
        );
        assert!(decode(&[ParamType::Address], &input).is_ok());
        assert!(decode_validate(&[ParamType::Address], &input).is_err());
        assert!(decode_validate(&[ParamType::Address, ParamType::Address], &input).is_ok());
    }

    #[test]
    fn decode_verify_bytes() {
        let input = hex!(
            "
		0000000000000000000000001234500000000000000000000000000000012345
		0000000000000000000000005432100000000000000000000000000000054321
		"
        );
        assert!(decode_validate(&[ParamType::Address, ParamType::FixedBytes(20)], &input).is_err());
        assert!(decode_validate(&[ParamType::Address, ParamType::Address], &input).is_ok());
    }
}
