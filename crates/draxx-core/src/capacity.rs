use crate::{BundleStatus, CorrectionAction, DomainError};

pub const MAX_BUNDLE_MEASUREMENTS: usize = 42;

/// Authoritative effective (not historical row) count, obtained under a write lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BundleCapacity(usize);

impl BundleCapacity {
    pub fn new(active: usize) -> Result<Self, DomainError> {
        if active > MAX_BUNDLE_MEASUREMENTS {
            return Err(DomainError::BundleFull);
        }
        Ok(Self(active))
    }
    pub const fn active(self) -> usize {
        self.0
    }
    pub fn ensure_can_append(self) -> Result<(), DomainError> {
        if self.0 == MAX_BUNDLE_MEASUREMENTS {
            return Err(DomainError::BundleFull);
        }
        Ok(())
    }
    pub fn ensure_can_close(self) -> Result<(), DomainError> {
        if self.0 == 0 {
            return Err(DomainError::EmptyBundle);
        }
        Ok(())
    }
    pub fn after_correction(
        self,
        was_active: bool,
        action: &CorrectionAction,
        status: BundleStatus,
    ) -> Result<Self, DomainError> {
        let active = match (was_active, action) {
            (false, CorrectionAction::Replace { .. }) => {
                self.ensure_can_append()?;
                self.0 + 1
            }
            (true, CorrectionAction::Invalidate) => {
                self.0.checked_sub(1).ok_or(DomainError::EmptyBundle)?
            }
            _ => self.0,
        };
        let next = Self::new(active)?;
        if status == BundleStatus::Closed {
            next.ensure_can_close()?;
        }
        Ok(next)
    }
}
