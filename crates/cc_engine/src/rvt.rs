//! The Revit converter that `rvt_import` runs in a child process (feature
//! `rvt`). By default it is this program started again with [`SUBCOMMAND`],
//! so a program that runs `rvt_import` calls [`child_main`] when its first
//! argument is [`SUBCOMMAND`] (`ccad` and the app do).

pub use cc_rvt::{Converter, SUBCOMMAND, child_main};

use std::sync::RwLock;

static CONVERTER: RwLock<Option<Converter>> = RwLock::new(None);

/// Set the program `rvt_import` runs to convert a file; `None` (the default)
/// runs this program again with [`SUBCOMMAND`].
pub fn set_converter(converter: Option<Converter>) {
    if let Ok(mut c) = CONVERTER.write() {
        *c = converter;
    }
}

/// The converter `rvt_import` runs.
pub(crate) fn converter() -> Result<Converter, cc_rvt::RvtError> {
    match CONVERTER.read().ok().and_then(|c| c.clone()) {
        Some(c) => Ok(c),
        None => Converter::this_program(),
    }
}
