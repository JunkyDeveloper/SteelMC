use crate::serial::{ReadFrom, WriteTo};
use glam::{DVec3, IVec2, IVec3, Vec3};
use std::io::{Cursor, Result, Write};

/// Implements `WriteTo`/`ReadFrom` for a 2-component glam vector by writing and
/// reading its `x`/`y` fields in order using the component codec `$comp`.
macro_rules! impl_vec2_codec {
    ($ty:ty, $comp:ty) => {
        impl WriteTo for $ty {
            fn write(&self, writer: &mut impl Write) -> Result<()> {
                self.x.write(writer)?;
                self.y.write(writer)
            }
        }

        impl ReadFrom for $ty {
            fn read(data: &mut Cursor<&[u8]>) -> Result<Self> {
                Ok(Self {
                    x: <$comp>::read(data)?,
                    y: <$comp>::read(data)?,
                })
            }
        }
    };
}

/// Implements `WriteTo`/`ReadFrom` for a 3-component glam vector by writing and
/// reading its `x`/`y`/`z` fields in order using the component codec `$comp`.
macro_rules! impl_vec3_codec {
    ($ty:ty, $comp:ty) => {
        impl WriteTo for $ty {
            fn write(&self, writer: &mut impl Write) -> Result<()> {
                self.x.write(writer)?;
                self.y.write(writer)?;
                self.z.write(writer)
            }
        }

        impl ReadFrom for $ty {
            fn read(data: &mut Cursor<&[u8]>) -> Result<Self> {
                Ok(Self {
                    x: <$comp>::read(data)?,
                    y: <$comp>::read(data)?,
                    z: <$comp>::read(data)?,
                })
            }
        }
    };
}

impl_vec2_codec!(IVec2, i32);
impl_vec3_codec!(IVec3, i32);
impl_vec3_codec!(DVec3, f64);
impl_vec3_codec!(Vec3, f32);
