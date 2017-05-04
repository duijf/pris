// Pris -- A language for designing slides
// Copyright 2017 Ruud van Asseldonk

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License version 3. A copy
// of the License is available in the root of the repository.

use std::rc::Rc;

use ast::Idents;
use cairo;
use elements::{Element, FillPolygon, StrokePolygon, Text, Vec2};
use error::{Error, Result};
use freetype;
use harfbuzz;
use pretty::Formatter;
use rsvg;
use runtime::{BoundingBox, Env, FontMap, Frame, Val};
use types::ValType;

fn validate_args<'a>(fn_name: &str,
                     expected: &[ValType],
                     actual: &[Val<'a>])
                     -> Result<()> {
    // First check that we have exactly the right number of arguments.
    if expected.len() != actual.len() {
        return Err(Error::arity(fn_name, expected.len() as u32, actual.len() as u32))
    }

    // Then check the type of each.
    for (i, (ex, ac)) in expected.iter().zip(actual).enumerate() {
        if *ex != ac.get_type() {
            return Err(Error::arg_type(fn_name, *ex, ac.get_type(), i as u32))
        }
    }

    Ok(())
}

pub fn fit<'a>(_fm: &mut FontMap,
               _env: &Env<'a>,
               mut args: Vec<Val<'a>>)
               -> Result<Val<'a>> {
    validate_args("fit", &[ValType::Frame, ValType::Coord(1)], &args)?;
    let frame = match args.remove(0) {
        Val::Frame(f) => f,
        _ => unreachable!(),
    };
    let size = match args.remove(0) {
        Val::Coord(w, h, 1) => (w, h),
        _ => unreachable!(),
    };

    let bb = frame.get_bounding_box();

    // Avoid division by zero in the aspect ratio computation. Fitting into a
    // box of which either size has length 0 is nonsense anyway.
    if size.0 == 0.0 || size.1 == 0.0 {
        return Err(Error::Other("Cannot fit frame in a box with \
                                 width or height equal to 0. \
                                 Simply don't place the frame then.".into()))
    }

    let scale = if bb.height != 0.0 {
        if (bb.width / bb.height) > (size.0 / size.1) {
            // The frame is constrained by width.
            size.0 / bb.width
        } else {
            // The frame is constrained by height.
            size.1 / bb.height
        }
    } else if bb.width != 0.0 {
        if (bb.height / bb.width) > (size.1 / size.0) {
            // The frame is constrained by height.
            size.1 / bb.height
        } else {
            // The frame is constrained by width.
            size.0 / bb.width
        }
    } else {
        return Err(Error::Other("Cannot fit a frame of size (0w, 0w).".into()))
    };

    // As the frame is immutable anyway, it would actually be possible to refer
    // to the elements in the frame, instead of copying them. If performance
    // ever becomes a concern, this would be a good place to start.
    let elements: Vec<_> = frame.get_elements().iter().cloned().collect();

    let mut scaled_frame = Frame::from_env(frame.get_env().clone());
    scaled_frame.place_element(Vec2::zero(), Element::Scaled(elements, scale));
    scaled_frame.set_anchor(frame.get_anchor() * scale);
    scaled_frame.union_bounding_box(&frame.get_bounding_box().scale(scale));

    Ok(Val::Frame(Rc::new(scaled_frame)))
}

pub fn line<'a>(_fm: &mut FontMap,
                env: &Env<'a>,
                mut args: Vec<Val<'a>>)
                -> Result<Val<'a>> {
    validate_args("line", &[ValType::Coord(1)], &args)?;
    let offset = match args.remove(0) {
        Val::Coord(x, y, 1) => Vec2::new(x, y),
        _ => unreachable!(),
    };

    let line = StrokePolygon {
        // TODO: Better idents type for non-ast use?
        color: env.lookup_color(&Idents(vec!["color"]))?,
        line_width: env.lookup_len(&Idents(vec!["line_width"]))?,
        close: false,
        vertices: vec![Vec2::zero(), offset],
    };

    let mut frame = Frame::new();
    frame.place_element(Vec2::zero(), Element::StrokePolygon(line));
    frame.set_anchor(offset);
    // TODO: Make bounding box take Vec2.
    frame.union_bounding_box(&BoundingBox::sized(offset.x, offset.y));

    Ok(Val::Frame(Rc::new(frame)))
}

pub fn fill_rectangle<'a>(_fm: &mut FontMap,
                          env: &Env<'a>,
                          mut args: Vec<Val<'a>>)
                          -> Result<Val<'a>> {
    validate_args("fill_rectangle", &[ValType::Coord(1)], &args)?;
    let (w, h) = match args.remove(0) {
        Val::Coord(x, y, 1) => (x, y),
        _ => unreachable!(),
    };

    let rect = FillPolygon {
        // TODO: Better idents type for non-ast use?
        color: env.lookup_color(&Idents(vec!["color"]))?,
        vertices: vec![
            Vec2::zero(),
            Vec2::new(0.0, h),
            Vec2::new(w, h),
            Vec2::new(w, 0.0),
        ],
    };

    let mut frame = Frame::new();
    frame.place_element(Vec2::zero(), Element::FillPolygon(rect));
    frame.set_anchor(Vec2::new(w, h));
    // TODO: Make bounding box take Vec2.
    frame.union_bounding_box(&BoundingBox::sized(w, h));

    Ok(Val::Frame(Rc::new(frame)))
}

