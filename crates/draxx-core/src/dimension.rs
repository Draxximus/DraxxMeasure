use std::fmt;

use crate::DomainError;

pub const NANOMETRES_PER_MILLIMETRE: i64 = 1_000_000;

/// Phase 1 supports millimetres only; conversions must be explicit future policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unit {
    Millimetre,
}

/// Nonnegative exact length, stored in signed SQLite-compatible nanometres.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Length(i64);

impl Length {
    pub fn from_nanometres(value: i64) -> Result<Self, DomainError> {
        if value < 0 {
            return Err(DomainError::NegativeLength);
        }
        Ok(Self(value))
    }

    /// Parse `[0-9]+(\.[0-9]{1,6})?` without whitespace, exponent, sign, or rounding.
    pub fn parse_mm(text: &str) -> Result<Self, DomainError> {
        let (whole, fraction) = match text.split_once('.') {
            Some((whole, fraction)) => {
                if fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(DomainError::InvalidDecimal);
                }
                (whole, fraction)
            }
            None => (text, ""),
        };
        if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DomainError::InvalidDecimal);
        }
        if fraction.len() > 6 {
            return Err(DomainError::PrecisionExceeded);
        }
        let whole = whole
            .parse::<i64>()
            .map_err(|_| DomainError::NumericOverflow)?;
        let mut fractional_nm = 0_i64;
        for byte in fraction.bytes() {
            fractional_nm = fractional_nm * 10 + i64::from(byte - b'0');
        }
        for _ in fraction.len()..6 {
            fractional_nm *= 10;
        }
        let value = whole
            .checked_mul(NANOMETRES_PER_MILLIMETRE)
            .and_then(|v| v.checked_add(fractional_nm))
            .ok_or(DomainError::NumericOverflow)?;
        Ok(Self(value))
    }

    pub const fn nanometres(self) -> i64 {
        self.0
    }

    /// Signed deviation is a quantity, not a nonnegative Length.
    pub const fn deviation_from(self, nominal: Self) -> i64 {
        self.0 - nominal.0
    }
}

impl fmt::Display for Length {
    /// Canonical exact millimetres; raw operator spelling remains in MeasurementInput.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.0 / NANOMETRES_PER_MILLIMETRE;
        let fraction = self.0 % NANOMETRES_PER_MILLIMETRE;
        if fraction == 0 {
            write!(f, "{whole}")
        } else {
            let fraction = format!("{fraction:06}");
            write!(f, "{whole}.{}", fraction.trim_end_matches('0'))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Resolution(Length);

impl Resolution {
    pub fn new(value: Length) -> Result<Self, DomainError> {
        if value.nanometres() == 0 {
            return Err(DomainError::InvalidResolution);
        }
        Ok(Self(value))
    }

    pub const fn length(self) -> Length {
        self.0
    }

    pub fn validate(self, value: Length) -> Result<(), DomainError> {
        if value.nanometres() % self.0.nanometres() != 0 {
            return Err(DomainError::ResolutionMismatch);
        }
        Ok(())
    }
}
