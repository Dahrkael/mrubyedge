// String methods missing from the mrubyedge prelude, plus the Kernel-level
// format entry points.

use std::rc::Rc;

use crate::compat::util::mrb_funcall;
use crate::Error;
use crate::yamrb::helpers::mrb_define_cmethod;
use crate::yamrb::value::{RObject, RValue, Value};
use crate::yamrb::vm::VM;

use super::{block_of, need_block, pos_args};
use crate::compat::util::{class_rc, native};

fn self_string(vm: &mut VM) -> Result<String, Error> {
    let this = vm.getself()?;
    match this.rvalue() {
        Some(RValue::String(bytes, _)) => Ok(String::from_utf8_lossy(&bytes.borrow()).to_string()),
        _ => Err(Error::RuntimeError("receiver is not a String".into())),
    }
}

fn to_display(vm: &mut VM, arg: &Value) -> Result<String, Error> {
    if let Value::Object(o) = arg
        && let RValue::String(b, _) = &o.value
    {
        return Ok(String::from_utf8_lossy(&b.borrow()).to_string());
    }
    let s = mrb_funcall(vm, Some(arg.clone()), "to_s", &[])?;
    String::try_from(&s).map_err(|_| Error::ArgumentError("to_s did not yield a String".into()))
}

/// printf-style formatter covering d/i/u/f/e/E/g/G/x/X/o/b/c/s/% with the
/// usual `-`, `0`, `+` and space flags plus width and precision.
///
/// Width/precision are capped so a pathological format cannot overflow or
/// allocate unbounded memory.
pub(super) fn ruby_format(vm: &mut VM, fmt: &str, args: &[Value]) -> Result<String, Error> {
    const MAX_FORMAT_WIDTH: usize = 1_000_000;
    let mut out = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut ci = 0usize;
    let mut ai = 0usize;
    while ci < chars.len() {
        if chars[ci] != '%' {
            out.push(chars[ci]);
            ci += 1;
            continue;
        }
        ci += 1;
        match chars.get(ci) {
            None => return Err(Error::ArgumentError("trailing '%' in format".into())),
            Some('%') => {
                out.push('%');
                ci += 1;
                continue;
            }
            _ => {}
        }
        let mut left = false;
        let mut zero = false;
        let mut plus = false;
        let mut space = false;
        while let Some(c) = chars.get(ci) {
            match c {
                '-' => left = true,
                '0' => zero = true,
                '+' => plus = true,
                ' ' => space = true,
                _ => break,
            }
            ci += 1;
        }
        let mut width = 0usize;
        while let Some(c) = chars.get(ci) {
            if c.is_ascii_digit() {
                width = width
                    .saturating_mul(10)
                    .saturating_add(*c as usize - '0' as usize)
                    .min(MAX_FORMAT_WIDTH);
                ci += 1;
            } else {
                break;
            }
        }
        let mut prec: Option<usize> = None;
        if chars.get(ci) == Some(&'.') {
            ci += 1;
            let mut p = 0usize;
            while let Some(c) = chars.get(ci) {
                if c.is_ascii_digit() {
                    p = p
                        .saturating_mul(10)
                        .saturating_add(*c as usize - '0' as usize)
                        .min(MAX_FORMAT_WIDTH);
                    ci += 1;
                } else {
                    break;
                }
            }
            prec = Some(p);
        }
        let spec = *chars
            .get(ci)
            .ok_or_else(|| Error::ArgumentError("malformed format string".into()))?;
        ci += 1;

        // %s may consume several values when given an array directive set.
        let arg = match args.get(ai) {
            Some(a) => a.clone(),
            None => return Err(Error::ArgumentError("too few arguments for format".into())),
        };
        ai += 1;

        let numeric = matches!(
            spec,
            'd' | 'i' | 'u' | 'f' | 'e' | 'E' | 'g' | 'G' | 'x' | 'X' | 'o' | 'b'
        );
        let mut body = match spec {
            'd' | 'i' | 'u' => {
                let n = i64::try_from(&arg)
                    .map_err(|_| Error::ArgumentError("expected Integer".into()))?;
                let mag = n.unsigned_abs().to_string();
                let sign = if n < 0 {
                    "-"
                } else if plus {
                    "+"
                } else if space {
                    " "
                } else {
                    ""
                };
                format!("{sign}{mag}")
            }
            'f' => {
                let v = f64::try_from(&arg)
                    .map_err(|_| Error::ArgumentError("expected Float".into()))?;
                let s = format!("{:.*}", prec.unwrap_or(6), v);
                decorate_sign(&s, plus, space)
            }
            'e' | 'E' => {
                let v = f64::try_from(&arg)
                    .map_err(|_| Error::ArgumentError("expected Float".into()))?;
                let s = format!("{:.*e}", prec.unwrap_or(6), v);
                let styled = expand_exponent(&s);
                if spec == 'E' {
                    styled.to_uppercase()
                } else {
                    styled
                }
            }
            'g' | 'G' => {
                let v = f64::try_from(&arg)
                    .map_err(|_| Error::ArgumentError("expected Float".into()))?;
                decorate_sign(&format!("{v}"), plus, space)
            }
            'x' => base_int(&arg, plus, space, |n| format!("{n:x}"))?,
            'X' => base_int(&arg, plus, space, |n| format!("{n:X}"))?,
            'o' => base_int(&arg, plus, space, |n| format!("{n:o}"))?,
            'b' => base_int(&arg, plus, space, |n| format!("{n:b}"))?,
            'c' => match &arg {
                Value::Integer(n) => char::from_u32(*n as u32)
                    .map(|c| c.to_string())
                    .ok_or_else(|| Error::ArgumentError("invalid codepoint".into()))?,
                Value::Object(o) if matches!(&o.value, RValue::String(_, _)) => {
                    to_display(vm, &arg)?
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .ok_or_else(|| Error::ArgumentError("invalid character".into()))?
                }
                _ => return Err(Error::ArgumentError("%c needs an Integer or String".into())),
            },
            's' => {
                let mut s = to_display(vm, &arg)?;
                if let Some(p) = prec {
                    // Truncate by characters, never splitting a UTF-8 char.
                    s = s.chars().take(p).collect();
                }
                s
            }
            other => {
                return Err(Error::ArgumentError(format!(
                    "unsupported format specifier %{other}"
                )));
            }
        };

        if body.chars().count() < width {
            let cur = body.chars().count();
            let pad = width - cur;
            if left {
                body.push_str(&" ".repeat(pad));
            } else if zero && numeric {
                let (head, rest) = split_sign(&body);
                body = format!("{}{}{}", head, "0".repeat(pad), rest);
            } else {
                body = format!("{}{}", " ".repeat(pad), body);
            }
        }
        out.push_str(&body);
    }
    if ai < args.len() {
        return Err(Error::ArgumentError("too many arguments for format".into()));
    }
    Ok(out)
}

