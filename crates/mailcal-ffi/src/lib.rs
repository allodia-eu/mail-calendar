//! The C ABI library every native client loads: `mailcal-bindings`, linked as one `cdylib`.
//!
//! Everything it exports is the bindings' own. UniFFI's scaffolding there is `#[no_mangle]`, and a
//! `cdylib` exports every such symbol of the crates it links; this line is what links it.
use mailcal_bindings as _;
