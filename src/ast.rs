// Pris -- A language for designing slides
// Copyright 2017 Ruud van Asseldonk

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License version 3. A copy
// of the License is available in the root of the repository.

use std::fmt::{Display, Error, Formatter};

pub enum Term<'a> {
    String(&'a str),
    Number(Num),
    Color(Color),
    Idents(Idents<'a>),
    Coord(Box<Coord<'a>>),
    BinOp(Box<BinTerm<'a>>),
}

pub struct Num(pub f64, pub Option<Unit>);

#[derive(Copy, Clone)]
pub enum Unit {
  W,
  H,
  Em,
  Pt,
}

pub struct Color(pub u8, pub u8, pub u8);

pub struct Idents<'a>(pub Vec<&'a str>);

pub struct Coord<'a>(pub Term<'a>, pub Term<'a>);

pub struct BinTerm<'a>(pub Term<'a>, pub BinOp, pub Term<'a>);

#[derive(Copy, Clone)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Exp,
}

// Pretty-printers.

impl<'a> Display for Term<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match *self {
            Term::String(s) => write!(f, "{}", s),
            Term::Number(ref n) => write!(f, "{}", n),
            Term::Color(ref c) => write!(f, "{}", c),
            Term::Idents(ref is) => write!(f, "{}", is),
            Term::Coord(ref co) => write!(f, "{}", co),
            Term::BinOp(ref bt) => write!(f, "{}", bt),
        }
    }
}

impl Display for Num {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self.0)?;
        if let Some(unit) = self.1 {
            write!(f, "{}", unit)?
        }
        Ok(())
    }
}

impl Display for Unit {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match *self {
            Unit::W => write!(f, "w"),
            Unit::H => write!(f, "h"),
            Unit::Em => write!(f, "em"),
            Unit::Pt => write!(f, "pt"),
        }
    }
}

impl Display for Color {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "#{:x}{:x}{:x}", self.0, self.1, self.2)
    }
}

impl<'a> Display for Idents<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        assert!(self.0.len() > 0);
        let mut parts = self.0.iter();
        write!(f, "{}", parts.next().unwrap())?;
        for p in parts {
            write!(f, ".{}", p)?;
        }
        Ok(())
    }
}

impl<'a> Display for Coord<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "({}, {})", self.0, self.1)
    }
}

impl<'a> Display for BinTerm<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "({} {} {})", self.0, self.1, self.2)
    }
}

impl Display for BinOp {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match *self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Exp => write!(f, "^"),
        }
    }
}
