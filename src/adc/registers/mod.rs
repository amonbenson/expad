use core::fmt;

pub mod status;
pub mod mode;
pub mod control;
pub mod filter;
pub mod data;
pub mod id;

/// An ADC register, `WIDTH` bytes wide on the wire (MSB first).
pub trait Register: Sized {
    const ADDRESS: u8;
    const WIDTH: usize;

    fn bits(&self) -> u32;
    fn from_bits(bits: u32) -> Self;
}

/// A register newtype generic over its wire address/width, with bitfield
/// helpers shared by every concrete register, e.g.
/// `pub type Id = RegisterValue<0x0f, 1>;`.
#[derive(Clone, Copy, Default)]
pub(crate) struct RegisterValue<const ADDRESS: u8, const WIDTH: usize>(u32);

impl<const ADDRESS: u8, const WIDTH: usize> fmt::Debug for RegisterValue<ADDRESS, WIDTH> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Register").field(&self.0).finish()
    }
}

impl<const ADDRESS: u8, const WIDTH: usize> Register for RegisterValue<ADDRESS, WIDTH> {
    const ADDRESS: u8 = ADDRESS;
    const WIDTH: usize = WIDTH;

    fn bits(&self) -> u32 {
        self.0
    }

    fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
}

impl<const ADDRESS: u8, const WIDTH: usize> RegisterValue<ADDRESS, WIDTH> {
    /// Reads a single bit, `true` when the wire bit is set.
    fn bit(&self, shift: u32) -> bool {
        (self.0 >> shift) & 1 != 0
    }

    /// Writes a single bit.
    fn set_bit(&mut self, shift: u32, value: bool) {
        self.0 = (self.0 & !(1 << shift)) | ((value as u32) << shift);
    }

    /// Reads a single bit that is active low on the wire (0 means "true").
    fn bit_active_low(&self, shift: u32) -> bool {
        (self.0 >> shift) & 1 == 0
    }

    /// Writes a single bit that is active low on the wire.
    fn set_bit_active_low(&mut self, shift: u32, value: bool) {
        self.0 = (self.0 & !(1 << shift)) | ((!value as u32) << shift);
    }

    /// Reads a multi-bit field, decoded through `T`'s `TryFrom<u8>` impl
    /// (typically derived via `num_enum::TryFromPrimitive`).
    fn field<T>(&self, shift: u32, width: u32) -> T
    where
        T: TryFrom<u8>,
        T::Error: fmt::Debug,
    {
        let mask = (1u32 << width) - 1;
        T::try_from(((self.0 >> shift) & mask) as u8).unwrap()
    }

    /// Writes a multi-bit field, encoded through `T`'s `Into<u8>` impl
    /// (typically derived via `num_enum::IntoPrimitive`).
    fn set_field<T>(&mut self, shift: u32, width: u32, value: T)
    where
        u8: From<T>,
    {
        let mask = (1u32 << width) - 1;
        self.0 = (self.0 & !(mask << shift)) | ((u8::from(value) as u32 & mask) << shift);
    }
}
