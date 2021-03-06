// Pris -- A language for designing slides
// Copyright 2017 Ruud van Asseldonk

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License version 3. A copy
// of the License is available in the root of the repository.

extern crate freetype;
extern crate lalrpop_util;

mod builtins;
mod elements;
mod fontconfig;
mod harfbuzz;
mod parser;
mod pretty;
mod rsvg;
mod types;

pub mod ast;
pub mod cairo;
pub mod driver;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod runtime;
pub mod syntax;

// This is the compiler entry point for the library, which is used by the
// command-line program. The source for that program is in bin/pris.rs.
