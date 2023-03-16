pub use ethers_abi_derive::SolAbiType;
pub use ethers_abi_enc::*;
pub use ethers_abi_file::*;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn derive() {
        #[derive(SolAbiType)]
        pub struct TupleStruct(u8, u8);
        dbg!(&TupleStruct(2, 5).encode_hex());

        #[derive(SolAbiType)]
        pub struct MyStruct {
            a: u8,
            b: u8,
        }
        dbg!(MyStruct { a: 3, b: 4 }.encode_hex());

        #[derive(SolAbiType)]
        pub struct Aleph {
            a: MyStruct,
            c: &'static str,
            b: TupleStruct,
            #[abi_skip]
            _d: [[u8; 3]; 2],
        }

        let aleph = Aleph {
            a: MyStruct { a: 1, b: 2 },
            c: "adhealskd",
            b: TupleStruct(3, 4),
            _d: [[5, 6, 7], [8, 9, 10]],
        };
        dbg!(aleph.encode_hex());
    }
}
