// Array methods missing from the mrubyedge prelude.

use std::cell::RefCell;
use std::rc::Rc;

use crate::Error;
use crate::yamrb::helpers::mrb_define_cmethod;
use crate::yamrb::value::{RObject, RValue, Value};
use crate::yamrb::vm::VM;

use crate::compat::util::mrb_funcall;

use super::{BlockResult, block_of, call_block_catch_break, pos_args, rb_eq};
use crate::compat::util::{class_rc, native, nil};

pub(super) fn arr_of(this: &Value) -> Option<&RefCell<Vec<Value>>> {
    match this.rvalue() {
        Some(RValue::Array(a)) => Some(a),
        _ => None,
    }
}

// Array#delete(obj): removes every element equal to obj; returns obj, or nil
// when nothing matched.
fn cm_array_delete(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let target = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("delete requires an argument".into()))?
        .clone();
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#delete must be called on an Array".into(),
        ));
    };

    // Snapshot before running user `==` code: the block may mutate the array
    // (and with it the RefCell), so no borrow may stay live across rb_eq.
    let elems: Vec<Value> = arr.borrow().clone();
    let mut kept: Vec<Value> = Vec::with_capacity(elems.len());
    let mut removed = false;
    for e in &elems {
        if rb_eq(vm, e, &target)? {
            removed = true;
        } else {
            kept.push(e.clone());
        }
    }
    if !removed {
        return Ok(Value::Nil);
    }
    *arr.borrow_mut() = kept;
    Ok(target)
}

// Array#index / Array#find_index(obj | block): position of the first element
// equal to obj (or matching the block), nil when none.
fn cm_array_index(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#index must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    if let Some(block) = block_of(args) {
        for (i, e) in elems.iter().enumerate() {
            let hit = match call_block_catch_break(vm, &block, std::slice::from_ref(e))? {
                BlockResult::Broke(v) => return Ok(v),
                BlockResult::Value(v) => v,
            };
            if hit.is_truthy() {
                return Ok(Value::Integer(i as i64));
            }
        }
        return Ok(Value::Nil);
    }
    let target = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("index requires an argument or a block".into()))?
        .clone();
    for (i, e) in elems.iter().enumerate() {
        if rb_eq(vm, e, &target)? {
            return Ok(Value::Integer(i as i64));
        }
    }
    Ok(Value::Nil)
}

// Array#rindex(obj | block): position of the last matching element, nil when
// none.
fn cm_array_rindex(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#rindex must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    if let Some(block) = block_of(args) {
        for (i, e) in elems.iter().enumerate().rev() {
            let hit = match call_block_catch_break(vm, &block, std::slice::from_ref(e))? {
                BlockResult::Broke(v) => return Ok(v),
                BlockResult::Value(v) => v,
            };
            if hit.is_truthy() {
                return Ok(Value::Integer(i as i64));
            }
        }
        return Ok(Value::Nil);
    }
    let target = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("rindex requires an argument or a block".into()))?
        .clone();
    for (i, e) in elems.iter().enumerate().rev() {
        if rb_eq(vm, e, &target)? {
            return Ok(Value::Integer(i as i64));
        }
    }
    Ok(Value::Nil)
}

// Array#insert(idx, *objs): inserts before position idx; negative positions
// count from the end (-1 appends), gaps are filled with nil. Returns self.
fn cm_array_insert(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let positional = pos_args(args);
    let mut idx = crate::compat::util::arg_i64(positional, 0)?;
    let items = &positional[1..];
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#insert must be called on an Array".into(),
        ));
    };

    let mut elems = arr.borrow().clone();
    let len = elems.len() as i64;
    if idx < 0 {
        idx += len + 1;
        if idx < 0 {
            return Err(Error::ArgumentError("insert index too small".into()));
        }
    }
    while (elems.len() as i64) < idx {
        elems.push(Value::Nil);
    }
    let at = idx as usize;
    for (k, item) in items.iter().enumerate() {
        elems.insert(at + k, item.as_ref().unwrap().clone());
    }
    *arr.borrow_mut() = elems;
    Ok(this)
}

