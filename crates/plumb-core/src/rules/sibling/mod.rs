//! Sibling-relationship rules.
//!
//! Currently exposes:
//!
//! - [`height_consistency`] — sibling elements that share a visual row
//!   should also share a height.
//! - [`padding_consistency`] — sibling elements should share consistent
//!   padding values.

pub mod height_consistency;
pub mod padding_consistency;
