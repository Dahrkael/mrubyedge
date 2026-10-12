use super::helpers::*;
use mrubyedge::yamrb::{value::RObject, vm::VM};
use std::rc::Rc;

fn trace(code: &str) -> Vec<String> {
    let binary = mrbc_compile_named_debug("trace.rb", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = VM::open(&mut rite);
    assert!(vm.run().is_err());
    vm.exception.as_ref().unwrap().backtrace.clone()
}

#[test]
fn nested_method_backtrace() {
    let code = r#"
    def inner
      raise 'boom'
    end
    def outer
      inner
    end
    outer
    "#;
    assert_eq!(
        trace(code),
        [
            "trace.rb:3:in raise",
            "trace.rb:3:in inner",
            "trace.rb:6:in outer",
            "trace.rb:8"
        ]
    );
}

#[test]
fn runtime_error_backtrace() {
    let code = r#"
    def lookup
      MissingConstant
    end
    lookup
    "#;
    assert_eq!(trace(code), ["trace.rb:3:in lookup", "trace.rb:5"]);
}

#[test]
fn ensure_preserves_original_backtrace() {
    let code = r#"
    def fail
      begin
        raise 'boom'
      ensure
        1 + 2
      end
    end
    fail
    "#;
    assert_eq!(
        trace(code),
        ["trace.rb:4:in raise", "trace.rb:4:in fail", "trace.rb:9"]
    );
}

#[test]
fn native_block_boundary_retains_callers() {
    let code = r#"
    def invoke
      [1].each do |n|
        raise 'boom'
      end
    end
    invoke
    "#;
    let lines = trace(code);
    assert_eq!(lines[0], "trace.rb:4:in raise");
    assert!(
        lines.contains(&"trace.rb:4:in <block>".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"trace.rb:3:in invoke".to_string()),
        "{lines:?}"
    );
    assert_eq!(lines.last().unwrap(), "trace.rb:7");
}

#[test]
fn ruby_backtrace_and_explicit_reraise() {
    let code = r#"
    def fail
      raise "boom"
    end
    begin
      begin
        fail
      rescue => error
        $original = error.backtrace
        raise error
      end
    rescue => error
      $reraised = error.backtrace
    end
    "#;
    let binary = mrbc_compile_named_debug("rescue.rb", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = VM::open(&mut rite);
    vm.run().unwrap();
    let original = vm.globals.get("$original").unwrap();
    let reraised = vm.globals.get("$reraised").unwrap();
    let strings = |obj: &Rc<RObject>| -> Vec<String> {
        let values: Vec<Rc<RObject>> = obj.as_ref().try_into().unwrap();
        values
            .iter()
            .map(|v| String::try_from(v.as_ref()).unwrap())
            .collect()
    };
    assert_eq!(strings(original), strings(reraised));
    assert_eq!(strings(original)[0], "rescue.rb:3:in raise");
}

#[test]
fn different_eval_files_keep_their_locations() {
    let code = r#"
    def fail
      raise 'boom'
    end
    "#;
    let binary = mrbc_compile_named_debug("definitions.rb", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = VM::open(&mut rite);
    vm.run().unwrap();
    let code = r#"
    fail
    "#;
    let binary = mrbc_compile_named_debug("caller.rb", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    assert!(vm.eval_rite(&mut rite).is_err());
    assert_eq!(
        vm.exception.as_ref().unwrap().backtrace,
        [
            "definitions.rb:3:in raise",
            "definitions.rb:3:in fail",
            "caller.rb:2"
        ]
    );
}

#[test]
fn debug_info_is_optional_for_exceptions() {
    let code = r#"
    raise 'boom'
    "#;
    let binary = mrbc_compile("no_debug", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = VM::open(&mut rite);
    assert!(vm.run().is_err());
    assert_eq!(
        vm.exception.as_ref().unwrap().backtrace,
        ["(unknown):0:in raise"]
    );
}

#[test]
fn host_funcall_keeps_method_source_location() {
    let code = r#"
    def fail
      raise 'boom'
    end
    "#;
    let binary = mrbc_compile_named_debug("host.rb", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = VM::open(&mut rite);
    vm.run().unwrap();
    assert!(mrb_funcall(&mut vm, None, "fail", &[]).is_err());
    let trace = &vm.exception.as_ref().unwrap().backtrace;
    assert_eq!(trace[0], "host.rb:3:in raise");
    assert_eq!(trace[1], "host.rb:3:in fail");
}

#[test]
fn replacement_exception_gets_a_new_backtrace() {
    let code = r#"
    begin
      raise 'boom'
    ensure
      raise 'boom'
    end
    "#;
    let lines = trace(code);
    assert_eq!(lines[0], "trace.rb:5:in raise");
}

#[test]
fn successful_native_block_does_not_leave_stale_frames() {
    let code = r#"
    [1].each { |n| n + 1 }
    raise 'boom'
    "#;
    let lines = trace(code);
    assert_eq!(lines, ["trace.rb:3:in raise", "trace.rb:3"]);
}