// Array#reject { |e| }: new array with the elements for which the block is
// falsey.
fn cm_array_reject(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = block_of(args).ok_or_else(|| Error::ArgumentError("no block given".into()))?;
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#reject must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    let mut out = Vec::new();
    for e in elems {
        let drop = match call_block_catch_break(vm, &block, std::slice::from_ref(&e))? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        if !drop.is_truthy() {
            out.push(e);
        }
    }
    Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
}

// Array#first(n = nil): head element, or the first n elements as an array.
fn cm_array_first(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#first must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    match pos_args(args).first() {
        None => Ok(elems.first().cloned().unwrap_or(Value::Nil)),
        Some(count) => {
            let n = crate::compat::util::arg_i64(std::slice::from_ref(count), 0)?;
            if n < 0 {
                return Err(Error::ArgumentError("negative array size".into()));
            }
            let take = (n as usize).min(elems.len());
            Ok(Value::from_rc(
                RObject::array(elems[..take].to_vec()).to_refcount_assigned(),
            ))
        }
    }
}

// Array#last(n = nil): tail element, or the last n elements in order.
fn cm_array_last(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#last must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    match pos_args(args).first() {
        None => Ok(elems.last().cloned().unwrap_or(Value::Nil)),
        Some(count) => {
            let n = crate::compat::util::arg_i64(std::slice::from_ref(count), 0)?;
            if n < 0 {
                return Err(Error::ArgumentError("negative array size".into()));
            }
            let skip = elems.len().saturating_sub(n as usize);
            Ok(Value::from_rc(
                RObject::array(elems[skip..].to_vec()).to_refcount_assigned(),
            ))
        }
    }
}

// Array#reverse: new array in reverse order.
fn cm_array_reverse(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#reverse must be called on an Array".into(),
        ));
    };
    let mut elems: Vec<Value> = arr.borrow().clone();
    elems.reverse();
    Ok(Value::from_rc(RObject::array(elems).to_refcount_assigned()))
}

// Array#reverse!: reverses self in place and returns self.
fn cm_array_reverse_self(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#reverse! must be called on an Array".into(),
        ));
    };
    arr.borrow_mut().reverse();
    Ok(this)
}

// Array#reverse_each { |e| }: iterates from tail to head; returns self.
fn cm_array_reverse_each(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = block_of(args).ok_or_else(|| Error::ArgumentError("no block given".into()))?;
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#reverse_each must be called on an Array".into(),
        ));
    };
    let mut elems: Vec<Value> = arr.borrow().clone();
    elems.reverse();
    for e in elems {
        match call_block_catch_break(vm, &block, std::slice::from_ref(&e))? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(_) => {}
        }
    }
    Ok(this)
}

// Array#rotate(n = 1): new array with elements shifted n positions toward the
// head (negative n shifts the other way).
fn cm_array_rotate(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#rotate must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    let n = match pos_args(args).first() {
        Some(v) => crate::compat::util::arg_i64(std::slice::from_ref(v), 0)?,
        None => 1,
    };
    let len = elems.len();
    if len == 0 {
        return Ok(Value::from_rc(
            RObject::array(vec![]).to_refcount_assigned(),
        ));
    }
    let k = (((n % len as i64) + len as i64) % len as i64) as usize;
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&elems[k..]);
    out.extend_from_slice(&elems[..k]);
    Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
}

// Array#take(n): the first n elements; raises on negative n.
fn cm_array_take(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#take must be called on an Array".into(),
        ));
    };
    let n = crate::compat::util::arg_i64(pos_args(args), 0)?;
    if n < 0 {
        return Err(Error::ArgumentError("negative array size".into()));
    }
    let elems: Vec<Value> = arr.borrow().clone();
    let take = (n as usize).min(elems.len());
    Ok(Value::from_rc(
        RObject::array(elems[..take].to_vec()).to_refcount_assigned(),
    ))
}