pub fn str<'a>(_fm: &mut FontMap,
               _env: &Env<'a>,
               mut args: Vec<Val<'a>>)
               -> Result<Val<'a>> {
    // TODO: Make this generic over the dimension?
    validate_args("str", &[ValType::Num(0)], &args)?;
    let num = match args.remove(0) {
        Val::Num(x, _) => x,
        _ => unreachable!(),
    };

    Ok(Val::Str(format!("{}", num)))
}

/// Typesets a single line of text.
///
/// Returns the glyphs as well as the width of the line.
fn typeset_line(ft_face: &mut freetype::Face<'static>,
                font_size: f64,
                text: &str)
                -> (Vec<cairo::Glyph>, f64) {
    // Shape the text using Harfbuzz: convert the UTF-8 string and input font
    // into a list of glyphs with offsets.
    let mut hb_font = harfbuzz::Font::from_ft_face(ft_face);

    let mut hb_buffer = harfbuzz::Buffer::new(harfbuzz::Direction::LeftToRight);
    hb_buffer.add_str(&text);
    hb_buffer.shape(&mut hb_font);

    // Position all the glyphs: Harfbuzz gives offsets, but we need absolute
    // locations. Store them in the representation that Cairo expects.
    let hb_glyphs = hb_buffer.glyphs();
    let mut cr_glyphs = Vec::with_capacity(hb_glyphs.len());
    let (mut cur_x, mut cur_y) = (0.0, 0.0);

    // Compensate for the fixed font size which is set for the Freetype font,
    // and apply the desired font size.
    let size_factor = font_size / 1000.0;

    for hg in hb_glyphs {
        cur_x += hg.x_offset as f64 * size_factor;
        cur_y += hg.y_offset as f64 * size_factor;
        let cg = cairo::Glyph::new(hg.codepoint as u64, cur_x, cur_y);
        cur_x += hg.x_advance as f64 * size_factor;
        cur_y += hg.y_advance as f64 * size_factor;
        cr_glyphs.push(cg);
    }

    (cr_glyphs, cur_x)
}

/// Split a string on newlines.
///
/// Unlike `std::str::lines`, the final newline is not swallowed.
fn split_lines(text: &str) -> Vec<&str> {
    // TODO: This might return an iterator instead of a vector.
    // At this point it is not worth the trouble performance-wise.
    let mut lines = Vec::new();
    let mut left = text;
    while let Some(index) = left.find("\n") {
        lines.push(&left[0..index]);
        left = &left[index + 1..];
    }
    lines.push(left);
    lines
}

#[test]
fn split_lines_returns_as_many_lines_as_newlines_plus_one() {
    let text = "\nfoo\nbar\n";
    let lines = split_lines(text);
    assert_eq!(&lines, &["", "foo", "bar", ""]);
}

pub fn t<'a>(fm: &mut FontMap,
             env: &Env<'a>,
             mut args: Vec<Val<'a>>)
             -> Result<Val<'a>> {
    validate_args("t", &[ValType::Str], &args)?;
    let text = match args.remove(0) {
        Val::Str(s) => s,
        _ => unreachable!(),
    };
    let text_lines = split_lines(&text);

    enum TextAlign { Left, Center, Right }

    // Read the font details from the 'font_family' and 'font_style' variables,
    // and locate the corresponding FreeType face. The line height is a bit of a
    // problem; we could make it dimensionless and relative to the font size --
    // which would make it scale automatically -- but then specifying absolute
    // line heights would be a bit of a hassle. We could make it absolute, but
    // then it does not scale automatically. Or we could allow both here:
    // numbers have units, so we could figure out what to do. But my gut feeling
    // is that dynamic typing will be confusing in the end.
    let font_family = env.lookup_str(&Idents(vec!["font_family"]))?;
    let font_style = env.lookup_str(&Idents(vec!["font_style"]))?;
    let font_size = env.lookup_len(&Idents(vec!["font_size"]))?;
    let line_height = env.lookup_len(&Idents(vec!["line_height"]))?;
    let text_align = env.lookup_str(&Idents(vec!["text_align"]))?;
    let mut ft_face = match fm.get(&font_family, &font_style) {
        Some(face) => face,
        None => return Err(Error::missing_font(font_family, font_style)),
    };
    let ta = match text_align.as_ref() {
        "left" => TextAlign::Left,
        "center" => TextAlign::Center,
        "right" => TextAlign::Right,
        other => {
            // TODO: Move this error to an error at assignment time, not at
            // evaluation time. More type safety is more better.
            let mut fmt = Formatter::new();
            fmt.print("'");
            fmt.print(other);
            fmt.print("' is not a valid value for 'text_align'. ");
            fmt.print("Must be one of 'left', 'center', 'right'.");
            return Err(Error::value(fmt.into_string()))
        }
    };

    let mut glyphs = Vec::new();
    let mut max_width: f64 = 0.0;
    let mut min_offset: f64 = 0.0;
    let mut cur_x = 0.0;
    let mut cur_y = 0.0;
    for line in text_lines {
        let (line_glyphs, width) = typeset_line(ft_face, font_size, line);

        // Apply x offset to enforce text alignment.
        let offset = match ta {
            TextAlign::Left => 0.0,
            TextAlign::Center => width * -0.5,
            TextAlign::Right => width * -1.0,
        };

        for g in line_glyphs {
            glyphs.push(g.offset(offset, cur_y));
        }

        max_width = max_width.max(width);
        min_offset = min_offset.min(offset);
        cur_y += line_height;
        cur_x = offset + width;
    }

    let text_elem = Text {
        color: env.lookup_color(&Idents(vec!["color"]))?,
        font_family: font_family,
        font_style: font_style,
        font_size: font_size,
        glyphs: glyphs,
    };

    let mut frame = Frame::new();
    frame.place_element(Vec2::zero(), Element::Text(text_elem));
    frame.set_anchor(Vec2::new(cur_x, cur_y - line_height));

    let top_left = Vec2::new(min_offset, -line_height);
    let size = Vec2::new(max_width, cur_y);
    frame.union_bounding_box(&BoundingBox::new(top_left, size));

    Ok(Val::Frame(Rc::new(frame)))
}