fn split_sign(s: &str) -> (&str, &str) {
    match s.as_bytes().first() {
        Some(b'-') | Some(b'+') | Some(b' ') => s.split_at(1),
        _ => ("", s),
    }
}

fn decorate_sign(s: &str, plus: bool, space: bool) -> String {
    if s.starts_with('-') {
        return s.to_string();
    }
    if plus {
        format!("+{s}")
    } else if space {
        format!(" {s}")
    } else {
        s.to_string()
    }
}

fn expand_exponent(s: &str) -> String {
    // Rust: 1.5e2 -> Ruby style: 1.500000e+02
    match s.split_once('e') {
        Some((mantissa, exp)) => {
            let (sign, digits) = match exp.strip_prefix('-') {
                Some(d) => ("-", d),
                None => ("+", exp),
            };
            let padded = format!("{digits:0>2}");
            format!("{mantissa}e{sign}{padded}")
        }
        None => s.to_string(),
    }
}

fn base_int(
    arg: &Value,
    plus: bool,
    space: bool,
    render: impl Fn(u64) -> String,
) -> Result<String, Error> {
    let n = i64::try_from(arg).map_err(|_| Error::ArgumentError("expected Integer".into()))?;
    let sign = if n < 0 {
        "-"
    } else if plus {
        "+"
    } else if space {
        " "
    } else {
        ""
    };
    Ok(format!("{sign}{}", render(n.unsigned_abs())))
}

