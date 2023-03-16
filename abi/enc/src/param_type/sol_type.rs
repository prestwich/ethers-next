use core::marker::PhantomData;

use ethers_primitives::{B160, B256, U256};

use std::string::String as RustString;

use crate::{decoder::*, Error::InvalidData, Token, Word};

pub trait SolType {
    type RustType;
    fn sol_type_name() -> RustString;
    fn is_dynamic() -> bool;
    fn type_check(token: &Token) -> bool;
    fn detokenize(token: &Token) -> crate::Result<Self::RustType>;
    fn tokenize(rust: Self::RustType) -> Token;

    #[doc(hidden)]
    fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult>;

    fn encode(rust: Self::RustType) -> Vec<u8> {
        let token = Self::tokenize(rust);
        crate::encode(&[token])
    }

    fn hex_encode(rust: Self::RustType) -> RustString {
        format!("0x{}", hex::encode(Self::encode(rust)))
    }

    fn decode(data: &[u8]) -> crate::Result<Self::RustType> {
        Self::detokenize(&Self::read_token(data, 0)?.token)
    }

    fn hex_decode(data: &str) -> crate::Result<Self::RustType> {
        let payload = data.strip_prefix("0x").unwrap_or(data);
        hex::decode(payload)
            .map_err(|_| InvalidData)
            .and_then(|buf| Self::decode(&buf))
    }
}

pub struct Address;

impl SolType for Address {
    type RustType = B160;

    fn is_dynamic() -> bool {
        false
    }

    fn sol_type_name() -> RustString {
        "address".to_string()
    }

    fn type_check(token: &Token) -> bool {
        matches!(token, Token::Word(_))
    }

    fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
        token
            .as_word_array()
            .map(|arr| &arr[12..])
            .map(B160::from_slice)
            .ok_or(InvalidData)
    }

    fn tokenize(rust: Self::RustType) -> Token {
        let mut word = Word::default();
        word[12..].copy_from_slice(&rust[..]);
        Token::Word(word)
    }

    fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult> {
        let slice = peek_32_bytes(data, offset)?;
        let result = DecodeResult {
            token: Token::Word(slice),
            new_offset: offset + 32,
        };
        if !Self::type_check(&result.token) {
            return Err(InvalidData);
        }
        Ok(result)
    }
}

pub struct Bytes;

impl SolType for Bytes {
    type RustType = Vec<u8>;

    fn is_dynamic() -> bool {
        true
    }

    fn sol_type_name() -> RustString {
        "bytes".to_string()
    }

    fn type_check(token: &Token) -> bool {
        matches!(token, Token::PackedSeq(_))
    }

    fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
        token
            .as_packed_data()
            .map(<[u8]>::to_vec)
            .ok_or(InvalidData)
    }

    fn tokenize(rust: Self::RustType) -> Token {
        Token::PackedSeq(rust)
    }

    fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult> {
        let dynamic_offset = as_usize(&peek_32_bytes(data, offset)?)?;
        let len = as_usize(&peek_32_bytes(data, dynamic_offset)?)?;
        let bytes = take_bytes(data, dynamic_offset + 32, len, true)?;
        let result = DecodeResult {
            token: Token::PackedSeq(bytes),
            new_offset: offset + 32,
        };
        Ok(result)
    }
}