pub fn glyph<'a>(fm: &mut FontMap,
                 env: &Env<'a>,
                 mut args: Vec<Val<'a>>)
                 -> Result<Val<'a>> {
    validate_args("glyph", &[ValType::Num(0)], &args)?;
    let index_f64 = match args.remove(0) {
        Val::Num(x, 0) => x,
        _ => unreachable!(),
    };

    let index = index_f64 as u64;

    if index as f64 != index_f64 {
        let msg = format!("Expected an unsigned integer glyph index, found {}.", index_f64);
        return Err(Error::value(msg))
    }

    // TODO: This was copy-pasted from the `t()` function. Extract the common
    // stuff.

    let font_family = env.lookup_str(&Idents(vec!["font_family"]))?;
    let font_style = env.lookup_str(&Idents(vec!["font_style"]))?;
    let font_size = env.lookup_len(&Idents(vec!["font_size"]))?;
    let line_height = env.lookup_len(&Idents(vec!["line_height"]))?;
    let _ft_face = match fm.get(&font_family, &font_style) {
        Some(face) => face,
        None => return Err(Error::missing_font(font_family, font_style)),
    };

    let glyphs = vec![cairo::Glyph::new(index, 0.0, 0.0)];

    // TODO: Extract the glyph width from the font.
    // TODO: Deal with text_align? Probably that is overkill and not very
    // useful.
    let width = 0.0;

    let text_elem = Text {
        color: env.lookup_color(&Idents(vec!["color"]))?,
        font_family: font_family,
        font_style: font_style,
        font_size: font_size,
        glyphs: glyphs,
    };

    let mut frame = Frame::new();
    frame.place_element(Vec2::zero(), Element::Text(text_elem));
    frame.set_anchor(Vec2::new(width, 0.0));

    let top_left = Vec2::new(0.0, -line_height);
    let size = Vec2::new(width, 0.0);
    frame.union_bounding_box(&BoundingBox::new(top_left, size));

    Ok(Val::Frame(Rc::new(frame)))
}

pub fn image<'a>(_fm: &mut FontMap,
                 _env: &Env<'a>,
                 mut args: Vec<Val<'a>>)
                 -> Result<Val<'a>> {
    validate_args("image", &[ValType::Str], &args)?;
    let path = match args.remove(0) {
        Val::Str(s) => s,
        _ => unreachable!(),
    };

    if !path.ends_with(".svg") {
        let msg = format!("Cannot load '{}', only svg images are supported for now.", path);
        return Err(Error::Other(msg))
    }

    // TODO: Make path relative to the source file. Some kind of state would be
    // required for this. Time to replace the FontMap everywhere with a more
    // elaborate state structure.

    let svg = match rsvg::Svg::open(&path) {
        Ok(svg) => svg,
        // TODO: Actually, the cause does not have to be a missing file, it
        // might be an ill-formed file or some other kind of IO error too.
        // Move error handling into the rsvg module proper.
        Err(()) => return Err(Error::missing_file(path)),
    };
    let (width, height) = svg.size();

    let mut frame = Frame::new();
    frame.place_element(Vec2::zero(), Element::Svg(svg));
    frame.union_bounding_box(&BoundingBox::sized(width as f64, height as f64));

    // The image anchor is in the top right, so images can be adjoined easily:
    // the origin is top left.
    frame.set_anchor(Vec2::new(width as f64, 0.0));

    Ok(Val::Frame(Rc::new(frame)))
}