// String#%(arg): printf-style interpolation; pass an Array for several slots.
fn cm_string_percent(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let fmt = self_string(vm)?;
    let values: Vec<Value> = match args.first().and_then(|a| a.as_ref()) {
        Some(a) => match a {
            Value::Object(o) if matches!(&o.value, RValue::Array(_)) => {
                Vec::try_from(a).map_err(|_| Error::ArgumentError("bad array argument".into()))?
            }
            _ => vec![a.clone()],
        },
        None => return Err(Error::ArgumentError("no argument for format".into())),
    };
    let text = ruby_format(vm, &fmt, &values)?;
    Ok(Value::from_rc(Rc::new(RObject::string(text))))
}

// Kernel#sprintf / Kernel#format.
fn cm_kernel_format(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let fmt = args
        .first()
        .and_then(|a| a.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("format requires a template".into()))?;
    let fmt = match &fmt {
        Value::Object(o) => match &o.value {
            RValue::String(b, _) => String::from_utf8_lossy(&b.borrow()).to_string(),
            _ => {
                return Err(Error::ArgumentError(
                    "format template must be a String".into(),
                ));
            }
        },
        _ => {
            return Err(Error::ArgumentError(
                "format template must be a String".into(),
            ));
        }
    };
    let values: Vec<Value> = args[1..]
        .iter()
        .map(|a| a.as_ref().unwrap().clone())
        .collect();
    let text = ruby_format(vm, &fmt, &values)?;
    Ok(Value::from_rc(Rc::new(RObject::string(text))))
}

// Shared engine of sub/gsub over literal string patterns. Replacement comes
// from the second argument or from a block receiving the matched text.
fn replace_occurrences(
    vm: &mut VM,
    haystack: &str,
    args: &[Option<Value>],
    all: bool,
) -> Result<Value, Error> {
    let pos = pos_args(args);
    let pattern = crate::compat::util::arg_string(pos, 0)?;
    if pattern.is_empty() {
        return Err(Error::ArgumentError(
            "empty pattern is not supported".into(),
        ));
    }
    let block = block_of(args);
    let literal = if block.is_none() {
        Some(crate::compat::util::arg_string(pos, 1)?)
    } else {
        None
    };
    let mut out = String::new();
    let mut cursor = 0usize;
    while let Some(rel) = haystack[cursor..].find(&pattern) {
        let at = cursor + rel;
        out.push_str(&haystack[cursor..at]);
        let matched = &haystack[at..at + pattern.len()];
        let rep = match &block {
            Some(b) => {
                let m = Value::from_rc(Rc::new(RObject::string(matched.to_string())));
                let r = crate::yamrb::helpers::mrb_call_block(
                    vm,
                    b.clone(),
                    None,
                    std::slice::from_ref(&m),
                    0,
                )?;
                to_display(vm, &r)?
            }
            None => literal.clone().unwrap_or_default(),
        };
        out.push_str(&rep);
        cursor = at + pattern.len();
        if !all {
            break;
        }
    }
    out.push_str(&haystack[cursor..]);
    Ok(Value::from_rc(Rc::new(RObject::string(out))))
}

// String#gsub(pattern, replacement | block): replaces every occurrence of the
// literal pattern.
fn cm_string_gsub(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    replace_occurrences(vm, &s, args, true)
}

// String#sub(pattern, replacement | block): replaces the first occurrence of
// the literal pattern.
fn cm_string_sub(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    replace_occurrences(vm, &s, args, false)
}

