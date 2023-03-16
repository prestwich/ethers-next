use core::marker::PhantomData;

use ethers_primitives::{B160, B256, U256};

use crate::{Error::InvalidData, Token, Word};

// TODO: recursive typecheck in seqs!!

pub trait SolType {
    type RustType;
    fn sol_type_name() -> std::string::String;
    fn is_dynamic() -> bool;
    fn type_check(token: &Token) -> bool;
    fn detokenize(token: &Token) -> crate::Result<Self::RustType>;
    fn tokenize(_rust: Self::RustType) -> Token;
}

pub struct Address;

impl SolType for Address {
    type RustType = B160;

    fn is_dynamic() -> bool {
        false
    }

    fn sol_type_name() -> std::string::String {
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
}

pub struct Bytes;

impl SolType for Bytes {
    type RustType = Vec<u8>;

    fn is_dynamic() -> bool {
        true
    }

    fn sol_type_name() -> std::string::String {
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
}

macro_rules! impl_int_sol_type {
    ($ity:ty, $bits:literal) => {
        impl SolType for Int<$bits> {
            type RustType = $ity;

            fn is_dynamic() -> bool {
                false
            }

            fn sol_type_name() -> std::string::String {
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

            fn sol_type_name() -> std::string::String {
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
        }
    };

    ($bits:literal) => {
        impl SolType for Uint<$bits> {
            type RustType = U256;

            fn is_dynamic() -> bool {
                false
            }

            fn sol_type_name() -> std::string::String {
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

    fn sol_type_name() -> std::string::String {
        "bool".into()
    }

    fn type_check(token: &Token) -> bool {
        matches!(token, Token::Word(_))
    }

    fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
        match token {
            Token::Word(word) => Ok(word[31] < 2),
            _ => Err(InvalidData),
        }
    }

    fn tokenize(_rust: Self::RustType) -> Token {
        todo!()
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

    fn sol_type_name() -> std::string::String {
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
}

pub struct String;

impl SolType for String {
    type RustType = std::string::String;

    fn is_dynamic() -> bool {
        true
    }

    fn sol_type_name() -> std::string::String {
        "string".to_owned()
    }

    fn type_check(token: &Token) -> bool {
        Bytes::type_check(token)
    }

    fn detokenize(token: &Token) -> crate::Result<Self::RustType> {
        std::string::String::from_utf8(Bytes::detokenize(token)?).map_err(|_| InvalidData)
    }

    fn tokenize(rust: Self::RustType) -> Token {
        Token::PackedSeq(rust.into_bytes())
    }
}

macro_rules! impl_fixed_bytes_sol_type {
    ($bytes:literal) => {
        impl SolType for FixedBytes<$bytes> {

            type RustType = [u8; $bytes];

            fn is_dynamic() -> bool {
                false
            }

            fn sol_type_name() -> std::string::String {
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

    fn sol_type_name() -> std::string::String {
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

            fn sol_type_name() -> std::string::String {
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
                let mut tokens = Vec::with_capacity($num);
                $(
                    tokens[$no] = $ty::tokenize(rust.$no);
                )+
                Token::FixedSeq(tokens)
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
