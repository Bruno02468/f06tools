//! This library implements types and functions to parse and manipulate the
//! data within formatted text output files from Nastran-like FEA solvers.
//! 
//! It was created with the main intent being the development of a tool to
//! convert output from the MYSTRAN solver.to a CSV for use in automated
//! verification of the solver's correctness.
//! 
//! However, the code is modular -- one can easily expand the library to
//! support parsing different "flavours" of text output, different solvers,
//! more elements/formulations, etc.

#![warn(missing_docs)] // almost sure this is default but whatever
#![warn(clippy::missing_docs_in_private_items)] // sue me
#![allow(clippy::needless_return)] // i'll never forgive rust for this
#![allow(dead_code)] // temporary

pub mod elements;
pub mod fields;
pub mod flavour;
pub mod geometry;
pub mod util;

#[cfg(test)]
mod tests;
