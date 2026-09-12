// Hash methods missing from the mrubyedge prelude.

use std::cell::RefCell;
use std::rc::Rc;

use crate::compat::util::mrb_funcall;
use crate::Error;
use crate::yamrb::helpers::mrb_define_cmethod;
use crate::yamrb::prelude::hash as h;
use crate::yamrb::value::{RHash, RObject, RValue, Value};
use crate::yamrb::vm::VM;

use super::{BlockResult, block_of, call_block_catch_break, need_block, pos_args, rb_eq};
use crate::compat::util::{class_rc, native};

type Pair = (Value, Value);

fn hash_of(this: &Value) -> Result<&RefCell<RHash>, Error> {
    match this.rvalue() {
        Some(RValue::Hash(hm)) => Ok(hm),
        _ => Err(Error::RuntimeError("receiver is not a Hash".into())),
    }
}

/// Cloned (key, value) snapshot so Ruby callbacks can run without borrow conflicts.
fn pairs(this: &Value) -> Result<Vec<Pair>, Error> {
    Ok(hash_of(this)?
        .borrow()
        .iter()
        .map(|(_, (k, v))| (k.clone(), v.clone()))
        .collect())
}

fn find_pair(vm: &mut VM, list: &[Pair], key: &Value) -> Result<Option<Value>, Error> {
    for (k, v) in list {
        if rb_eq(vm, k, key)? {
            return Ok(Some(v.clone()));
        }
    }
    Ok(None)
}

// Hash#fetch(key, default = nil) { |key| }: value or default/block; raises a
// KeyError when missing and no fallback exists.
fn cm_hash_fetch(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let positional = pos_args(args);
    let key = positional
        .first()
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("fetch requires a key".into()))?;
    if let Some(v) = find_pair(vm, &pairs(&this)?, &key)? {
        return Ok(v);
    }
    if let Some(Value::Object(o)) = args.last().and_then(|a| a.as_ref())
        && matches!(o.value, RValue::Proc(_))
    {
        let block = o.clone();
        return match call_block_catch_break(vm, &block, std::slice::from_ref(&key))? {
            BlockResult::Broke(v) | BlockResult::Value(v) => Ok(v),
        };
    }
    match positional.get(1).and_then(|v| v.as_ref()) {
        Some(dflt) => Ok(dflt.clone()),
        None => {
            let shown = mrb_funcall(vm, Some(key.clone()), "inspect", &[])?;
            let text = String::try_from(&shown).unwrap_or_default();
            Err(Error::TaggedError(
                "KeyError".to_string(),
                format!("key not found: {text}"),
            ))
        }
    }
}

// Hash#key?(k) / #member?(k).
fn cm_hash_has_key(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let key = args
        .first()
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("expected a key".into()))?;
    let hit = find_pair(vm, &pairs(&this)?, &key)?;
    Ok(Value::Bool(hit.is_some()))
}

// Hash#value?(v).
fn cm_hash_has_value(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let target = args
        .first()
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("expected a value".into()))?;
    for (_, v) in pairs(&this)? {
        if rb_eq(vm, &v, &target)? {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}

// Hash#dig(key, *rest).
fn cm_hash_dig(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let key = args
        .first()
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("dig requires a key".into()))?;
    let mut cur = match find_pair(vm, &pairs(&this)?, &key)? {
        Some(v) => v,
        None => Value::Nil,
    };
    for next in &args[1..] {
        if matches!(cur, Value::Nil) {
            break;
        }
        let next = next.as_ref().unwrap().clone();
        cur = mrb_funcall(vm, Some(cur.clone()), "dig", std::slice::from_ref(&next))?;
    }
    Ok(cur)
}

// Hash#values_at(*keys): missing positions yield nil.
fn cm_hash_values_at(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let stored = pairs(&this)?;
    let mut out = Vec::new();
    for a in pos_args(args) {
        let key = a.as_ref().unwrap().clone();
        out.push(find_pair(vm, &stored, &key)?.unwrap_or(Value::Nil));
    }
    Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
}

// Hash#transform_values { |v| }: new hash with the same keys.
fn cm_hash_transform_values(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let block = need_block(args)?;
    let mut out = RHash::default();
    for (k, v) in pairs(&this)? {
        let nv = match call_block_catch_break(vm, &block, std::slice::from_ref(&v))? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        out.insert(k.as_hash_key()?, (k.clone(), nv));
    }
    Ok(Value::from_rc(RObject::hash(out).to_refcount_assigned()))
}

// Hash#transform_keys { |k| }: new hash with the same values.
fn cm_hash_transform_keys(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let block = need_block(args)?;
    let mut out = RHash::default();
    for (k, v) in pairs(&this)? {
        let nk = match call_block_catch_break(vm, &block, std::slice::from_ref(&k))? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        out.insert(nk.as_hash_key()?, (nk.clone(), v.clone()));
    }
    Ok(Value::from_rc(RObject::hash(out).to_refcount_assigned()))
}

