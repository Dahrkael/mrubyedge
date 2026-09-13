// Enumerable methods missing from the mrubyedge prelude. Implemented over a
// materialized element list: Array yields itself, Hash yields [key, value]
// pairs, Range walks its integers, anything else falls back to #to_a.

use crate::Error;
use crate::compat::util::mrb_funcall;
use crate::yamrb::helpers::mrb_define_module_cmethod;
use crate::yamrb::value::{RHash, RObject, RValue, Value};
use crate::yamrb::vm::VM;

use super::{BlockResult, block_of, call_block_catch_break, need_block};

type CMethod = fn(&mut VM, &[Option<Value>]) -> Result<Value, Error>;
type ElementPair = (Vec<Value>, Vec<Value>);

fn arr(elems: Vec<Value>) -> Value {
    Value::from_rc(RObject::array(elems).to_refcount_assigned())
}

fn pair(k: &Value, v: &Value) -> Value {
    Value::from_rc(RObject::array(vec![k.clone(), v.clone()]).to_refcount_assigned())
}

/// One iteration item: how the block is invoked (`args`) plus what the method
/// treats as the element (`value`; a [key, value] pair for Hash).
struct Item {
    args: Vec<Value>,
    value: Value,
}

fn item_one(v: Value) -> Item {
    Item {
        args: vec![v.clone()],
        value: v,
    }
}

fn receiver_elements(vm: &mut VM) -> Result<Vec<Item>, Error> {
    let this = vm.getself()?;
    match this.rvalue() {
        Some(RValue::Array(a)) => Ok(a.borrow().iter().map(|e| item_one(e.clone())).collect()),
        Some(RValue::Hash(h)) => Ok(h
            .borrow()
            .iter()
            .map(|(_, (k, v))| Item {
                args: vec![k.clone(), v.clone()],
                value: pair(k, v),
            })
            .collect()),
        Some(RValue::Range(s, e, excl)) => {
            let sv = i64::try_from(s)
                .map_err(|_| Error::RuntimeError("range bounds must be integers".into()))?;
            let ev = i64::try_from(e)
                .map_err(|_| Error::RuntimeError("range bounds must be integers".into()))?;
            let mut items = Vec::new();
            let mut cur = sv;
            while if *excl { cur < ev } else { cur <= ev } {
                items.push(item_one(Value::Integer(cur)));
                match cur.checked_add(1) {
                    Some(next) => cur = next,
                    None => break,
                }
            }
            Ok(items)
        }
        _ => {
            let list = mrb_funcall(vm, Some(this.clone()), "to_a", &[])?;
            let elems: Vec<Value> = Vec::try_from(&list)
                .map_err(|_| Error::ArgumentError("to_a did not return an Array".into()))?;
            Ok(elems.into_iter().map(item_one).collect())
        }
    }
}

// Enumerable#reject { |e| }.
fn en_reject(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let mut out = Vec::new();
    for it in receiver_elements(vm)? {
        let drop = match call_block_catch_break(vm, &block, &it.args)? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        if !drop.is_truthy() {
            out.push(it.value);
        }
    }
    Ok(arr(out))
}

// Enumerable#filter: alias of the existing select.
fn en_filter(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let rc_args: Vec<Value> = args.iter().map(|a| a.as_ref().unwrap().clone()).collect();
    mrb_funcall(vm, Some(this), "select", &rc_args)
}

// Enumerable#none?(pattern = nil) { |e| }.
fn none_one(vm: &mut VM, args: &[Option<Value>], want_exactly_one: bool) -> Result<Value, Error> {
    let block = need_block(args)?;
    let mut hits = 0usize;
    for it in receiver_elements(vm)? {
        let hit = match call_block_catch_break(vm, &block, &it.args)? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        if hit.is_truthy() {
            hits += 1;
            if hits > 1 && want_exactly_one {
                return Ok(Value::Bool(false));
            }
        }
    }
    let ok = if want_exactly_one {
        hits == 1
    } else {
        hits == 0
    };
    Ok(Value::Bool(ok))
}

fn en_none(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    none_one(vm, args, false)
}

fn en_one(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    none_one(vm, args, true)
}

// Enumerable#flat_map { |e| }: maps and concatenates Array results.
fn en_flat_map(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let mut out = Vec::new();
    for it in receiver_elements(vm)? {
        let mapped = match call_block_catch_break(vm, &block, &it.args)? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        match Vec::try_from(&mapped) {
            Ok(mut inner) => out.append(&mut inner),
            Err(_) => out.push(mapped),
        }
    }
    Ok(arr(out))
}

// Enumerable#filter_map { |e| }: maps keeping truthy results.
fn en_filter_map(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let mut out = Vec::new();
    for it in receiver_elements(vm)? {
        let mapped = match call_block_catch_break(vm, &block, &it.args)? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        if mapped.is_truthy() {
            out.push(mapped);
        }
    }
    Ok(arr(out))
}

// Enumerable#group_by { |e| }.
fn en_group_by(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let mut groups: Vec<(Value, Vec<Value>)> = Vec::new();
    for it in receiver_elements(vm)? {
        let key = match call_block_catch_break(vm, &block, &it.args)? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        match groups.iter_mut().find(|(k, _)| rb_eq(k, &key)) {
            Some((_, list)) => list.push(it.value),
            None => groups.push((key, vec![it.value])),
        }
    }
    let mut out = RHash::default();
    for (k, list) in groups {
        out.insert(k.as_hash_key()?, (k.clone(), arr(list)));
    }
    Ok(Value::from_rc(RObject::hash(out).to_refcount_assigned()))
}