macro_rules! impl_int_sol_type {
    ($ity:ty, $bits:literal) => {
        impl SolType for Int<$bits> {
            type RustType = $ity;

            fn is_dynamic() -> bool {
                false
            }

            fn sol_type_name() -> RustString {
                format!("int{}", $bits)
            }

            fn type_check(token: &Token) -> bool {
                matches!(token, Token::Word(_))
            }

            fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
                let bytes = (<$ity>::BITS / 8) as usize;
                token
                    .as_word_array()
                    .map(|arr| &arr[32 - bytes..])
                    .map(|sli| <$ity>::from_be_bytes(sli.try_into().unwrap()))
                    .ok_or(InvalidData)
            }

            fn tokenize(rust: Self::RustType) -> Token {
                let bytes = (<$ity>::BITS / 8) as usize;
                let mut word = if rust < 0 {
                    // account for negative
                    Word::repeat_byte(0xff)
                } else {
                    Word::default()
                };
                let slice = rust.to_be_bytes();
                word[32 - bytes..].copy_from_slice(&slice);
                Token::Word(word)
            }

            fn read_token(
                data: &[u8],
                offset: usize,
            ) -> crate::Result<crate::decoder::DecodeResult> {
                let slice = peek_32_bytes(data, offset)?;
                let result = DecodeResult {
                    token: Token::Word(slice),
                    new_offset: offset + 32,
                };
                if !Self::type_check(&result.token) {
                    return Err(InvalidData);
                }
                Ok(result)
            }
        }
    };
}

pub struct Int<const BITS: usize>;
impl_int_sol_type!(i8, 8);
impl_int_sol_type!(i16, 16);
impl_int_sol_type!(i32, 24);
impl_int_sol_type!(i32, 32);
impl_int_sol_type!(i64, 40);
impl_int_sol_type!(i64, 48);
impl_int_sol_type!(i64, 56);
impl_int_sol_type!(i64, 64);
// TODO: larger

macro_rules! impl_uint_sol_type {
    ($uty:ty, $bits:literal) => {
        impl SolType for Uint<$bits> {
            type RustType = $uty;

            fn is_dynamic() -> bool {
                false
            }

            fn sol_type_name() -> RustString {
                format!("uint{}", $bits)
            }

            fn type_check(token: &Token) -> bool {
                matches!(token, Token::Word(_))
            }

            fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
                let bytes = (<$uty>::BITS / 8) as usize;
                token
                    .as_word_array()
                    .map(|arr| &arr[32 - bytes..])
                    .map(|sli| <$uty>::from_be_bytes(sli.try_into().unwrap()))
                    .ok_or(InvalidData)
            }

            fn tokenize(rust: Self::RustType) -> Token {
                let bytes = (<$uty>::BITS / 8) as usize;
                let mut word = Word::default();
                let slice = rust.to_be_bytes();
                word[32 - bytes..].copy_from_slice(&slice);
                Token::Word(word)
            }

            fn read_token(
                data: &[u8],
                offset: usize,
            ) -> crate::Result<crate::decoder::DecodeResult> {
                let slice = peek_32_bytes(data, offset)?;
                let result = DecodeResult {
                    token: Token::Word(slice),
                    new_offset: offset + 32,
                };
                if !Self::type_check(&result.token) {
                    return Err(InvalidData);
                }
                Ok(result)
            }
        }
    };

    ($bits:literal) => {
        impl SolType for Uint<$bits> {
            type RustType = U256;

            fn is_dynamic() -> bool {
                false
            }

            fn sol_type_name() -> RustString {
                format!("uint{}", $bits)
            }

            fn type_check(token: &Token) -> bool {
                matches!(token, Token::Word(_))
            }

            fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
                token
                    .as_word_array()
                    .map(|word| U256::from_be_bytes::<32>(*word))
                    .ok_or(InvalidData)
            }

            fn tokenize(rust: Self::RustType) -> Token {
                Token::Word(B256(rust.to_be_bytes::<32>()))
            }

            fn read_token(
                data: &[u8],
                offset: usize,
            ) -> crate::Result<crate::decoder::DecodeResult> {
                let slice = peek_32_bytes(data, offset)?;
                let result = DecodeResult {
                    token: Token::Word(slice),
                    new_offset: offset + 32,
                };
                if !Self::type_check(&result.token) {
                    return Err(InvalidData);
                }
                Ok(result)
            }
        }
    };

    ($($bits:literal,)+) => {
        $(
            impl_uint_sol_type!($bits);
        )+
    }
}

