//! The composer's recipient fields: finished addresses as pills, the one being typed as text,
//! with ranked suggestions under it.

pub(crate) mod field;
mod tokens;

pub(crate) use field::RecipientField;
pub(crate) use tokens::is_empty;