// Array#drop(n): everything but the first n elements; raises on negative n.
fn cm_array_drop(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#drop must be called on an Array".into(),
        ));
    };
    let n = crate::compat::util::arg_i64(pos_args(args), 0)?;
    if n < 0 {
        return Err(Error::ArgumentError("negative array size".into()));
    }
    let elems: Vec<Value> = arr.borrow().clone();
    let skip = (n as usize).min(elems.len());
    Ok(Value::from_rc(
        RObject::array(elems[skip..].to_vec()).to_refcount_assigned(),
    ))
}

// Array#values_at(*idx): element at each index (negatives from the end);
// missing positions yield nil.
fn cm_array_values_at(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#values_at must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    let len = elems.len() as i64;
    let mut out = Vec::new();
    for a in pos_args(args) {
        let mut i = crate::compat::util::arg_i64(std::slice::from_ref(a), 0)?;
        if i < 0 {
            i += len;
        }
        match elems.get(i as usize) {
            Some(e) => out.push(e.clone()),
            None => out.push(Value::Nil),
        }
    }
    Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
}

// Array#dig(idx, *rest): nested access; nil holes short-circuit, receivers
// lacking #dig surface their own NoMethodError.
fn cm_array_dig(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#dig must be called on an Array".into(),
        ));
    };
    let positional = pos_args(args);
    let mut i = crate::compat::util::arg_i64(positional, 0)?;
    let len = arr.borrow().len() as i64;
    if i < 0 {
        i += len;
    }
    let mut cur = match arr.borrow().get(i as usize) {
        Some(e) => e.clone(),
        None => Value::Nil,
    };
    for key in &positional[1..] {
        if matches!(cur, Value::Nil) {
            break;
        }
        cur = mrb_funcall(
            vm,
            Some(cur.clone()),
            "dig",
            std::slice::from_ref(key.as_ref().unwrap()),
        )?;
    }
    Ok(cur)
}

// Array#each_with_object(obj) { |e, memo| }: yields element and memo; the
// memo is also the return value.
fn cm_array_each_with_object(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = block_of(args).ok_or_else(|| Error::ArgumentError("no block given".into()))?;
    let memo = pos_args(args)
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("each_with_object requires an object".into()))?
        .clone();
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#each_with_object must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    for e in elems {
        let pair = [e, memo.clone()];
        match call_block_catch_break(vm, &block, &pair)? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(_) => {}
        }
    }
    Ok(memo)
}

// Array#compact!: drops nils in place; returns self, or nil when unchanged.
fn cm_array_compact_self(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#compact! must be called on an Array".into(),
        ));
    };
    let kept: Vec<Value> = arr
        .borrow()
        .iter()
        .filter(|e| !matches!(e, Value::Nil))
        .cloned()
        .collect();
    let changed = kept.len() != arr.borrow().len();
    *arr.borrow_mut() = kept;
    Ok(if changed { this } else { Value::Nil })
}

// Array#product(*others): cartesian product as arrays of tuples.
fn cm_array_product(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#product must be called on an Array".into(),
        ));
    };
    let mut lists: Vec<Vec<Value>> = vec![arr.borrow().clone()];
    for a in pos_args(args) {
        let v: Vec<Value> = Vec::try_from(a.as_ref().unwrap())
            .map_err(|_| Error::ArgumentError("expected Array".into()))?;
        lists.push(v);
    }
    let mut acc: Vec<Vec<Value>> = vec![Vec::new()];
    for list in lists {
        let mut next = Vec::new();
        for prefix in &acc {
            for item in &list {
                let mut row = prefix.clone();
                row.push(item.clone());
                next.push(row);
            }
        }
        acc = next;
    }
    Ok(Value::from_rc(
        RObject::array(
            acc.into_iter()
                .map(|row| Value::from_rc(RObject::array(row).to_refcount_assigned()))
                .collect(),
        )
        .to_refcount_assigned(),
    ))
}