pub struct Uint<const BITS: usize>;
impl_uint_sol_type!(u8, 8);
impl_uint_sol_type!(u16, 16);
impl_uint_sol_type!(u32, 24);
impl_uint_sol_type!(u32, 32);
impl_uint_sol_type!(u64, 40);
impl_uint_sol_type!(u64, 48);
impl_uint_sol_type!(u64, 56);
impl_uint_sol_type!(u64, 64);
impl_uint_sol_type!(
    72, 80, 88, 96, 104, 112, 120, 128, 136, 144, 152, 160, 168, 176, 184, 192, 200, 208, 216, 224,
    232, 240, 248, 256,
);

pub struct Bool;
impl SolType for Bool {
    type RustType = bool;

    fn is_dynamic() -> bool {
        false
    }

    fn sol_type_name() -> RustString {
        "bool".into()
    }

    fn type_check(token: &Token) -> bool {
        match token {
            Token::Word(word) => check_bool(*word).is_ok(),
            _ => false,
        }
    }

    fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
        match token {
            Token::Word(word) => Ok(word[31] < 2),
            _ => Err(InvalidData),
        }
    }

    fn tokenize(rust: Self::RustType) -> Token {
        let mut word = Word::default();
        word[31..32].copy_from_slice(&[rust as u8]);
        Token::Word(word)
    }

    fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult> {
        let slice = peek_32_bytes(data, offset)?;
        let result = DecodeResult {
            token: Token::Word(slice),
            new_offset: offset + 32,
        };
        if !Self::type_check(&result.token) {
            return Err(InvalidData);
        }
        Ok(result)
    }
}

pub struct Array<T: SolType>(PhantomData<T>);

impl<T> SolType for Array<T>
where
    T: SolType,
{
    type RustType = Vec<T::RustType>;

    fn is_dynamic() -> bool {
        true
    }

    fn sol_type_name() -> RustString {
        format!("{}[]", T::sol_type_name())
    }

    fn type_check(token: &Token) -> bool {
        matches!(token, Token::DynSeq(_))
    }

    fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
        if let Token::DynSeq(tokens) = token {
            tokens.iter().map(T::detokenize).collect()
        } else {
            Err(InvalidData)
        }
    }

    fn tokenize(rust: Self::RustType) -> Token {
        Token::DynSeq(rust.into_iter().map(|r| T::tokenize(r)).collect())
    }

    fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult> {
        let len_offset = as_usize(&peek_32_bytes(data, offset)?)?;
        let len = as_usize(&peek_32_bytes(data, len_offset)?)?;

        let tail_offset = len_offset + 32;
        let tail = &data[tail_offset..];

        let mut tokens = vec![];
        let mut new_offset = 0;

        for _ in 0..len {
            let res = T::read_token(tail, new_offset)?;
            new_offset = res.new_offset;
            tokens.push(res.token);
        }

        let result = DecodeResult {
            token: Token::DynSeq(tokens),
            new_offset: offset + 32,
        };

        Ok(result)
    }
}

pub struct String;

impl SolType for String {
    type RustType = RustString;

    fn is_dynamic() -> bool {
        true
    }

    fn sol_type_name() -> RustString {
        "string".to_owned()
    }

    fn type_check(token: &Token) -> bool {
        match token {
            Token::PackedSeq(bytes) => std::str::from_utf8(bytes).is_ok(),
            _ => false,
        }
    }

    fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
        RustString::from_utf8(Bytes::detokenize(token)?).map_err(|_| InvalidData)
    }

    fn tokenize(rust: Self::RustType) -> Token {
        Token::PackedSeq(rust.into_bytes())
    }

    fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult> {
        let dynamic_offset = as_usize(&peek_32_bytes(data, offset)?)?;
        let len = as_usize(&peek_32_bytes(data, dynamic_offset)?)?;
        let bytes = take_bytes(data, dynamic_offset + 32, len, true)?;
        let result = DecodeResult {
            token: Token::PackedSeq(bytes),
            new_offset: offset + 32,
        };
        Ok(result)
    }
}

