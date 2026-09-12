//! Hash index fast path (`OP_GETIDX`/`OP_SETIDX`): the pristine `Hash#[]` and
//! `Hash#[]=` run without a method send, but a redefinition or a subclass
//! override must still be honoured.

extern crate mec_mrbc_sys;
extern crate mrubyedge;

mod helpers;

use helpers::*;
use mrubyedge::yamrb::value::Value;

fn run(name: &'static str, code: &'static str) -> Value {
    let binary = mrbc_compile(name, code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    mrb_funcall(&mut vm, None, "run", &[]).unwrap()
}

/// Pristine Hash reads/writes go through the fast path and stay correct.
#[test]
fn hash_index_basic() {
    let result = run(
        "hash_index_basic",
        r#"
        def run
          h = {}
          h["a"] = 1
          h["b"] = 2
          h["a"] = 3
          [h["a"], h["b"], h["missing"]]
        end
        "#,
    );
    let v: Vec<Value> = (&result).try_into().unwrap();
    assert_eq!(i64::try_from(&v[0]).unwrap(), 3);
    assert_eq!(i64::try_from(&v[1]).unwrap(), 2);
    assert!(v[2].is_nil());
}

/// A redefined `Hash#[]` disables the read fast path.
#[test]
fn redefined_hash_index_is_respected() {
    let result = run(
        "hash_index_redefined",
        r#"
        class Hash
          def [](_k)
            :custom_get
          end
        end
        def run
          h = {}
          h["a"]
        end
        "#,
    );
    let s: String = (&result).try_into().unwrap();
    assert_eq!(s, "custom_get");
}

/// A redefined `Hash#[]=` disables the write fast path.
#[test]
fn redefined_hash_aset_is_respected() {
    let result = run(
        "hash_aset_redefined",
        r#"
        class Hash
          def []=(_k, _v)
            $TOUCHED = 42
          end
        end
        def run
          h = {}
          h["a"] = 1
          $TOUCHED
        end
        "#,
    );
    assert_eq!(i64::try_from(&result).unwrap(), 42);
}

/// Deleting and re-adding through the fast path stays consistent.
#[test]
fn hash_index_delete_and_reinsert() {
    let result = run(
        "hash_index_delete",
        r#"
        def run
          h = {}
          h["k"] = 1
          h.delete("k")
          a = h["k"]
          h["k"] = 2
          [a, h["k"], h.size]
        end
        "#,
    );
    let v: Vec<Value> = (&result).try_into().unwrap();
    assert!(v[0].is_nil());
    assert_eq!(i64::try_from(&v[1]).unwrap(), 2);
    assert_eq!(i64::try_from(&v[2]).unwrap(), 1);
}
