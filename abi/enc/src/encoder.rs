// Copyright 2015-2020 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! ABI encoder.

#[cfg(not(feature = "std"))]
use crate::no_std_prelude::*;
use crate::{util::pad_u32, Bytes, Token, Word};

fn pad_bytes_len(bytes: &[u8]) -> u32 {
    // "+ 1" because len is also appended
    ((bytes.len() + 31) / 32) as u32 + 1
}

fn pad_bytes_append(data: &mut Vec<Word>, bytes: &[u8]) {
    data.push(pad_u32(bytes.len() as u32));
    fixed_bytes_append(data, bytes);
}

fn fixed_bytes_append(result: &mut Vec<Word>, bytes: &[u8]) {
    let len = (bytes.len() + 31) / 32;
    for i in 0..len {
        let mut padded = Word::default();

        let to_copy = match i == len - 1 {
            false => 32,
            true => match bytes.len() % 32 {
                0 => 32,
                x => x,
            },
        };

        let offset = 32 * i;
        padded[..to_copy].copy_from_slice(&bytes[offset..offset + to_copy]);
        result.push(padded);
    }
}

#[derive(Debug)]
enum Mediate<'a> {
    // head
    Raw(u32, &'a Token),
    RawArray(Vec<Mediate<'a>>),

    // head + tail
    Prefixed(u32, &'a Token),
    PrefixedArray(Vec<Mediate<'a>>),
    PrefixedArrayWithLength(Vec<Mediate<'a>>),
}

impl Mediate<'_> {
    fn head_len(&self) -> u32 {
        match self {
            Mediate::Raw(len, _) => 32 * len,
            Mediate::RawArray(ref mediates) => {
                mediates.iter().map(|mediate| mediate.head_len()).sum()
            }
            Mediate::Prefixed(_, _)
            | Mediate::PrefixedArray(_)
            | Mediate::PrefixedArrayWithLength(_) => 32,
        }
    }

    fn tail_len(&self) -> u32 {
        match self {
            Mediate::Raw(_, _) | Mediate::RawArray(_) => 0,
            Mediate::Prefixed(len, _) => 32 * len,
            Mediate::PrefixedArray(ref mediates) => mediates
                .iter()
                .fold(0, |acc, m| acc + m.head_len() + m.tail_len()),
            Mediate::PrefixedArrayWithLength(ref mediates) => mediates
                .iter()
                .fold(32, |acc, m| acc + m.head_len() + m.tail_len()),
        }
    }

    fn head_append(&self, acc: &mut Vec<Word>, suffix_offset: u32) {
        match *self {
            Mediate::Raw(_, raw) => encode_token_append(acc, raw),
            Mediate::RawArray(ref raw) => {
                raw.iter().for_each(|mediate| mediate.head_append(acc, 0))
            }
            Mediate::Prefixed(_, _)
            | Mediate::PrefixedArray(_)
            | Mediate::PrefixedArrayWithLength(_) => acc.push(pad_u32(suffix_offset)),
        }
    }

    fn tail_append(&self, acc: &mut Vec<Word>) {
        match *self {
            Mediate::Raw(_, _) | Mediate::RawArray(_) => {}
            Mediate::Prefixed(_, raw) => encode_token_append(acc, raw),
            Mediate::PrefixedArray(ref mediates) => encode_head_tail_append(acc, mediates),
            Mediate::PrefixedArrayWithLength(ref mediates) => {
                // + 32 added to offset represents len of the array prepended to tail
                acc.push(pad_u32(mediates.len() as u32));
                encode_head_tail_append(acc, mediates);
            }
        };
    }
}

/// Encodes vector of tokens into ABI compliant vector of bytes.
pub fn encode(tokens: &[Token]) -> Bytes {
    let mediates = &tokens.iter().map(mediate_token).collect::<Vec<_>>();

    encode_head_tail(mediates)
        .into_iter()
        .flat_map(Into::<[u8; 32]>::into)
        .collect()
}

fn encode_head_tail(mediates: &[Mediate]) -> Vec<Word> {
    let (heads_len, tails_len) = mediates.iter().fold((0, 0), |(head_acc, tail_acc), m| {
        (head_acc + m.head_len(), tail_acc + m.tail_len())
    });

    let mut result = Vec::with_capacity((heads_len + tails_len) as usize);
    encode_head_tail_append(&mut result, mediates);

    result
}

fn encode_head_tail_append(acc: &mut Vec<Word>, mediates: &[Mediate]) {
    let heads_len = mediates
        .iter()
        .fold(0, |head_acc, m| head_acc + m.head_len());

    let mut offset = heads_len;
    for mediate in mediates {
        mediate.head_append(acc, offset);
        offset += mediate.tail_len();
    }

    mediates.iter().for_each(|m| m.tail_append(acc));
}

fn mediate_token(token: &Token) -> Mediate {
    match token {
        Token::Word(_) => Mediate::Raw(1, token),
        Token::FixedSeq(tokens) => {
            let mediates = tokens.iter().map(mediate_token).collect();

            if token.is_dynamic() {
                Mediate::PrefixedArray(mediates)
            } else {
                Mediate::RawArray(mediates)
            }
        }
        Token::DynSeq(tokens) => {
            let mediates = tokens.iter().map(mediate_token).collect();

            Mediate::PrefixedArrayWithLength(mediates)
        }
        Token::PackedSeq(seq) => Mediate::Prefixed(pad_bytes_len(seq), token),
    }
}

fn encode_token_append(data: &mut Vec<Word>, token: &Token) {
    match token {
        Token::Word(word) => data.push(*word),
        Token::PackedSeq(bytes) => pad_bytes_append(data, bytes),
        _ => panic!("Unhandled nested token: {:?}", token),
    };
}

#[cfg(test)]
mod tests {
    use ethers_primitives::B160;
    use hex_literal::hex;

    #[cfg(not(feature = "std"))]
    use crate::no_std_prelude::*;
    use crate::{encode, util::pad_u32, Token, Tokenize, Word};

    #[test]
    fn encode_address() {
        let address = Token::Word(B160([0x11u8; 20]).into());
        let encoded = encode(&[address]);
        let expected = hex!("0000000000000000000000001111111111111111111111111111111111111111");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_dynamic_array_of_addresses() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let addresses = Token::DynSeq(vec![address1, address2]);
        let encoded = encode(&[addresses]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_fixed_array_of_addresses() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let addresses = Token::FixedSeq(vec![address1, address2]);
        let encoded = encode(&[addresses]);
        let expected = hex!(
            "
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_two_addresses() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let encoded = encode(&[address1, address2]);
        let expected = hex!(
            "
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_fixed_array_of_dynamic_array_of_addresses() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let address3 = Token::Word(B160([0x33u8; 20]).into());
        let address4 = Token::Word(B160([0x44u8; 20]).into());
        let array0 = Token::DynSeq(vec![address1, address2]);
        let array1 = Token::DynSeq(vec![address3, address4]);
        let fixed = Token::FixedSeq(vec![array0, array1]);
        let encoded = encode(&[fixed]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000040
			00000000000000000000000000000000000000000000000000000000000000a0
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000003333333333333333333333333333333333333333
			0000000000000000000000004444444444444444444444444444444444444444
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_dynamic_array_of_fixed_array_of_addresses() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let address3 = Token::Word(B160([0x33u8; 20]).into());
        let address4 = Token::Word(B160([0x44u8; 20]).into());
        let array0 = Token::FixedSeq(vec![address1, address2]);
        let array1 = Token::FixedSeq(vec![address3, address4]);
        let dynamic = Token::DynSeq(vec![array0, array1]);
        let encoded = encode(&[dynamic]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
			0000000000000000000000003333333333333333333333333333333333333333
			0000000000000000000000004444444444444444444444444444444444444444
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_dynamic_array_of_dynamic_arrays() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let array0 = Token::DynSeq(vec![address1]);
        let array1 = Token::DynSeq(vec![address2]);
        let dynamic = Token::DynSeq(vec![array0, array1]);
        let encoded = encode(&[dynamic]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000000000000000000000000000000000000000000040
			0000000000000000000000000000000000000000000000000000000000000080
			0000000000000000000000000000000000000000000000000000000000000001
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000000000000000000000000000000000000000000001
			0000000000000000000000002222222222222222222222222222222222222222
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_dynamic_array_of_dynamic_arrays2() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let address3 = Token::Word(B160([0x33u8; 20]).into());
        let address4 = Token::Word(B160([0x44u8; 20]).into());
        let array0 = Token::DynSeq(vec![address1, address2]);
        let array1 = Token::DynSeq(vec![address3, address4]);
        let dynamic = Token::DynSeq(vec![array0, array1]);
        let encoded = encode(&[dynamic]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000000000000000000000000000000000000000000040
			00000000000000000000000000000000000000000000000000000000000000a0
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000003333333333333333333333333333333333333333
			0000000000000000000000004444444444444444444444444444444444444444
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_fixed_array_of_fixed_arrays() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let address3 = Token::Word(B160([0x33u8; 20]).into());
        let address4 = Token::Word(B160([0x44u8; 20]).into());
        let array0 = Token::FixedSeq(vec![address1, address2]);
        let array1 = Token::FixedSeq(vec![address3, address4]);
        let fixed = Token::FixedSeq(vec![array0, array1]);
        let encoded = encode(&[fixed]);
        let expected = hex!(
            "
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
			0000000000000000000000003333333333333333333333333333333333333333
			0000000000000000000000004444444444444444444444444444444444444444
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_fixed_array_of_static_tuples_followed_by_dynamic_type() {
        let tuple1 = Token::FixedSeq(vec![
            Token::Word(pad_u32(93523141)),
            Token::Word(pad_u32(352332135)),
            Token::Word(B160([0x44u8; 20]).into()),
        ]);
        let tuple2 = Token::FixedSeq(vec![
            Token::Word(pad_u32(12411)),
            Token::Word(pad_u32(451)),
            Token::Word(B160([0x22u8; 20]).into()),
        ]);
        let fixed = Token::FixedSeq(vec![tuple1, tuple2]);
        let s = Token::PackedSeq(b"gavofyork".to_vec());
        let encoded = encode(&[fixed, s]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000005930cc5
			0000000000000000000000000000000000000000000000000000000015002967
			0000000000000000000000004444444444444444444444444444444444444444
			000000000000000000000000000000000000000000000000000000000000307b
			00000000000000000000000000000000000000000000000000000000000001c3
			0000000000000000000000002222222222222222222222222222222222222222
			00000000000000000000000000000000000000000000000000000000000000e0
			0000000000000000000000000000000000000000000000000000000000000009
			6761766f66796f726b0000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_empty_array() {
        // Empty arrays
        let encoded = encode(&[Token::DynSeq(vec![]), Token::DynSeq(vec![])]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000040
			0000000000000000000000000000000000000000000000000000000000000060
			0000000000000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);

        // Nested empty arrays
        let encoded = encode(&[
            Token::DynSeq(vec![Token::DynSeq(vec![])]),
            Token::DynSeq(vec![Token::DynSeq(vec![])]),
        ]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000040
			00000000000000000000000000000000000000000000000000000000000000a0
			0000000000000000000000000000000000000000000000000000000000000001
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000001
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_bytes() {
        let bytes = Token::PackedSeq(vec![0x12, 0x34]);
        let encoded = encode(&[bytes]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000002
			1234000000000000000000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_fixed_bytes() {
        let mut word = Word::default();
        word[0..2].copy_from_slice(&[0x12, 0x34]);
        let bytes = Token::Word(word);
        let encoded = encode(&[bytes]);
        let expected = hex!("1234000000000000000000000000000000000000000000000000000000000000");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_string() {
        let s = Token::PackedSeq(b"gavofyork".to_vec());
        let encoded = encode(&[s]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000009
			6761766f66796f726b0000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_bytes2() {
        let bytes = Token::PackedSeq(
            hex!("10000000000000000000000000000000000000000000000000000000000002").to_vec(),
        );
        let encoded = encode(&[bytes]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			000000000000000000000000000000000000000000000000000000000000001f
			1000000000000000000000000000000000000000000000000000000000000200
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_bytes3() {
        let bytes = Token::PackedSeq(
            hex!(
                "
			1000000000000000000000000000000000000000000000000000000000000000
			1000000000000000000000000000000000000000000000000000000000000000
		"
            )
            .to_vec(),
        );
        let encoded = encode(&[bytes]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000040
			1000000000000000000000000000000000000000000000000000000000000000
			1000000000000000000000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_two_bytes() {
        let bytes1 = Token::PackedSeq(
            hex!("10000000000000000000000000000000000000000000000000000000000002").to_vec(),
        );
        let bytes2 = Token::PackedSeq(
            hex!("0010000000000000000000000000000000000000000000000000000000000002").to_vec(),
        );
        let encoded = encode(&[bytes1, bytes2]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000040
			0000000000000000000000000000000000000000000000000000000000000080
			000000000000000000000000000000000000000000000000000000000000001f
			1000000000000000000000000000000000000000000000000000000000000200
			0000000000000000000000000000000000000000000000000000000000000020
			0010000000000000000000000000000000000000000000000000000000000002
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_uint() {
        let mut uint = [0u8; 32];
        uint[31] = 4;
        let encoded = encode(&[Token::Word(uint.into())]);
        let expected = hex!("0000000000000000000000000000000000000000000000000000000000000004");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_int() {
        let mut int = [0u8; 32];
        int[31] = 4;
        let encoded = encode(&[Token::Word(int.into())]);
        let expected = hex!("0000000000000000000000000000000000000000000000000000000000000004");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_bool() {
        let encoded = encode(&[true.to_token()]);
        let expected = hex!("0000000000000000000000000000000000000000000000000000000000000001");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_bool2() {
        let encoded = encode(&[false.to_token()]);
        let expected = hex!("0000000000000000000000000000000000000000000000000000000000000000");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn comprehensive_test() {
        let bytes = hex!(
            "
			131a3afc00d1b1e3461b955e53fc866dcf303b3eb9f4c16f89e388930f48134b
			131a3afc00d1b1e3461b955e53fc866dcf303b3eb9f4c16f89e388930f48134b
		"
        )
        .to_vec();
        let encoded = encode(&[
            Token::Word(pad_u32(5)),
            Token::PackedSeq(bytes.clone()),
            Token::Word(pad_u32(3)),
            Token::PackedSeq(bytes),
        ]);

        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000005
			0000000000000000000000000000000000000000000000000000000000000080
			0000000000000000000000000000000000000000000000000000000000000003
			00000000000000000000000000000000000000000000000000000000000000e0
			0000000000000000000000000000000000000000000000000000000000000040
			131a3afc00d1b1e3461b955e53fc866dcf303b3eb9f4c16f89e388930f48134b
			131a3afc00d1b1e3461b955e53fc866dcf303b3eb9f4c16f89e388930f48134b
			0000000000000000000000000000000000000000000000000000000000000040
			131a3afc00d1b1e3461b955e53fc866dcf303b3eb9f4c16f89e388930f48134b
			131a3afc00d1b1e3461b955e53fc866dcf303b3eb9f4c16f89e388930f48134b
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn test_pad_u32() {
        // this will fail if endianess is not supported
        assert_eq!(pad_u32(0x1)[31], 1);
        assert_eq!(pad_u32(0x100)[30], 1);
    }

    #[test]
    fn comprehensive_test2() {
        let encoded = encode(&vec![
            Token::Word(pad_u32(1)),
            Token::PackedSeq(b"gavofyork".to_vec()),
            Token::Word(pad_u32(2)),
            Token::Word(pad_u32(3)),
            Token::Word(pad_u32(4)),
            Token::DynSeq(vec![
                Token::Word(pad_u32(5)),
                Token::Word(pad_u32(6)),
                Token::Word(pad_u32(7)),
            ]),
        ]);

        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000001
			00000000000000000000000000000000000000000000000000000000000000c0
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000000000000000000000000000000000000000000003
			0000000000000000000000000000000000000000000000000000000000000004
			0000000000000000000000000000000000000000000000000000000000000100
			0000000000000000000000000000000000000000000000000000000000000009
			6761766f66796f726b0000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000003
			0000000000000000000000000000000000000000000000000000000000000005
			0000000000000000000000000000000000000000000000000000000000000006
			0000000000000000000000000000000000000000000000000000000000000007
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_dynamic_array_of_bytes() {
        let bytes =
            hex!("019c80031b20d5e69c8093a571162299032018d913930d93ab320ae5ea44a4218a274f00d607");
        let encoded = encode(&[Token::DynSeq(vec![Token::PackedSeq(bytes.to_vec())])]);

        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000001
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000026
			019c80031b20d5e69c8093a571162299032018d913930d93ab320ae5ea44a421
			8a274f00d6070000000000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_dynamic_array_of_bytes2() {
        let bytes =
            hex!("4444444444444444444444444444444444444444444444444444444444444444444444444444");
        let bytes2 =
            hex!("6666666666666666666666666666666666666666666666666666666666666666666666666666");
        let encoded = encode(&[Token::DynSeq(vec![
            Token::PackedSeq(bytes.to_vec()),
            Token::PackedSeq(bytes2.to_vec()),
        ])]);

        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000000000000000000000000000000000000000000040
			00000000000000000000000000000000000000000000000000000000000000a0
			0000000000000000000000000000000000000000000000000000000000000026
			4444444444444444444444444444444444444444444444444444444444444444
			4444444444440000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000026
			6666666666666666666666666666666666666666666666666666666666666666
			6666666666660000000000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_static_tuple_of_addresses() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let encoded = encode(&[Token::FixedSeq(vec![address1, address2])]);

        let expected = hex!(
            "
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_dynamic_tuple() {
        let string1 = Token::PackedSeq(b"gavofyork".to_vec());
        let string2 = Token::PackedSeq(b"gavofyork".to_vec());
        let tuple = Token::FixedSeq(vec![string1, string2]);
        let encoded = encode(&[tuple]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000040
			0000000000000000000000000000000000000000000000000000000000000080
			0000000000000000000000000000000000000000000000000000000000000009
			6761766f66796f726b0000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000009
			6761766f66796f726b0000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_dynamic_tuple_of_bytes2() {
        let bytes =
            hex!("4444444444444444444444444444444444444444444444444444444444444444444444444444");
        let bytes2 =
            hex!("6666666666666666666666666666666666666666666666666666666666666666666666666666");
        let encoded = encode(&[Token::FixedSeq(vec![
            Token::PackedSeq(bytes.to_vec()),
            Token::PackedSeq(bytes2.to_vec()),
        ])]);

        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000040
			00000000000000000000000000000000000000000000000000000000000000a0
			0000000000000000000000000000000000000000000000000000000000000026
			4444444444444444444444444444444444444444444444444444444444444444
			4444444444440000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000026
			6666666666666666666666666666666666666666666666666666666666666666
			6666666666660000000000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_complex_tuple() {
        let uint = Token::Word([0x11u8; 32].into());
        let string = Token::PackedSeq(b"gavofyork".to_vec());
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let tuple = Token::FixedSeq(vec![uint, string, address1, address2]);
        let encoded = encode(&[tuple]);
        let expected = hex!(
            "
            0000000000000000000000000000000000000000000000000000000000000020
            1111111111111111111111111111111111111111111111111111111111111111
            0000000000000000000000000000000000000000000000000000000000000080
            0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
			0000000000000000000000000000000000000000000000000000000000000009
			6761766f66796f726b0000000000000000000000000000000000000000000000
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_nested_tuple() {
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
        let encoded = encode(&[outer_tuple]);
        let expected = hex!(
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
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_params_containing_dynamic_tuple() {
        let address1 = Token::Word(B160([0x22u8; 20]).into());
        let bool1 = true.to_token();
        let string1 = Token::PackedSeq(b"spaceship".to_vec());
        let string2 = Token::PackedSeq(b"cyborg".to_vec());
        let tuple = Token::FixedSeq(vec![bool1, string1, string2]);
        let address2 = Token::Word(B160([0x33u8; 20]).into());
        let address3 = Token::Word(B160([0x44u8; 20]).into());
        let bool2 = false.to_token();
        let encoded = encode(&[address1, tuple, address2, address3, bool2]);
        let expected = hex!(
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
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_params_containing_static_tuple() {
        let address1 = Token::Word(B160([0x11u8; 20]).into());
        let address2 = Token::Word(B160([0x22u8; 20]).into());
        let bool1 = true.to_token();
        let bool2 = false.to_token();
        let tuple = Token::FixedSeq(vec![address2, bool1, bool2]);
        let address3 = Token::Word(B160([0x33u8; 20]).into());
        let address4 = Token::Word(B160([0x44u8; 20]).into());
        let encoded = encode(&[address1, tuple, address3, address4]);
        let expected = hex!(
            "
			0000000000000000000000001111111111111111111111111111111111111111
			0000000000000000000000002222222222222222222222222222222222222222
			0000000000000000000000000000000000000000000000000000000000000001
			0000000000000000000000000000000000000000000000000000000000000000
			0000000000000000000000003333333333333333333333333333333333333333
			0000000000000000000000004444444444444444444444444444444444444444
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn encode_dynamic_tuple_with_nested_static_tuples() {
        let token = {
            use crate::Token::*;
            FixedSeq(vec![
                FixedSeq(vec![FixedSeq(vec![false.to_token(), Word(pad_u32(0x777))])]),
                DynSeq(vec![Word(pad_u32(0x42)), Word(pad_u32(0x1337))]),
            ])
        };
        let encoded = encode(&[token]);
        let expected = hex!(
            "
			0000000000000000000000000000000000000000000000000000000000000020
			0000000000000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000777
			0000000000000000000000000000000000000000000000000000000000000060
			0000000000000000000000000000000000000000000000000000000000000002
			0000000000000000000000000000000000000000000000000000000000000042
			0000000000000000000000000000000000000000000000000000000000001337
		"
        )
        .to_vec();
        assert_eq!(encoded, expected);
    }
}