// Array#zip(*others): rows of self paired element-wise; short lists pad nil.
fn cm_array_zip(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#zip must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    let mut others: Vec<Vec<Value>> = Vec::new();
    for a in pos_args(args) {
        let v: Vec<Value> = Vec::try_from(a.as_ref().unwrap())
            .map_err(|_| Error::ArgumentError("expected Array".into()))?;
        others.push(v);
    }
    let mut out = Vec::with_capacity(elems.len());
    for (i, e) in elems.iter().enumerate() {
        let mut row = vec![e.clone()];
        for o in &others {
            match o.get(i) {
                Some(v) => row.push(v.clone()),
                None => row.push(Value::Nil),
            }
        }
        out.push(Value::from_rc(RObject::array(row).to_refcount_assigned()));
    }
    Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
}

/// Random index below max through the Random class RNG (shared with srand).
fn rb_rand_below(vm: &mut VM, max: usize) -> Result<usize, Error> {
    let rng = vm
        .get_const_by_name("Random")
        .ok_or_else(|| Error::NameError("Random".into()))?;
    let arg = Value::Integer(max as i64);
    let r = mrb_funcall(
        vm,
        Some(Value::from_rc(rng)),
        "rand",
        std::slice::from_ref(&arg),
    )?;
    i64::try_from(&r)
        .map(|v| v as usize)
        .map_err(|_| Error::RuntimeError("rand returned a non-Integer".into()))
}

// Array#shuffle: new array in random order (Fisher-Yates over the global RNG).
fn cm_array_shuffle(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#shuffle must be called on an Array".into(),
        ));
    };
    let mut elems: Vec<Value> = arr.borrow().clone();
    for i in (1..elems.len()).rev() {
        let j = rb_rand_below(vm, i + 1)?;
        elems.swap(i, j);
    }
    Ok(Value::from_rc(RObject::array(elems).to_refcount_assigned()))
}

// Array#sample(n = nil): one random element (nil on empty), or n random picks
// without replacement.
fn cm_array_sample(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#sample must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    match pos_args(args).first() {
        None => {
            if elems.is_empty() {
                return Ok(Value::Nil);
            }
            let idx = rb_rand_below(vm, elems.len())?;
            Ok(elems[idx].clone())
        }
        Some(count) => {
            let n = crate::compat::util::arg_i64(std::slice::from_ref(count), 0)? as usize;
            let mut pool: Vec<Value> = elems.clone();
            let mut out = Vec::new();
            for _ in 0..n.min(pool.len()) {
                let idx = rb_rand_below(vm, pool.len())?;
                out.push(pool.remove(idx));
            }
            Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
        }
    }
}

// Array#reject! { |e| }: removes the matching elements in place; returns self,
// or nil when nothing was removed.
fn cm_array_reject_self(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = block_of(args).ok_or_else(|| Error::ArgumentError("no block given".into()))?;
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#reject! must be called on an Array".into(),
        ));
    };
    let elems: Vec<Value> = arr.borrow().clone();
    let mut kept = Vec::with_capacity(elems.len());
    for e in elems {
        let drop = match call_block_catch_break(vm, &block, std::slice::from_ref(&e))? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        if !drop.is_truthy() {
            kept.push(e);
        }
    }
    let changed = kept.len() != arr.borrow().len();
    *arr.borrow_mut() = kept;
    Ok(if changed { this } else { Value::Nil })
}

// Array#concat(*others): appends the elements of each other array to self in
// place; returns self.
fn cm_array_concat(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#concat must be called on an Array".into(),
        ));
    };
    let mut append = Vec::new();
    for a in pos_args(args) {
        let v: Vec<Value> = Vec::try_from(a.as_ref().unwrap())
            .map_err(|_| Error::ArgumentError("expected Array".into()))?;
        append.extend(v);
    }
    arr.borrow_mut().extend(append);
    Ok(this)
}