macro_rules! impl_fixed_bytes_sol_type {
    ($bytes:literal) => {
        impl SolType for FixedBytes<$bytes> {

            type RustType = [u8; $bytes];

            fn is_dynamic() -> bool {
                false
            }

            fn sol_type_name() -> RustString {
                format!("bytes{}", $bytes)
            }

            fn type_check(token: &Token) -> bool {
                matches!(token, Token::Word(_))
            }

            fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
                let word = token
                    .as_word_array()
                    .ok_or(InvalidData)?;
                let mut res = Self::RustType::default();
                res[..$bytes].copy_from_slice(&word[..$bytes]);
                Ok(res)
            }

            fn tokenize(rust: Self::RustType) -> Token {
                let mut word = Word::default();
                word[..$bytes].copy_from_slice(&rust[..]);
                Token::Word(word)
            }

            fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult> {
                let word = peek_32_bytes(data, offset)?;
                check_fixed_bytes(word, $bytes)?;

                let result = DecodeResult {
                    token: Token::Word(word),
                    new_offset: offset + 32,
                };
                Ok(result)
            }
        }
    };

    ($($bytes:literal,)+) => {
        $(impl_fixed_bytes_sol_type!($bytes);)+
    };
}

pub struct FixedBytes<const N: usize>;
impl_fixed_bytes_sol_type!(
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32,
);

pub struct FixedArray<T, const N: usize>(PhantomData<T>);

impl<T, const N: usize> SolType for FixedArray<T, N>
where
    T: SolType,
{
    type RustType = [T::RustType; N];

    fn is_dynamic() -> bool {
        T::is_dynamic()
    }

    fn sol_type_name() -> RustString {
        format!("{}[{}]", T::sol_type_name(), N)
    }

    fn type_check(token: &Token) -> bool {
        match token {
            Token::FixedSeq(tokens) => {
                tokens.len() == N && tokens.iter().all(|token| T::type_check(token))
            }
            _ => false,
        }
    }

    fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
        token
            .as_fixed_seq()
            .ok_or(InvalidData)?
            .iter()
            .map(|t| T::detokenize(t))
            .collect::<crate::Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| InvalidData)
    }

    fn tokenize(rust: Self::RustType) -> Token {
        Token::FixedSeq(rust.into_iter().map(|r| T::tokenize(r)).collect())
    }

    fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult> {
        let is_dynamic = Self::is_dynamic();

        let (tail, mut new_offset) = if is_dynamic {
            let offset = as_usize(&peek_32_bytes(data, offset)?)?;
            if offset > data.len() {
                return Err(InvalidData);
            }
            (&data[offset..], 0)
        } else {
            (data, offset)
        };

        let mut tokens = Vec::with_capacity(N);

        for _ in 0..N {
            let res = T::read_token(tail, new_offset)?;
            new_offset = res.new_offset;
            tokens.push(res.token);
        }

        let result = DecodeResult {
            token: Token::FixedSeq(tokens),
            new_offset: if is_dynamic { offset + 32 } else { new_offset },
        };

        Ok(result)
    }
}