// Hash#invert: swapped key/value pairs.
fn cm_hash_invert(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let mut out = RHash::default();
    for (k, v) in pairs(&this)? {
        out.insert(v.as_hash_key()?, (v.clone(), k.clone()));
    }
    Ok(Value::from_rc(RObject::hash(out).to_refcount_assigned()))
}

// Hash#store(key, value): alias of []=.
fn cm_hash_store(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let key = args
        .first()
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("store requires a key".into()))?;
    let value = args
        .get(1)
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("store requires a value".into()))?;
    h::mrb_hash_set_index(&this, key, value)
}

// Hash#each_pair { |k, v| }: delegates to Hash#each.
fn cm_hash_each_pair(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let rc_args: Vec<Value> = args.iter().map(|a| a.as_ref().unwrap().clone()).collect();
    mrb_funcall(vm, Some(this), "each", &rc_args)
}

// Shared body of merge/merge!: target receives other's pairs; conflicts go
// through the optional block (key, old, new).
fn merge_into(
    vm: &mut VM,
    target: &Value,
    other: &Value,
    block: Option<&Rc<RObject>>,
) -> Result<Option<Value>, Error> {
    for (k, nv) in pairs(other)? {
        match find_pair(vm, &pairs(target)?, &k)? {
            Some(ov) => {
                let final_v = match block {
                    Some(b) => {
                        let trio = [k.clone(), ov.clone(), nv.clone()];
                        match call_block_catch_break(vm, b, &trio)? {
                            BlockResult::Broke(v) => return Ok(Some(v)),
                            BlockResult::Value(v) => v,
                        }
                    }
                    None => nv,
                };
                h::mrb_hash_set_index(target, k, final_v)?;
            }
            None => {
                h::mrb_hash_set_index(target, k, nv)?;
            }
        }
    }
    Ok(None)
}

fn other_hash(args: &[Option<Value>]) -> Result<Value, Error> {
    pos_args(args)
        .first()
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("merge requires a Hash".into()))
}

// Hash#merge(other) { |k, o, n| }: new combined hash.
fn cm_hash_merge(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let out = mrb_funcall(vm, Some(this.clone()), "dup", &[])?;
    if let Some(v) = merge_into(vm, &out, &other_hash(args)?, block_of(args).as_ref())? {
        return Ok(v);
    }
    Ok(out)
}

// Hash#merge!(other) { |k, o, n| }: merges into self.
fn cm_hash_merge_self(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    if let Some(v) = merge_into(vm, &this, &other_hash(args)?, block_of(args).as_ref())? {
        return Ok(v);
    }
    Ok(this)
}

// Hash#keep_if { |k, v| }: drops non-matching pairs in place; returns self.
fn cm_hash_keep_if(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let block = need_block(args)?;
    for (k, v) in pairs(&this)? {
        let duo = [k.clone(), v.clone()];
        let keep = match call_block_catch_break(vm, &block, &duo)? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(v) => v,
        };
        if !keep.is_truthy() {
            h::mrb_hash_delete(&this, k)?;
        }
    }
    Ok(this)
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let hash = class_rc(vm, "Hash")?;
    native!(mrb_define_cmethod, vm, hash, "fetch", cm_hash_fetch);
    native!(mrb_define_cmethod, vm, hash, "key?", cm_hash_has_key);
    native!(mrb_define_cmethod, vm, hash, "member?", cm_hash_has_key);
    native!(mrb_define_cmethod, vm, hash, "value?", cm_hash_has_value);
    native!(mrb_define_cmethod, vm, hash, "dig", cm_hash_dig);
    native!(mrb_define_cmethod, vm, hash, "values_at", cm_hash_values_at);
    native!(
        mrb_define_cmethod,
        vm,
        hash,
        "transform_values",
        cm_hash_transform_values
    );
    native!(
        mrb_define_cmethod,
        vm,
        hash,
        "transform_keys",
        cm_hash_transform_keys
    );
    native!(mrb_define_cmethod, vm, hash, "invert", cm_hash_invert);
    native!(mrb_define_cmethod, vm, hash, "store", cm_hash_store);
    native!(mrb_define_cmethod, vm, hash, "each_pair", cm_hash_each_pair);
    native!(mrb_define_cmethod, vm, hash, "merge", cm_hash_merge);
    native!(mrb_define_cmethod, vm, hash, "merge!", cm_hash_merge_self);
    native!(mrb_define_cmethod, vm, hash, "keep_if", cm_hash_keep_if);
    Ok(())
}