/// Resolves a fill start (Integer or Range) and an optional length into the
/// half-open index span [beg, end) over an array of `len` elements.
fn fill_span(
    len: usize,
    start: &Rc<RObject>,
    length: Option<i64>,
) -> Result<(usize, usize), Error> {
    let len_i = len as i64;
    let (beg, end) = match &start.value {
        RValue::Integer(v) => {
            let mut b = *v;
            if b < 0 {
                b += len_i;
                if b < 0 {
                    b = 0;
                }
            }
            let e = match length {
                Some(n) => b + n,
                None => len_i,
            };
            (b, e)
        }
        RValue::Range(s, e, excl) => {
            if length.is_some() {
                return Err(Error::ArgumentError(
                    "fill length cannot be combined with a range".into(),
                ));
            }
            let b = match s {
                Value::Integer(v) => {
                    let mut b = *v;
                    if b < 0 {
                        b += len_i;
                        if b < 0 {
                            b = 0;
                        }
                    }
                    b
                }
                Value::Nil => 0,
                _ => return Err(Error::ArgumentError("wrong argument type".into())),
            };
            let e = match e {
                Value::Integer(v) => {
                    let mut v = *v;
                    if v < 0 {
                        v += len_i;
                    }
                    if *excl {
                        v -= 1;
                    }
                    v + 1
                }
                Value::Nil => len_i,
                _ => return Err(Error::ArgumentError("wrong argument type".into())),
            };
            (b, e)
        }
        _ => return Err(Error::ArgumentError("wrong argument type".into())),
    };
    Ok((beg as usize, end.max(0) as usize))
}

// Array#fill(obj | start [, length] [, range]): overwrites a section of self
// (the whole array by default) with the object, or with the block result at
// each index. Negative starts count from the end; ends past the length extend
// the array, padding the gap with nil. Returns self.
fn cm_array_fill(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let Some(arr) = arr_of(&this) else {
        return Err(Error::RuntimeError(
            "Array#fill must be called on an Array".into(),
        ));
    };
    let positional = pos_args(args);
    let block = block_of(args);
    let (item, start, length) = if block.is_some() {
        match positional.len() {
            0 => (None, None, None),
            1 => (None, Some(positional[0].as_ref().unwrap().to_rc()), None),
            2 => (
                None,
                Some(positional[0].as_ref().unwrap().to_rc()),
                Some(positional[1].clone()),
            ),
            _ => {
                return Err(Error::ArgumentError(
                    "fill takes at most two arguments with a block".into(),
                ));
            }
        }
    } else {
        match positional.len() {
            0 => {
                return Err(Error::ArgumentError(
                    "fill requires a value or a block".into(),
                ));
            }
            1 => (Some(positional[0].as_ref().unwrap().to_rc()), None, None),
            2 => (
                Some(positional[0].as_ref().unwrap().to_rc()),
                Some(positional[1].as_ref().unwrap().to_rc()),
                None,
            ),
            3 => (
                Some(positional[0].as_ref().unwrap().to_rc()),
                Some(positional[1].as_ref().unwrap().to_rc()),
                Some(positional[2].clone()),
            ),
            _ => {
                return Err(Error::ArgumentError(
                    "fill takes at most three arguments".into(),
                ));
            }
        }
    };
    let length = match length {
        Some(l) => {
            let n = crate::compat::util::arg_i64(std::slice::from_ref(&l), 0)?;
            if n < 0 {
                return Err(Error::ArgumentError("negative argument".into()));
            }
            if n == 0 {
                return Ok(this);
            }
            Some(n)
        }
        None => None,
    };
    let len = arr.borrow().len();
    let (beg, end) = match start {
        None => (0usize, len),
        Some(s) => fill_span(len, &s, length)?,
    };
    if end > len {
        arr.borrow_mut().resize(end, Value::Nil);
    }
    match item {
        Some(v) => {
            let mut guard = arr.borrow_mut();
            for i in beg..end {
                guard[i] = Value::from_rc(v.clone());
            }
        }
        None => {
            let block = block.expect("block form has a block");
            let mut elems: Vec<Value> = arr.borrow().clone();
            for (i, slot) in elems.iter_mut().enumerate().take(end).skip(beg) {
                let idx = Value::Integer(i as i64);
                let v = call_block_catch_break(vm, &block, std::slice::from_ref(&idx))?;
                let v = match v {
                    BlockResult::Broke(v) => return Ok(v),
                    BlockResult::Value(v) => v,
                };
                *slot = v;
            }
            *arr.borrow_mut() = elems;
        }
    }
    Ok(this)
}