macro_rules! impl_tuple_sol_type {
    ($num:expr, $( $ty:ident : $no:tt ),+ $(,)?) => {
        impl<$($ty,)+> SolType for ($( $ty, )+)
        where
            $(
                $ty: SolType,
            )+
        {
            type RustType = ($( $ty::RustType, )+);

            fn is_dynamic() -> bool {
                $(
                    if $ty::is_dynamic() {
                        return true;
                    }
                )+
                false
            }

            fn sol_type_name() -> RustString {
                let mut types = Vec::with_capacity($num);
                $(
                    types.push($ty::sol_type_name());
                )+

                format!("({})", types.join(","))

            }

            fn type_check(token: &Token) -> bool {
                match token {
                    Token::FixedSeq(tokens) => {
                        if tokens.len() != $num {
                            return false
                        }
                        $(
                            if !$ty::type_check(&tokens[$no]) {
                                return false
                            }
                        )+
                        true
                    },
                    _ => false
                }
            }

            fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
                if !Self::type_check(token) {
                    return Err(InvalidData)
                }
                let mut tokens = token.as_fixed_seq().ok_or(InvalidData)?.iter();

                Ok((
                    $(
                        $ty::detokenize(tokens.next().unwrap())?,
                    )+
                ))
            }

            fn tokenize(rust: Self::RustType) -> Token {
                let tokens = vec![
                    $(
                        $ty::tokenize(rust.$no),
                    )+
                ];
                Token::FixedSeq(tokens)
            }

            fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult> {
                let is_dynamic = Self::is_dynamic();

                // The first element in a dynamic Tuple is an offset to the Tuple's data
                // For a static Tuple the data begins right away
                let (tail, mut new_offset) = if is_dynamic {
                    let offset = as_usize(&peek_32_bytes(data, offset)?)?;
                    if offset > data.len() {
                        return Err(InvalidData);
                    }
                    (&data[offset..], 0)
                } else {
                    (data, offset)
                };

                let mut tokens = Vec::with_capacity($num);
                $(
                    let res = $ty::read_token(tail, new_offset)?;
                    new_offset = res.new_offset;
                    tokens.push(res.token);
                )+

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
    };
}
impl_tuple_sol_type!(1, A:0, );
impl_tuple_sol_type!(2, A:0, B:1, );
impl_tuple_sol_type!(3, A:0, B:1, C:2, );
impl_tuple_sol_type!(4, A:0, B:1, C:2, D:3, );
impl_tuple_sol_type!(5, A:0, B:1, C:2, D:3, E:4, );
impl_tuple_sol_type!(6, A:0, B:1, C:2, D:3, E:4, F:5, );
impl_tuple_sol_type!(7, A:0, B:1, C:2, D:3, E:4, F:5, G:6, );
impl_tuple_sol_type!(8, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, );
impl_tuple_sol_type!(9, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, );
impl_tuple_sol_type!(10, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, );
impl_tuple_sol_type!(11, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, );
impl_tuple_sol_type!(12, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, );
impl_tuple_sol_type!(13, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, );
impl_tuple_sol_type!(14, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, );
impl_tuple_sol_type!(15, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, );
impl_tuple_sol_type!(16, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, );
impl_tuple_sol_type!(17, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16,);
impl_tuple_sol_type!(18, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17,);
impl_tuple_sol_type!(19, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18,);
impl_tuple_sol_type!(20, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18, T:19,);
impl_tuple_sol_type!(21, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18, T:19, U:20,);

pub struct Function;

impl SolType for Function {
    type RustType = (B160, [u8; 4]);

    fn sol_type_name() -> RustString {
        "function".to_string()
    }

    fn is_dynamic() -> bool {
        false
    }

    fn type_check(token: &Token) -> bool {
        match token {
            Token::Word(word) => crate::decoder::check_fixed_bytes(*word, 24).is_ok(),
            _ => false,
        }
    }

    fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
        if !Self::type_check(token) {
            return Err(InvalidData);
        }
        let t = token.as_word_array().unwrap();

        let mut address = [0u8; 20];
        let mut selector = [0u8; 4];
        address.copy_from_slice(&t[..20]);
        selector.copy_from_slice(&t[20..24]);
        Ok((B160(address), selector))
    }

    fn tokenize(rust: Self::RustType) -> Token {
        let mut word = Word::default();
        word[..20].copy_from_slice(&rust.0[..]);
        word[20..24].copy_from_slice(&rust.1[..]);
        Token::Word(word)
    }

    fn read_token(data: &[u8], offset: usize) -> crate::Result<crate::decoder::DecodeResult> {
        let word = peek_32_bytes(data, offset)?;
        check_fixed_bytes(word, 24)?;

        let result = DecodeResult {
            token: Token::Word(word),
            new_offset: offset + 32,
        };
        Ok(result)
    }
}
