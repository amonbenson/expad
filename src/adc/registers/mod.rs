pub mod id;
pub mod status;
pub mod mode;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Address {
    Status = 0x00,
    Mode = 0x01,
    Control = 0x02,
    Filter = 0x03,
    Data = 0x04,
    Offset = 0x05,
    Gain = 0x06,
    IoControl = 0x07,
    Test1 = 0x0c,
    Test2 = 0x0d,
    Id = 0x0f,
}

/// A single, byte-wide ADC register.
pub trait RegisterSpec {
    const ADDRESS: Address;

    fn bits(&self) -> u8;
    fn from_bits(bits: u8) -> Self;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Field {
    shift: u8,
    mask: u8,
}

impl Field {
    pub(crate) const fn new(shift: u8, mask: u8) -> Self {
        Self { shift, mask }
    }

    pub(crate) fn get(self, bits: u8) -> u8 {
        (bits >> self.shift) & self.mask
    }

    pub(crate) fn set(self, bits: u8, value: u8) -> u8 {
        (bits & !(self.mask << self.shift)) | ((value & self.mask) << self.shift)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Register(u8);

impl Register {
    pub(crate) const fn new(bits: u8) -> Self {
        Self(bits)
    }

    pub(crate) fn bits(self) -> u8 {
        self.0
    }

    pub(crate) fn field(self, field: Field) -> u8 {
        field.get(self.0)
    }

    pub(crate) fn bit(self, field: Field) -> bool {
        self.field(field) != 0
    }

    pub(crate) fn set_field(&mut self, field: Field, value: u8) {
        self.0 = field.set(self.0, value);
    }

    pub(crate) fn set_bit(&mut self, field: Field, value: bool) {
        self.set_field(field, value as u8);
    }
}
