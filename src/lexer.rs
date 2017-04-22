// Pris -- A language for designing slides
// Copyright 2017 Ruud van Asseldonk

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License version 3. A copy
// of the License is available in the root of the repository.

//! This module contains building blocks for the parser. The actual parser is
//! generated by Lalrpop, and can be found in the `syntax` module.


//! This module contains a custom lexer that feeds tokens to Lalrpop.
//!
//! A custom lexer is required because the lexer generated by Lalrpop cannot
//! deal with comments that span to the end of the line. It also enables support
//! for non-greedy triple quoted strings, which cannot be expressed in as regex
//! without support for non-greedy matching.

use error::{Error, Result};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Token {
    String,
    RawString,
    Color,
    Number,
    Ident,

    KwAt,
    KwFunction,
    KwImport,
    KwPut,
    KwReturn,

    UnitEm,
    UnitH,
    UnitW,
    UnitPt,

    Comma,
    Dot,
    Equals,
    Hat,
    Minus,
    Plus,
    Slash,
    Star,
    Tilde,

    LParen,
    RParen,
    LBrace,
    RBrace,
}

/// Lexes a UTF-8 input file into (start_index, token, past_end_index) tokens.
pub fn lex(input: &[u8]) -> Result<Vec<(usize, Token, usize)>> {
    Lexer::new(input).run()
}

enum State {
    Base,
    Done,
    InColor,
    InComment,
    InIdent,
    InNumber,
    InRawString,
    InString,
    Space,
}