/// Expands a character-set spec supporting `a-z` style ranges.
fn expand_set(spec: &str) -> Vec<u8> {
    let b = spec.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if i + 2 < b.len() && b[i + 1] == b'-' {
            let (lo, hi) = if b[i] <= b[i + 2] {
                (b[i], b[i + 2])
            } else {
                (b[i + 2], b[i])
            };
            let mut c = lo;
            loop {
                out.push(c);
                if c == hi {
                    break;
                }
                c += 1;
            }
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

fn self_bytes(vm: &mut VM) -> Result<Vec<u8>, Error> {
    let this = vm.getself()?;
    match this.rvalue() {
        Some(RValue::String(bytes, _)) => Ok(bytes.borrow().clone()),
        _ => Err(Error::RuntimeError("receiver is not a String".into())),
    }
}

fn string_out(bytes: Vec<u8>) -> Rc<RObject> {
    Rc::new(RObject::string_from_vec(bytes))
}

// String#tr(from, to): transliterates set members; missing targets keep the
// last available one; an empty `to` deletes the matched characters.
fn cm_string_tr(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let src = self_bytes(vm)?;
    let from = expand_set(&crate::compat::util::arg_string(args, 0)?);
    let to_spec = crate::compat::util::arg_string(args, 1)?;
    let to = expand_set(&to_spec);
    let result: Vec<u8> = match to.len() {
        0 => src.into_iter().filter(|c| !from.contains(c)).collect(),
        _ => src
            .into_iter()
            .map(|c| match from.iter().position(|f| *f == c) {
                Some(i) => to[i.min(to.len() - 1)],
                None => c,
            })
            .collect(),
    };
    Ok(Value::from_rc(string_out(result)))
}

// String#delete(set): removes every member of the set.
fn cm_string_delete(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let src = self_bytes(vm)?;
    let set = expand_set(&crate::compat::util::arg_string(args, 0)?);
    Ok(Value::from_rc(string_out(
        src.into_iter().filter(|c| !set.contains(c)).collect(),
    )))
}

// String#squeeze(set = nil): collapses runs of repeated characters,
// optionally restricted to a set.
fn cm_string_squeeze(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let src = self_bytes(vm)?;
    let set = match args.first() {
        Some(a) => Some(expand_set(&crate::compat::util::arg_string(
            std::slice::from_ref(a),
            0,
        )?)),
        None => None,
    };
    let mut out: Vec<u8> = Vec::with_capacity(src.len());
    for c in src {
        if out.last() == Some(&c) && set.as_ref().is_none_or(|s| s.contains(&c)) {
            continue;
        }
        out.push(c);
    }
    Ok(Value::from_rc(string_out(out)))
}

// String#count(set): how many characters belong to the set.
fn cm_string_count(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let src = self_bytes(vm)?;
    let set = expand_set(&crate::compat::util::arg_string(args, 0)?);
    Ok(Value::Integer(
        src.into_iter().filter(|c| set.contains(c)).count() as i64,
    ))
}

fn pad_to(body: &str, width: usize, left: bool, right: bool, pad: &str) -> String {
    let len = body.chars().count();
    if len >= width || pad.is_empty() {
        return body.to_string();
    }
    let total = width - len;
    let mut out = String::new();
    if !left {
        // rjust / centered-left half
        let n = if right { total } else { total / 2 };
        out.extend(pad.chars().cycle().take(n));
    }
    out.push_str(body);
    if left || !right {
        let n = if left { total } else { total - total / 2 };
        out.extend(pad.chars().cycle().take(n));
    }
    out
}

/// Non-negative width argument for ljust/rjust/center.
fn arg_width(args: &[Option<Value>], i: usize) -> Result<usize, Error> {
    let w = crate::compat::util::arg_i64(args, i)?;
    if w < 0 {
        return Err(Error::ArgumentError(format!("negative width ({w})")));
    }
    Ok(w as usize)
}

// String#capitalize: first character upcased, the rest downcased.
fn cm_string_capitalize(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    let mut chars = s.chars();
    let out = match chars.next() {
        Some(first) => first
            .to_uppercase()
            .chain(chars.flat_map(|c| c.to_lowercase()))
            .collect::<String>(),
        None => s,
    };
    Ok(Value::from_rc(Rc::new(RObject::string(out))))
}

// String#reverse: character-reversed copy (UTF-8 safe).
fn cm_string_reverse(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    Ok(Value::from_rc(Rc::new(RObject::string(
        s.chars().rev().collect::<String>(),
    ))))
}

// String#ljust(width, pad = " ").
fn cm_string_ljust(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    let width = arg_width(args, 0)?;
    let pad = if args.len() > 1 {
        crate::compat::util::arg_string(args, 1)?
    } else {
        " ".to_string()
    };
    Ok(Value::from_rc(Rc::new(RObject::string(pad_to(
        &s, width, true, false, &pad,
    )))))
}

// String#rjust(width, pad = " ").
fn cm_string_rjust(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    let width = arg_width(args, 0)?;
    let pad = if args.len() > 1 {
        crate::compat::util::arg_string(args, 1)?
    } else {
        " ".to_string()
    };
    Ok(Value::from_rc(Rc::new(RObject::string(pad_to(
        &s, width, false, true, &pad,
    )))))
}

// String#center(width, pad = " "): odd padding goes to the right.
fn cm_string_center(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    let width = arg_width(args, 0)?;
    let pad = if args.len() > 1 {
        crate::compat::util::arg_string(args, 1)?
    } else {
        " ".to_string()
    };
    Ok(Value::from_rc(Rc::new(RObject::string(pad_to(
        &s, width, false, false, &pad,
    )))))
}

// String#swapcase.
fn cm_string_swapcase(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    Ok(Value::from_rc(Rc::new(RObject::string(
        s.chars()
            .flat_map(|c| {
                if c.is_lowercase() {
                    c.to_uppercase().collect::<Vec<_>>()
                } else {
                    c.to_lowercase().collect::<Vec<_>>()
                }
            })
            .collect::<String>(),
    ))))
}

// String#casecmp(other): -1/0/1 ignoring case; nil when other is no String.
fn cm_string_casecmp(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    let other = match args.first().and_then(|a| a.as_ref()).map(Value::to_rc) {
        Some(a) => match &a.value {
            RValue::String(b, _) => String::from_utf8_lossy(&b.borrow()).to_string(),
            _ => return Ok(Value::Nil),
        },
        None => return Ok(Value::Nil),
    };
    let ord = s.to_lowercase().cmp(&other.to_lowercase()) as i64;
    Ok(Value::Integer(ord))
}

// String#casecmp?(other).
fn cm_string_casecmp_p(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let eq = cm_string_casecmp(vm, args)?;
    Ok(match &eq {
        Value::Integer(0) => Value::Bool(true),
        Value::Nil => Value::Nil,
        _ => Value::Bool(false),
    })
}

// String#chop: drops the final character (or a trailing "\r\n" as one).
fn cm_string_chop(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    let trimmed = s
        .strip_suffix("\r\n")
        .map(str::to_string)
        .unwrap_or_else(|| {
            s.chars()
                .take(s.chars().count().saturating_sub(1))
                .collect()
        });
    Ok(Value::from_rc(Rc::new(RObject::string(trimmed))))
}

// String#prepend(*others): destructive head-append; returns self.
fn cm_string_prepend(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let mut prefix = String::new();
    for a in args {
        prefix.push_str(&to_display(vm, a.as_ref().unwrap())?);
    }
    match this.rvalue() {
        Some(RValue::String(bytes, _)) => {
            let mut merged = prefix.into_bytes();
            merged.extend_from_slice(&bytes.borrow());
            *bytes.borrow_mut() = merged;
        }
        _ => return Err(Error::RuntimeError("receiver is not a String".into())),
    }
    Ok(this)
}

// String#replace(other): swaps the whole content; returns self.
fn cm_string_replace(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let other = crate::compat::util::arg_string(args, 0)?;
    match this.rvalue() {
        Some(RValue::String(bytes, _)) => *bytes.borrow_mut() = other.into_bytes(),
        _ => return Err(Error::RuntimeError("receiver is not a String".into())),
    }
    Ok(this)
}

// String#concat(*objs): destructive tail-append of each argument; returns self.
fn cm_string_concat(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let mut extra = Vec::new();
    for a in args {
        extra.extend_from_slice(to_display(vm, a.as_ref().unwrap())?.as_bytes());
    }
    match this.rvalue() {
        Some(RValue::String(bytes, _)) => bytes.borrow_mut().extend_from_slice(&extra),
        _ => return Err(Error::RuntimeError("receiver is not a String".into())),
    }
    Ok(this)
}

// String#partition(sep): [before, sep, after].
fn cm_string_partition(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    let sep = crate::compat::util::arg_string(args, 0)?;
    let (a, b, c) = match s.find(&sep) {
        Some(i) => (
            s[..i].to_string(),
            sep.clone(),
            s[i + sep.len()..].to_string(),
        ),
        None => (s.clone(), String::new(), String::new()),
    };
    Ok(Value::from_rc(
        RObject::array(vec![
            Value::from_rc(Rc::new(RObject::string(a))),
            Value::from_rc(Rc::new(RObject::string(b))),
            Value::from_rc(Rc::new(RObject::string(c))),
        ])
        .to_refcount_assigned(),
    ))
}

// String#each_char { |ch| }: walks characters; returns self.
fn cm_string_each_char(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let s = self_string(vm)?;
    for ch in s.chars() {
        let a = Value::from_rc(Rc::new(RObject::string(ch.to_string())));
        crate::yamrb::helpers::mrb_call_block(
            vm,
            block.clone(),
            None,
            std::slice::from_ref(&a),
            0,
        )?;
    }
    vm.getself()
}

// String#each_byte { |b| }.
fn cm_string_each_byte(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let src = self_bytes(vm)?;
    for b in src {
        let a = Value::Integer(b as i64);
        crate::yamrb::helpers::mrb_call_block(
            vm,
            block.clone(),
            None,
            std::slice::from_ref(&a),
            0,
        )?;
    }
    vm.getself()
}

// String#each_line(sep = "\n") { |line| } and #lines(sep = "\n"): keeps the
// separator at each line end.
fn split_lines(s: &str, sep: &str) -> Vec<String> {
    if sep.is_empty() {
        return vec![s.to_string()];
    }
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find(sep) {
        out.push(rest[..i + sep.len()].to_string());
        rest = &rest[i + sep.len()..];
    }
    if !rest.is_empty() {
        out.push(rest.to_string());
    }
    out
}

fn line_sep(args: &[Option<Value>]) -> Result<String, Error> {
    Ok(match pos_args(args).first() {
        Some(a) => crate::compat::util::arg_string(std::slice::from_ref(a), 0)?,
        None => "\n".to_string(),
    })
}

fn cm_string_each_line(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let s = self_string(vm)?;
    let sep = line_sep(args)?;
    for line in split_lines(&s, &sep) {
        let a = Value::from_rc(Rc::new(RObject::string(line)));
        crate::yamrb::helpers::mrb_call_block(
            vm,
            block.clone(),
            None,
            std::slice::from_ref(&a),
            0,
        )?;
    }
    vm.getself()
}

fn cm_string_lines(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    let sep = line_sep(args)?;
    Ok(Value::from_rc(
        RObject::array(
            split_lines(&s, &sep)
                .into_iter()
                .map(|l| Value::from_rc(Rc::new(RObject::string(l))))
                .collect(),
        )
        .to_refcount_assigned(),
    ))
}

// String#hex: leading-whitespace tolerant hexadecimal; 0 when unparsable.
fn cm_string_hex(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?.trim().to_string();
    let body = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(&s);
    let value = i64::from_str_radix(body.trim_start_matches('+'), 16).unwrap_or(0);
    Ok(Value::Integer(value))
}

// String#oct: 0x/0b/0o prefixes, otherwise base-8; 0 when unparsable.
fn cm_string_oct(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?.trim().to_string();
    let (radix, body) = if let Some(b) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (16, b)
    } else if let Some(b) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        (2, b)
    } else if let Some(b) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        (8, b)
    } else {
        (8, s.as_str())
    };
    let value = i64::from_str_radix(body, radix).unwrap_or(0);
    Ok(Value::Integer(value))
}

