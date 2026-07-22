use std::{
    array,
    io::{Cursor, Error, Read, Result},
    str::FromStr,
};

use uuid::Uuid;

use crate::{
    Identifier,
    codec::VarInt,
    serial::{PrefixedRead, ReadFrom},
};

impl ReadFrom for bool {
    fn read(data: &mut Cursor<&[u8]>) -> Result<Self> {
        let byte = u8::read(data)?;
        Ok(byte != 0)
    }
}

/// Implements `ReadFrom` for fixed-width primitives by reading their
/// big-endian byte representation.
macro_rules! impl_read_be_bytes {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ReadFrom for $ty {
                fn read(data: &mut Cursor<&[u8]>) -> Result<Self> {
                    let mut buf = [0; size_of::<Self>()];
                    data.read_exact(&mut buf)?;
                    Ok(Self::from_be_bytes(buf))
                }
            }
        )+
    };
}

impl_read_be_bytes!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);

impl<T: ReadFrom> ReadFrom for Option<T> {
    fn read(data: &mut Cursor<&[u8]>) -> Result<Self> {
        if bool::read(data)? {
            Ok(Some(T::read(data)?))
        } else {
            Ok(None)
        }
    }
}

impl<T: ReadFrom, const N: usize> ReadFrom for [T; N] {
    fn read(data: &mut Cursor<&[u8]>) -> Result<Self> {
        array::try_from_fn(|_| T::read(data))
    }
}

impl ReadFrom for Uuid {
    fn read(data: &mut Cursor<&[u8]>) -> Result<Self> {
        let most_significant_bits = u64::read(data)?;
        let least_significant_bits = u64::read(data)?;

        Ok(Uuid::from_u64_pair(
            most_significant_bits,
            least_significant_bits,
        ))
    }
}

impl ReadFrom for Identifier {
    fn read(data: &mut Cursor<&[u8]>) -> Result<Self> {
        Identifier::from_str(&String::read_prefixed::<VarInt>(data)?).map_err(Error::other)
    }
}
