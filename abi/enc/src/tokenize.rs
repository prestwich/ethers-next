use crate::{decode, encode, ParamType, Token, Word};
use ethers_primitives::{B160, B256, U128, U256, U64};

/// Tokenize a struct
pub trait Tokenize {
    /// Convert to tokens
    fn to_token(&self) -> Token;

    /// ABI encode
    fn encode(&self) -> Vec<u8> {
        encode(&[self.to_token()])
    }

    /// Hex encode
    fn encode_hex(&self) -> String {
        hex::encode(self.encode())
    }

    /// Hex with selector
    fn encode_hex_with_selector(&self, selector: [u8; 4]) -> String {
        hex::encode(self.encode_with_selector(selector))
    }

    /// ABI encode with a selector
    fn encode_with_selector(&self, selector: [u8; 4]) -> Vec<u8> {
        let mut v = Vec::from(selector);
        v.extend(self.encode());
        v
    }
}

pub trait Detokenize: Sized {
    fn params() -> &'static [ParamType];

    fn from_tokens(token: Vec<Token>) -> crate::Result<Self>;

    fn decode(buf: &[u8]) -> crate::Result<Self> {
        Self::from_tokens(decode(Self::params(), buf)?)
    }
}

macro_rules! impl_tokenize_ints {
    ($int:ty, $uint:ty) => {
        impl Tokenize for $int {
            fn to_token(&self) -> Token {
                (*self as $uint).to_token()
            }
        }

        impl Tokenize for $uint {
            fn to_token(&self) -> Token {
                Token::Word(B256::from(U256::from(*self)))
            }
        }
    };
}

impl_tokenize_ints!(i8, u8);
impl_tokenize_ints!(i16, u16);
impl_tokenize_ints!(i32, u32);
impl_tokenize_ints!(i64, u64);
impl_tokenize_ints!(isize, usize);

impl Tokenize for &str {
    fn to_token(&self) -> Token {
        Token::PackedSeq(self.as_bytes().to_vec())
    }
}

impl Tokenize for String {
    fn to_token(&self) -> Token {
        Token::PackedSeq(self.as_bytes().to_vec())
    }
}

impl<T, const N: usize> Tokenize for [T; N]
where
    T: Tokenize,
{
    fn to_token(&self) -> Token {
        Token::FixedSeq(self.iter().map(Tokenize::to_token).collect())
    }
}

impl Tokenize for bool {
    fn to_token(&self) -> Token {
        let mut word = Word::default();
        word[31..].copy_from_slice(&[*self as u8]);
        Token::Word(word)
    }
}

impl Tokenize for B160 {
    fn to_token(&self) -> Token {
        let mut word = Word::default();
        word[12..].copy_from_slice(&self[..]);

        Token::Word(word)
    }
}

impl Tokenize for B256 {
    fn to_token(&self) -> Token {
        Token::Word(*self)
    }
}

impl Tokenize for U64 {
    fn to_token(&self) -> Token {
        U256::from(*self).to_token()
    }
}

impl Tokenize for U128 {
    fn to_token(&self) -> Token {
        let mut word = Word::default();
        word[16..].copy_from_slice(&self.to_be_bytes::<16>());
        word.to_token()
    }
}

impl Tokenize for U256 {
    fn to_token(&self) -> Token {
        B256(self.to_be_bytes::<32>()).to_token()
    }
}