struct Lexer<'a> {
    input: &'a [u8],
    start: usize,
    state: State,
    tokens: Vec<(usize, Token, usize)>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a [u8]) -> Lexer<'a> {
        Lexer {
            input: input,
            start: 0,
            state: State::Base,
            tokens: Vec::new(),
        }
    }

    /// Run the lexer on the full input and return the tokens.
    ///
    /// Returns tuples of (start_index, token, past_end_index).
    fn run(mut self) -> Result<Vec<(usize, Token, usize)>> {
        loop {
            let (start, state) = match self.state {
                State::Base => self.lex_base()?,
                State::InColor => self.lex_color()?,
                State::InComment => self.lex_comment()?,
                State::InIdent => self.lex_ident()?,
                State::InNumber => self.lex_number()?,
                State::InRawString => self.lex_raw_string()?,
                State::InString => self.lex_string()?,
                State::Space => self.lex_space()?,
                State::Done => break,
            };
            self.start = start;
            self.state = state;
        }

        Ok(self.tokens)
    }

    /// Check whether the byte sequence occurs at an index.
    fn has_at(&self, at: usize, expected: &[u8]) -> bool {
        // There must at least be sufficient bytes left to match the entire
        // expected string.
        if at + expected.len() > self.input.len() {
            return false
        }

        // Then check that every byte matches.
        for (a, e) in self.input[at..].iter().zip(expected) {
            if a != e {
                return false
            }
        }

        true
    }

    /// Push a single-byte token, and set the start of the next token past it.
    fn push_single(&mut self, at: usize, tok: Token) {
        self.tokens.push((at, tok, at + 1));
        self.start = at + 1;
    }

    /// Lex in the base state until a state change occurs.
    ///
    /// Returns new values for `self.start` and `self.state`.
    fn lex_base(&mut self) -> Result<(usize, State)> {
        for i in self.start..self.input.len() {
            match self.input[i] {
                // There are two characters that require a brief lookahead:
                // * '/', to find the start of a comment "//".
                // * '-', to find the start of a raw string "---".
                // If the lookahead does not match, these characters are matched
                // again as single-character tokens further below.
                b'/' if self.has_at(i + 1, b"/") => {
                    return change_state(i, State::InComment)
                }
                b'-' if self.has_at(i + 1, b"--") => {
                    return change_state(i, State::InRawString)
                }

                // A few characters signal a change of state immediately. Note
                // that only spaces and newlines are considered whitespace.
                // No tabs or carriage returns please.
                b'"' => {
                    return change_state(i, State::InString)
                }
                b' ' | b'\n' => {
                    return change_state(i, State::Space)
                }
                b'#' => {
                    return change_state(i, State::InColor)
                }
                byte if is_alphabetic_or_underscore(byte) => {
                    return change_state(i, State::InIdent)
                }
                byte if is_digit(byte) => {
                    return change_state(i, State::InNumber)
                }

                // A number of punctuation characters are tokens themselves. For
                // these we push a single-byte token and continue after without
                // changing state. Pushing a single token does reset the start
                // counter.
                b',' => self.push_single(i, Token::Comma),
                b'.' => self.push_single(i, Token::Dot),
                b'=' => self.push_single(i, Token::Equals),
                b'^' => self.push_single(i, Token::Hat),
                b'-' => self.push_single(i, Token::Minus),
                b'+' => self.push_single(i, Token::Plus),
                b'/' => self.push_single(i, Token::Slash),
                b'*' => self.push_single(i, Token::Star),
                b'~' => self.push_single(i, Token::Tilde),
                b'(' => self.push_single(i, Token::LParen),
                b')' => self.push_single(i, Token::RParen),
                b'{' => self.push_single(i, Token::LBrace),
                b'}' => self.push_single(i, Token::RBrace),

                // If we detect the start of a byte order mark, complain about a
                // wrong encoding. (No BOMs for UTF-8 either, please.)
                0xef | 0xfe | 0xff | 0x00 => {
                    return Err(make_encoding_error(i, &self.input[i..]))
                }

                // Anything else is invalid. Please, no tabs or carriage
                // returns. And *definitely* no levitating men in business
                // suits. (Note that all of those are fine in comments and
                // strings, so you can still document everything in a non-Latin
                // language, or make slides for that. Just keep the source clean
                // please.)
                byte => return Err(make_parse_error(i, &self.input[i..])),
            }
        }

        done_at_end_of_input()
    }

    /// Lex in the color state until a state change occurs.
    fn lex_color(&mut self) -> Result<(usize, State)> {
        debug_assert!(self.has_at(self.start, b"#"));

        // Skip over the first '#' byte.
        for i in 1..self.input.len() - self.start {
            let start = self.start;
            let start_i = self.start + i;
            let c = self.input[start_i];

            // A hexadecimal character, as expected.
            if i < 7 && is_hexadecimal(c) {
                continue
            }

            // We expected more hexadecimal digits, but found something else.
            if i < 7 {
                let msg = format!("Expected hexadecimal digit, found '{}'.", char::from(c));
                return Err(Error::parse(start_i, start_i + 1, msg))
            }

            // We expect at most 6 hexadecimal digits, but if another
            // alphanumeric character comes after this, we don't want to
            // terminate the color and switch to identifier; that would lead to
            // very confusing parse errors later on. Report an error here
            // instead.
            if i == 7 && is_hexadecimal(c) {
                let msg = "Expected only six hexadecimal digits, found one more.";
                return Err(Error::parse(start, start_i + 1, msg.into()))
            }
            if i == 7 && is_alphanumeric_or_underscore(c) {
                let msg = format!("Expected six hexadecimal digits, found extra '{}'.", char::from(c));
                return Err(Error::parse(start, start_i + 1, msg))
            }

            // The end of the color in a non-hexadecimal character, as expected.
            // Re-inspect the current character from the base state.
            if i == 7 && !is_hexadecimal(c) {
                self.tokens.push((self.start, Token::Color, i));
                return change_state(i, State::Base)
            }

            assert!(i < self.start + 7, "Would enter infinite loop when lexing color.");
        }

        // The input ends in a color.
        self.tokens.push((self.start, Token::Color, self.input.len()));
        done_at_end_of_input()
    }

    /// Skip until a newline is found, then switch to the whitespace state.
    fn lex_comment(&mut self) -> Result<(usize, State)> {
        debug_assert!(self.has_at(self.start, b"//"));

        // Skip the first two bytes, those are the "//" characters.
        for i in self.start + 2..self.input.len() {
            if self.input[i] == b'\n' {
                // Change to the whitespace state, because the last character
                // we saw was whitespace after all. Continue immediately at
                // the next byte (i + 1), there is no need to re-inspect the
                // newline.
                return change_state(i + 1, State::Space)
            }
        }

        done_at_end_of_input()
    }

    /// Lex an identifier untl a state change occurs.
    fn lex_ident(&mut self) -> Result<(usize, State)> {
        debug_assert!(is_alphabetic_or_underscore(self.input[self.start]));

        // Skip the first byte, because we already know that it contains
        // either an alphabetic character or underscore. For the other
        // characters, digits are allowed too.
        for i in self.start + 1..self.input.len() {
            if !is_alphanumeric_or_underscore(self.input[i]) {
                // An identifier consists of alphanumeric characters or
                // underscores, so at the first one that is not one of those,
                // change to the base state and re-inspect it.
                self.tokens.push((self.start, Token::Ident, i));
                return change_state(i, State::Base)
            }
        }

        // The input ended in an identifier.
        self.tokens.push((self.start, Token::Ident, self.input.len()));
        done_at_end_of_input()
    }

    /// Lex in the number state until a state change occurs.
    fn lex_number(&mut self) -> Result<(usize, State)> {
        debug_assert!(is_digit(self.input[self.start]));

        let mut period_seen = false;

        // Skip over the first digit, as we know already that it is a digit.
        for i in self.start + 1..self.input.len() {
            match self.input[i] {
                c if is_digit(c) => {
                    continue
                }
                b'.' if !period_seen => {
                    // Allow a single decimal period in the number.
                    period_seen = true
                    // TODO: Enforce that the next byte is a digit; numbers
                    // should not end in a period. (Just for style). But the
                    // lexer is simpler if this is allowed.
                }
                // For the various unit suffixes, we emit a separate token,
                // after emitting the number token. Then switch to the base
                // state and continue after the suffix.
                b'e' if self.has_at(i + 1, b"m") => {
                    self.tokens.push((self.start, Token::Number, i));
                    self.tokens.push((i, Token::UnitEm, i + 2));
                    return change_state(i + 2, State::Base)
                }
                b'p' if self.has_at(i + 1, b"t") => {
                    self.tokens.push((self.start, Token::Number, i));
                    self.tokens.push((i, Token::UnitPt, i + 2));
                    return change_state(i + 2, State::Base)
                }
                b'h' => {
                    self.tokens.push((self.start, Token::Number, i));
                    self.push_single(i, Token::UnitH);
                    return change_state(i + 1, State::Base)
                }
                b'w' => {
                    self.tokens.push((self.start, Token::Number, i));
                    self.push_single(i, Token::UnitW);
                    return change_state(i + 1, State::Base)
                }
                _ => {
                    // Not a digit or first period, re-inspect this byte in the
                    // base state.
                    self.tokens.push((self.start, Token::Number, i));
                    return change_state(i, State::Base)
                }
            }
        }

        // The input ended in a number.
        self.tokens.push((self.start, Token::Number, self.input.len()));
        done_at_end_of_input()
    }

    /// Lex in the raw string state until a "---" is found.
    fn lex_raw_string(&mut self) -> Result<(usize, State)> {
        debug_assert!(self.has_at(self.start, b"---"));

        // Skip over the first "---" that starts the literal.
        for i in self.start + 3..self.input.len() {
            match self.input[i] {
                b'-' if self.has_at(i + 1, b"--") => {
                    // Another "---" marks the end of the raw string. Continue
                    // in the base state after the last dash.
                    self.tokens.push((self.start, Token::RawString, i + 3));
                    return change_state(i + 3, State::Base)
                }
                _ => continue,
            }
        }

        // If we reach end of input inside a raw string, that's an error.
        let msg = "Raw string was not closed with '---' before end of input.";
        Err(Error::parse(self.start, self.start + 3, msg.into()))
    }

    /// Lex in the string state until a closing quote is found.
    fn lex_string(&mut self) -> Result<(usize, State)> {
        debug_assert!(self.has_at(self.start, b"\""));

        // Skip over the first quote that starts the literal.
        let mut skip_next = false;
        for i in self.start + 1..self.input.len() {
            if skip_next {
                skip_next = false;
                continue
            }
            match self.input[i] {
                b'\\' => {
                    // For the lexer, skip over anything after a backslash, even
                    // if it is not a valid escape code. The parser will handle
                    // those.
                    skip_next = true
                }
                b'"' => {
                    // Continue in the base state after the closing quote.
                    self.tokens.push((self.start, Token::String, i + 1));
                    return change_state(i + 1, State::Base)
                }
                _ => continue,
            }
        }

        // If we reach end of input inside a string, that's an error.
        let msg = "String was not closed with '\"' before end of input.";
        Err(Error::parse(self.start, self.start + 1, msg.into()))
    }

    /// Lex in the whitespace state until a state change occurs.
    fn lex_space(&mut self) -> Result<(usize, State)> {
        for i in self.start..self.input.len() {
            match self.input[i] {
                b' ' | b'\n' => {
                    continue
                }
                b'\t' | b'\r' => {
                    // Be very strict about whitespace; report an error for tabs
                    // and carriage returns. `make_parse_error()` generates a
                    // specialized error message for these.
                    return Err(make_parse_error(i, &self.input[i..]))
                }
                _ => {
                    // On anything else we switch back to the base state and
                    // inspect the current byte again in that state.
                    return change_state(i, State::Base)
                }
            }
        }

        done_at_end_of_input()
    }
}