// Array#<=>: lexicographic ordering; nil when the other side is no Array or
// when an element comparison is incomparable. Lets sort_by order by array
// keys like [z, serial].
fn cm_array_spaceship(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let other = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("<=> requires an argument".into()))?;
    let a: Vec<Value> = match arr_of(&this) {
        Some(a) => a.borrow().iter().cloned().collect(),
        None => return Ok(nil()),
    };
    let b: Vec<Value> = match arr_of(other) {
        Some(b) => b.borrow().iter().cloned().collect(),
        None => return Ok(nil()),
    };
    for (x, y) in a.iter().zip(b.iter()) {
        let cmp = mrb_funcall(vm, Some(x.clone()), "<=>", std::slice::from_ref(y))?;
        if cmp.is_nil() {
            return Ok(nil());
        }
        match i64::try_from(&cmp) {
            Ok(0) => {}
            Ok(v) => return Ok(Value::Integer(v)),
            Err(_) => return Ok(nil()),
        }
    }
    let r = if a.len() < b.len() {
        -1
    } else if a.len() > b.len() {
        1
    } else {
        0
    };
    Ok(Value::Integer(r))
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let array = class_rc(vm, "Array")?;
    native!(mrb_define_cmethod, vm, array, "<=>", cm_array_spaceship);
    native!(mrb_define_cmethod, vm, array, "concat", cm_array_concat);
    native!(mrb_define_cmethod, vm, array, "fill", cm_array_fill);
    native!(mrb_define_cmethod, vm, array, "delete", cm_array_delete);
    native!(mrb_define_cmethod, vm, array, "index", cm_array_index);
    native!(mrb_define_cmethod, vm, array, "find_index", cm_array_index);
    native!(mrb_define_cmethod, vm, array, "rindex", cm_array_rindex);
    native!(mrb_define_cmethod, vm, array, "insert", cm_array_insert);
    native!(mrb_define_cmethod, vm, array, "reject", cm_array_reject);
    native!(mrb_define_cmethod, vm, array, "first", cm_array_first);
    native!(mrb_define_cmethod, vm, array, "last", cm_array_last);
    native!(mrb_define_cmethod, vm, array, "reverse", cm_array_reverse);
    native!(
        mrb_define_cmethod,
        vm,
        array,
        "reverse!",
        cm_array_reverse_self
    );
    native!(
        mrb_define_cmethod,
        vm,
        array,
        "reverse_each",
        cm_array_reverse_each
    );
    native!(mrb_define_cmethod, vm, array, "rotate", cm_array_rotate);
    native!(mrb_define_cmethod, vm, array, "take", cm_array_take);
    native!(mrb_define_cmethod, vm, array, "drop", cm_array_drop);
    native!(
        mrb_define_cmethod,
        vm,
        array,
        "values_at",
        cm_array_values_at
    );
    native!(mrb_define_cmethod, vm, array, "dig", cm_array_dig);
    native!(
        mrb_define_cmethod,
        vm,
        array,
        "each_with_object",
        cm_array_each_with_object
    );
    native!(
        mrb_define_cmethod,
        vm,
        array,
        "compact!",
        cm_array_compact_self
    );
    native!(mrb_define_cmethod, vm, array, "product", cm_array_product);
    native!(mrb_define_cmethod, vm, array, "zip", cm_array_zip);
    native!(mrb_define_cmethod, vm, array, "shuffle", cm_array_shuffle);
    native!(mrb_define_cmethod, vm, array, "sample", cm_array_sample);
    native!(
        mrb_define_cmethod,
        vm,
        array,
        "reject!",
        cm_array_reject_self
    );
    Ok(())
}