// String#succ: alphanumeric increment with carry ("az" -> "ba", "z9" -> "aa0").
fn cm_string_succ(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let mut chars: Vec<char> = self_string(vm)?.chars().collect();
    if chars.is_empty() {
        return Ok(Value::from_rc(Rc::new(RObject::string(String::new()))));
    }
    let mut i = chars.len();
    loop {
        if i == 0 {
            // Carry past the head: grow following the first character's class.
            let seed = chars[0];
            let grown = if seed.is_ascii_lowercase() {
                'a'
            } else if seed.is_ascii_uppercase() {
                'A'
            } else if seed.is_ascii_digit() {
                // "9".succ == "10": digits grow with a leading 1.
                '1'
            } else {
                break;
            };
            chars.insert(0, grown);
            break;
        }
        i -= 1;
        let c = chars[i];
        match c {
            'z' => chars[i] = 'a',
            'Z' => chars[i] = 'A',
            '9' => chars[i] = '0',
            _ => {
                if c.is_ascii_alphanumeric() {
                    chars[i] = ((c as u8) + 1) as char;
                }
                break;
            }
        }
    }
    Ok(Value::from_rc(Rc::new(RObject::string(
        chars.into_iter().collect(),
    ))))
}

// String#<=>: byte-wise ordering; nil when the other side is no String.
fn cm_string_spaceship(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = self_string(vm)?;
    let other = match args.first().and_then(|a| a.as_ref()).map(Value::to_rc) {
        Some(a) => match &a.value {
            RValue::String(b, _) => String::from_utf8_lossy(&b.borrow()).to_string(),
            _ => return Ok(Value::Nil),
        },
        None => return Ok(Value::Nil),
    };
    Ok(Value::Integer(s.cmp(&other) as i64))
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let string = class_rc(vm, "String")?;
    native!(mrb_define_cmethod, vm, string, "%", cm_string_percent);
    native!(mrb_define_cmethod, vm, string, "gsub", cm_string_gsub);
    native!(mrb_define_cmethod, vm, string, "sub", cm_string_sub);
    native!(mrb_define_cmethod, vm, string, "tr", cm_string_tr);
    native!(mrb_define_cmethod, vm, string, "delete", cm_string_delete);
    native!(mrb_define_cmethod, vm, string, "squeeze", cm_string_squeeze);
    native!(mrb_define_cmethod, vm, string, "count", cm_string_count);
    native!(mrb_define_cmethod, vm, string, "reverse", cm_string_reverse);
    native!(mrb_define_cmethod, vm, string, "ljust", cm_string_ljust);
    native!(mrb_define_cmethod, vm, string, "rjust", cm_string_rjust);
    native!(mrb_define_cmethod, vm, string, "center", cm_string_center);
    native!(
        mrb_define_cmethod,
        vm,
        string,
        "swapcase",
        cm_string_swapcase
    );
    native!(
        mrb_define_cmethod,
        vm,
        string,
        "capitalize",
        cm_string_capitalize
    );
    native!(mrb_define_cmethod, vm, string, "casecmp", cm_string_casecmp);
    native!(
        mrb_define_cmethod,
        vm,
        string,
        "casecmp?",
        cm_string_casecmp_p
    );
    native!(mrb_define_cmethod, vm, string, "<=>", cm_string_spaceship);
    native!(mrb_define_cmethod, vm, string, "chop", cm_string_chop);
    native!(mrb_define_cmethod, vm, string, "prepend", cm_string_prepend);
    native!(mrb_define_cmethod, vm, string, "replace", cm_string_replace);
    native!(mrb_define_cmethod, vm, string, "concat", cm_string_concat);
    native!(
        mrb_define_cmethod,
        vm,
        string,
        "partition",
        cm_string_partition
    );
    native!(
        mrb_define_cmethod,
        vm,
        string,
        "each_char",
        cm_string_each_char
    );
    native!(
        mrb_define_cmethod,
        vm,
        string,
        "each_byte",
        cm_string_each_byte
    );
    native!(
        mrb_define_cmethod,
        vm,
        string,
        "each_line",
        cm_string_each_line
    );
    native!(mrb_define_cmethod, vm, string, "lines", cm_string_lines);
    native!(mrb_define_cmethod, vm, string, "hex", cm_string_hex);
    native!(mrb_define_cmethod, vm, string, "oct", cm_string_oct);
    native!(mrb_define_cmethod, vm, string, "succ", cm_string_succ);

    let object = class_rc(vm, "Object")?;
    native!(mrb_define_cmethod, vm, object, "sprintf", cm_kernel_format);
    native!(mrb_define_cmethod, vm, object, "format", cm_kernel_format);
    Ok(())
}