/// Make `Lexer::run()` change to a different state, starting at the given byte.
///
/// This is only a helper function to make the lexer code a bit more readable,
/// the logic is in `Lexer::run()`.
fn change_state(at: usize, state: State) -> Result<(usize, State)> {
    Ok((at, state))
}

/// Signal end of input to the `Lexer::run()` method.
///
/// This is only a helper function to make the lexer code a bit more readable,
/// the logic is in `Lexer::run()`.
fn done_at_end_of_input() -> Result<(usize, State)> {
    Ok((0, State::Done))
}

/// Check whether a byte of UTF-8 is an ASCII letter.
fn is_alphabetic(byte: u8) -> bool {
    (b'a' <= byte && byte <= b'z') || (b'A' <= byte && byte <= b'Z')
}

/// Check whether a byte of UTF-8 is an ASCII letter or underscore.
fn is_alphabetic_or_underscore(byte: u8) -> bool {
    is_alphabetic(byte) || (byte == b'_')
}

/// Check whether a byte of UTF-8 is an ASCII letter, digit, or underscore.
fn is_alphanumeric_or_underscore(byte: u8) -> bool {
    is_alphabetic_or_underscore(byte) || (b'0' <= byte && byte <= b'9')
}

/// Check whether a byte of UTF-8 is an ASCII digit.
fn is_digit(byte: u8) -> bool {
    b'0' <= byte && byte <= b'9'
}

