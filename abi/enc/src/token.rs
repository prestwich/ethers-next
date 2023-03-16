// Copyright 2015-2020 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Ethereum ABI params.

use core::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "std"))]
use crate::no_std_prelude::*;
use crate::{ParamType, Word};

/// Ethereum ABI params.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    /// Single Word
    Word(Word),
    /// Tuple or `T[M]`
    FixedSeq(Vec<Token>),
    /// T[]
    DynSeq(Vec<Token>),
    /// String or Bytes
    PackedSeq(Vec<u8>),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Token::Word(contents) => write!(f, "Word {contents}"),
            Token::FixedSeq(contents) => write!(f, "FixedSeq {contents:?}"),
            Token::DynSeq(contents) => write!(f, "DynSeq {contents:?}"),
            Token::PackedSeq(contents) => write!(f, "PackedSeq {contents:?}"),
        }
    }
}

impl Token {
    /// Return a reference to the underlying word for a value type
    pub fn as_word(&self) -> Option<&Word> {
        match self {
            Token::Word(word) => Some(word),
            _ => None,
        }
    }

    /// Return a reference to the underlying word for a value type
    pub fn as_word_array(&self) -> Option<&[u8; 32]> {
        self.as_word().map(AsRef::as_ref)
    }

    /// Return a reference to the underlying buffer for a packed sequence
    /// (string or bytes)
    pub fn as_packed_data(&self) -> Option<&[u8]> {
        match self {
            Token::PackedSeq(buf) => Some(buf.as_ref()),
            _ => None,
        }
    }

    /// Return a reference to the underlying vector for a dynamic sequence
    pub fn as_dyn_seq(&self) -> Option<&[Token]> {
        match self {
            Token::DynSeq(buf) => Some(buf.as_ref()),
            _ => None,
        }
    }

    /// Return a reference to the underlying vector for a dynamic sequence
    pub fn as_fixed_seq(&self) -> Option<&[Token]> {
        match self {
            Token::FixedSeq(buf) => Some(buf.as_ref()),
            _ => None,
        }
    }

    /// Check whether the type of the token matches the given parameter type.
    ///
    /// Numeric types (`Int` and `Uint`) type check if the size of the token
    /// type is of greater or equal size than the provided parameter type.
    pub fn type_check(&self, param_type: &ParamType) -> bool {
        match param_type {
            ParamType::Address => matches!(self, Self::Word(_)),
            ParamType::Bytes => matches!(self, Self::PackedSeq(_)),
            ParamType::Int(_) => matches!(self, Self::Word(_)),
            ParamType::Uint(_) => matches!(self, Self::Word(_)),
            ParamType::Bool => matches!(self, Self::Word(_)),
            ParamType::String => matches!(self, Self::PackedSeq(_)),
            ParamType::Array(ref inner_param) => match self {
                Self::DynSeq(inner_tokens) => {
                    inner_tokens.iter().all(|t| t.type_check(inner_param))
                }
                _ => false,
            },
            ParamType::FixedBytes(_) => matches!(self, Self::Word(_)),
            ParamType::FixedArray(inner_param, expected_size) => match self {
                Self::FixedSeq(inner_tokens) => {
                    inner_tokens.len() == *expected_size
                        && inner_tokens.iter().all(|t| t.type_check(inner_param))
                }
                _ => false,
            },
            ParamType::Tuple(param_types) => match self {
                Self::FixedSeq(inner_tokens) => {
                    inner_tokens.len() == param_types.len()
                        && inner_tokens
                            .iter()
                            .zip(param_types.iter())
                            .all(|(t, p)| t.type_check(p))
                }
                _ => false,
            },
        }
    }

    /// Check if all the types of the tokens match the given parameter types.
    pub fn types_check(tokens: &[Token], param_types: &[ParamType]) -> bool {
        param_types.len() == tokens.len() && {
            param_types
                .iter()
                .zip(tokens)
                .all(|(param_type, token)| token.type_check(param_type))
        }
    }