fn rb_eq(a: &Value, b: &Value) -> bool {
    a.as_eq_value() == b.as_eq_value()
}

// Shared max_by/min_by engine through <=> comparisons on block results.
fn by_extreme(vm: &mut VM, args: &[Option<Value>], want_max: bool) -> Result<Value, Error> {
    let block = need_block(args)?;
    let mut best: Option<(Value, Value)> = None;
    for it in receiver_elements(vm)? {
        let score = match call_block_catch_break(vm, &block, &it.args)? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        let better = match &best {
            None => true,
            Some((best_score, _)) => {
                let ord = mrb_funcall(
                    vm,
                    Some(score.clone()),
                    "<=>",
                    std::slice::from_ref(best_score),
                )?;
                match i64::try_from(&ord) {
                    Ok(v) => {
                        if want_max {
                            v > 0
                        } else {
                            v < 0
                        }
                    }
                    Err(_) => false,
                }
            }
        };
        if better {
            best = Some((score, it.value));
        }
    }
    Ok(best.map(|(_, e)| e).unwrap_or(Value::Nil))
}

fn en_max_by(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    by_extreme(vm, args, true)
}

fn en_min_by(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    by_extreme(vm, args, false)
}

// Enumerable#partition { |e| }: [matches, rest].
fn en_partition(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let mut yes = Vec::new();
    let mut no = Vec::new();
    for it in receiver_elements(vm)? {
        let hit = match call_block_catch_break(vm, &block, &it.args)? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        if hit.is_truthy() {
            yes.push(it.value);
        } else {
            no.push(it.value);
        }
    }
    Ok(arr(vec![arr(yes), arr(no)]))
}

fn slice_args(args: &[Option<Value>]) -> Result<usize, Error> {
    let positional_len = match block_of(args) {
        Some(_) => args.len() - 1,
        None => args.len(),
    };
    if positional_len == 0 {
        return Err(Error::ArgumentError("slice size required".into()));
    }
    let n = i64::try_from(args[0].as_ref().unwrap())
        .map_err(|_| Error::ArgumentError("size must be an Integer".into()))?;
    if n <= 0 {
        return Err(Error::ArgumentError("invalid slice size".into()));
    }
    Ok(n as usize)
}

// Enumerable#each_slice(n) { |chunk| }: yields fixed-size groups.
fn en_each_slice(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let n = slice_args(args)?;
    let items = receiver_elements(vm)?;
    for chunk in items.chunks(n) {
        let a = arr(chunk.iter().map(|it| it.value.clone()).collect());
        match call_block_catch_break(vm, &block, std::slice::from_ref(&a))? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(_) => {}
        }
    }
    vm.getself()
}

// Enumerable#each_cons(n) { |group| }: yields sliding windows.
fn en_each_cons(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let n = slice_args(args)?;
    let items = receiver_elements(vm)?;
    if items.len() >= n {
        for w in items.windows(n) {
            let a = arr(w.iter().map(|it| it.value.clone()).collect());
            match call_block_catch_break(vm, &block, std::slice::from_ref(&a))? {
                BlockResult::Broke(v) => return Ok(v),
                BlockResult::Value(_) => {}
            }
        }
    }
    vm.getself()
}

// Enumerable#tally: value counts as a Hash.
fn en_tally(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let mut counts: Vec<(Value, i64)> = Vec::new();
    for it in receiver_elements(vm)? {
        match counts.iter_mut().find(|(k, _)| rb_eq(k, &it.value)) {
            Some((_, n)) => *n += 1,
            None => counts.push((it.value, 1)),
        }
    }
    let mut out = RHash::default();
    for (k, n) in counts {
        out.insert(k.as_hash_key()?, (k.clone(), Value::Integer(n)));
    }
    Ok(Value::from_rc(RObject::hash(out).to_refcount_assigned()))
}

// Enumerable#take_while / #drop_while { |e| }.
fn while_split(vm: &mut VM, args: &[Option<Value>]) -> Result<(ElementPair, Option<Value>), Error> {
    let block = need_block(args)?;
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut broken = false;
    for it in receiver_elements(vm)? {
        let keep_going = if broken {
            Value::Bool(false)
        } else {
            match call_block_catch_break(vm, &block, &it.args)? {
                BlockResult::Broke(v) => return Ok(((head, tail), Some(v))),
                BlockResult::Value(v) => v,
            }
        };
        if !broken && keep_going.is_truthy() {
            head.push(it.value);
        } else {
            broken = true;
            tail.push(it.value);
        }
    }
    Ok(((head, tail), None))
}

fn en_take_while(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let ((head, _), brk) = while_split(vm, args)?;
    if let Some(v) = brk {
        return Ok(v);
    }
    Ok(arr(head))
}

fn en_drop_while(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let ((_, tail), brk) = while_split(vm, args)?;
    if let Some(v) = brk {
        return Ok(v);
    }
    Ok(arr(tail))
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let enumerable = vm.get_module_by_name("Enumerable");
    let defs: &[(&str, CMethod)] = &[
        ("reject", en_reject),
        ("filter", en_filter),
        ("none?", en_none),
        ("one?", en_one),
        ("flat_map", en_flat_map),
        ("filter_map", en_filter_map),
        ("group_by", en_group_by),
        ("max_by", en_max_by),
        ("min_by", en_min_by),
        ("partition", en_partition),
        ("each_slice", en_each_slice),
        ("each_cons", en_each_cons),
        ("tally", en_tally),
        ("take_while", en_take_while),
        ("drop_while", en_drop_while),
    ];
    for (name, func) in defs {
        mrb_define_module_cmethod(vm, enumerable.clone(), name, Box::new(*func));
    }
    Ok(())
}