/// Check whether a byte of UTF-8 is a hexadecimal character.
fn is_hexadecimal(byte: u8) -> bool {
    is_digit(byte) || (b'a' <= byte && byte <= b'f') || (b'A' <= byte && byte <= b'F')
}

/// Detects a few byte order marks and returns an error
fn make_encoding_error(at: usize, input: &[u8]) -> Error {
    let (message, count) = if input.starts_with(&[0xef, 0xbb, 0xbf]) {
        // There is a special place in hell for people who use byte order marks
        // in UTF-8.
        ("Found UTF-8 byte order mark. Please remove it.", 3)
    } else if input.starts_with(&[0xfe, 0xff]) ||
              input.starts_with(&[0xff, 0xfe]) {
        ("Expected UTF-8 encoded file, but found UTF-16 byte order mark.", 2)
    } else if input.starts_with(&[0x00, 0x00, 0xfe, 0xff]) ||
              input.starts_with(&[0xff, 0xfe, 0x00, 0x00]) {
        ("Expected UTF-8 encoded file, but found UTF-32 byte order mark.", 4)
    } else {
        // If it was not a known byte order mark after all, complain about the
        // character as a normal parse error.
        return make_parse_error(at, input)
    };

    Error::parse(at, at + count, message.into())
}

fn make_parse_error(at: usize, input: &[u8]) -> Error {
    let message = match input[0] {
        b'\t' => {
            "Found tab character. Please use spaces instead.".into()
        }
        b'\r' => {
            "Found carriage return. Please use Unix line endings instead.".into()
        }
        x if x < 0x20 || x == 0x7f => {
            // An ASCII control character. In this case the character is likely
            // not printable as-is, so we include the byte in the message, and
            // an encoding hint.
            format!("Found unexpected control character 0x{:x}. ", x) +
            "Note that Pris expects UTF-8 encoded files."
        }
        x if x < 0x7f => {
            // A regular ASCII character, but apparently not one we expected at
            // this place.
            format!("Found unexpected character '{}'.", char::from(x))
        }
        x => {
            // If we find a non-ASCII byte, try to decode the next few bytes as
            // UTF-8. If that succeeds, complain about non-ASCII identifiers.
            // Otherwise complain about the encoding. Note that the unwrap will
            // succeed, as we have at least one byte in the input.
            match String::from_utf8_lossy(&input[..8]).chars().next().unwrap() {
                '\u{fffd}' => {
                    // U+FFFD is generated when decoding UTF-8 fails.
                    format!("Found unexpected byte 0x{:x}. ", x) +
                    "Note that Pris expects UTF-8 encoded files."
                }
                c => {
                    format!("Found unexpected character '{}'. ", c) +
                    "Note that identifiers must be ASCII."
                }
            }
        }
    };

    // The end index is not entirely correct for the non-ASCII but valid UTF-8
    // case, but meh.
    Error::parse(at, at + 1, message)
}

#[test]
fn lex_handles_a_simple_input() {
    let input = b"foo bar";
    let tokens = lex(input).unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0], (0, Token::Ident, 3));
    assert_eq!(tokens[1], (4, Token::Ident, 7));
}

#[test]
fn lex_handles_a_string_literal() {
    let input = br#"foo "bar""#;
    let tokens = lex(input).unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0], (0, Token::Ident, 3));
    assert_eq!(tokens[1], (4, Token::String, 9));
}

#[test]
fn lex_handles_a_string_literal_with_escaped_quote() {
    let input = br#""bar\"baz""#;
    let tokens = lex(input).unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0], (0, Token::String, 10));
}

#[test]
fn lex_strips_a_comment() {
    let input = b"foo\n// This is comment\nbar";
    let tokens = lex(input).unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0], (0, Token::Ident, 3));
    assert_eq!(tokens[1], (23, Token::Ident, 26));
}

#[test]
fn lex_handles_a_raw_string() {
    let input = b"foo---bar---baz";
    let tokens = lex(input).unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0], (0, Token::Ident, 3));
    assert_eq!(tokens[1], (3, Token::RawString, 12));
    assert_eq!(tokens[2], (12, Token::Ident, 15));
}
