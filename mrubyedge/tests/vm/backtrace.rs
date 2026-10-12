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
    assert_eq!(
        trace("def inner\n  raise 'boom'\nend\ndef outer\n  inner\nend\nouter\n"),
        [
            "trace.rb:2:in raise",
            "trace.rb:2:in inner",
            "trace.rb:5:in outer",
            "trace.rb:7"
        ]
    );
}

#[test]
fn runtime_error_backtrace() {
    assert_eq!(
        trace("def lookup\n  MissingConstant\nend\nlookup\n"),
        ["trace.rb:2:in lookup", "trace.rb:4"]
    );
}

#[test]
fn ensure_preserves_original_backtrace() {
    assert_eq!(
        trace("def fail\n  begin\n    raise 'boom'\n  ensure\n    1 + 2\n  end\nend\nfail\n"),
        ["trace.rb:3:in raise", "trace.rb:3:in fail", "trace.rb:8"]
    );
}

#[test]
fn native_block_boundary_retains_callers() {
    let lines = trace("def invoke\n  [1].each do |n|\n    raise 'boom'\n  end\nend\ninvoke\n");
    assert_eq!(lines[0], "trace.rb:3:in raise");
    assert!(
        lines.contains(&"trace.rb:3:in <block>".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"trace.rb:2:in invoke".to_string()),
        "{lines:?}"
    );
    assert_eq!(lines.last().unwrap(), "trace.rb:6");
}

#[test]
fn ruby_backtrace_and_explicit_reraise() {
    let binary = mrbc_compile_named_debug(
        "rescue.rb",
        r#"
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
    "#,
    );
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
    let binary = mrbc_compile_named_debug("definitions.rb", "def fail\n  raise 'boom'\nend\n");
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = VM::open(&mut rite);
    vm.run().unwrap();
    let binary = mrbc_compile_named_debug("caller.rb", "fail\n");
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    assert!(vm.eval_rite(&mut rite).is_err());
    assert_eq!(
        vm.exception.as_ref().unwrap().backtrace,
        [
            "definitions.rb:2:in raise",
            "definitions.rb:2:in fail",
            "caller.rb:1"
        ]
    );
}

#[test]
fn debug_info_is_optional_for_exceptions() {
    let binary = mrbc_compile("no_debug", "raise 'boom'");
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
    let binary = mrbc_compile_named_debug("host.rb", "def fail\n  raise 'boom'\nend\n");
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = VM::open(&mut rite);
    vm.run().unwrap();
    assert!(mrb_funcall(&mut vm, None, "fail", &[]).is_err());
    let trace = &vm.exception.as_ref().unwrap().backtrace;
    assert_eq!(trace[0], "host.rb:2:in raise");
    assert_eq!(trace[1], "host.rb:2:in fail");
}

#[test]
fn replacement_exception_gets_a_new_backtrace() {
    let lines = trace("begin\n  raise 'boom'\nensure\n  raise 'boom'\nend\n");
    assert_eq!(lines[0], "trace.rb:4:in raise");
}

#[test]
fn successful_native_block_does_not_leave_stale_frames() {
    let lines = trace("[1].each { |n| n + 1 }\nraise 'boom'\n");
    assert_eq!(lines, ["trace.rb:2:in raise", "trace.rb:2"]);
}