    /// Check if the token is a dynamic type resulting in prefixed encoding
    pub fn is_dynamic(&self) -> bool {
        match self {
            Token::DynSeq(_) | Token::PackedSeq(_) => true,
            Token::FixedSeq(tokens) => tokens.iter().any(Token::is_dynamic),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use ethers_primitives::B256;

    #[cfg(not(feature = "std"))]
    use crate::no_std_prelude::*;
    use crate::{ParamType, Token};

    macro_rules! assert_type_check {
        ($left:expr, $right:expr,) => {
            assert!(Token::types_check($left.as_slice(), &$right.as_slice()))
        };
        ($left:expr, $right:expr) => {
            assert_type_check!($left, $right,)
        };
    }

    macro_rules! assert_not_type_check {
        ($left:expr, $right:expr,) => {
            assert!(!Token::types_check($left.as_slice(), &$right.as_slice()))
        };
        ($left:expr, $right:expr) => {
            assert_not_type_check!($left, $right,)
        };
    }

    #[test]
    fn test_type_check() {
        assert_type_check!(
            vec![Token::Word(B256::default()), Token::Word(B256::default())],
            vec![ParamType::Uint(256), ParamType::Bool],
        );
        assert_type_check!(
            vec![Token::Word(B256::default()), Token::Word(B256::default())],
            vec![ParamType::Uint(32), ParamType::Bool],
        );

        assert_not_type_check!(
            vec![Token::Word(B256::default())],
            vec![ParamType::Uint(32), ParamType::Bool],
        );
        assert_not_type_check!(
            vec![Token::Word(B256::default()), Token::Word(B256::default())],
            vec![ParamType::Uint(32)],
        );
        assert_type_check!(
            vec![Token::Word(B256::default()), Token::Word(B256::default())],
            vec![ParamType::Uint(32), ParamType::Bool],
        );

        assert_type_check!(
            vec![Token::DynSeq(vec![
                Token::Word(B256::default()),
                Token::Word(B256::default()),
            ])],
            vec![ParamType::Array(Box::new(ParamType::Bool))],
        );
        assert_type_check!(
            vec![Token::DynSeq(vec![
                Token::Word(B256::default()),
                Token::Word(B256::default()),
            ])],
            vec![ParamType::Array(Box::new(ParamType::Bool))],
        );
        assert_type_check!(
            vec![Token::DynSeq(vec![
                Token::Word(B256::default()),
                Token::Word(B256::default()),
            ])],
            vec![ParamType::Array(Box::new(ParamType::Address))],
        );

        assert_type_check!(
            vec![Token::FixedSeq(vec![
                Token::Word(B256::default()),
                Token::Word(B256::default()),
            ])],
            vec![ParamType::FixedArray(Box::new(ParamType::Bool), 2)],
        );
        assert_not_type_check!(
            vec![Token::FixedSeq(vec![
                Token::Word(B256::default()),
                Token::Word(B256::default()),
            ])],
            vec![ParamType::FixedArray(Box::new(ParamType::Bool), 3)],
        );
        assert_type_check!(
            vec![Token::FixedSeq(vec![
                Token::Word(B256::default()),
                Token::Word(B256::default()),
            ])],
            vec![ParamType::FixedArray(Box::new(ParamType::Bool), 2)],
        );
        assert_type_check!(
            vec![Token::FixedSeq(vec![
                Token::Word(B256::default()),
                Token::Word(B256::default()),
            ])],
            vec![ParamType::FixedArray(Box::new(ParamType::Address), 2)],
        );
    }

    #[test]
    fn test_is_dynamic() {
        assert!(!Token::Word(B256::default()).is_dynamic());
        assert!(Token::PackedSeq(vec![0, 0, 0, 0]).is_dynamic());
        assert!(!Token::Word(B256::default()).is_dynamic());
        assert!(!Token::Word(B256::default()).is_dynamic());
        assert!(!Token::Word(B256::default()).is_dynamic());
        assert!(Token::PackedSeq("".into()).is_dynamic());
        assert!(Token::DynSeq(vec![Token::Word(B256::default())]).is_dynamic());
        assert!(!Token::FixedSeq(vec![Token::Word(B256::default())]).is_dynamic());
        assert!(Token::FixedSeq(vec![Token::PackedSeq("".into())]).is_dynamic());
        assert!(
            Token::FixedSeq(vec![Token::DynSeq(vec![Token::Word(B256::default())])]).is_dynamic()
        );
    }
}
