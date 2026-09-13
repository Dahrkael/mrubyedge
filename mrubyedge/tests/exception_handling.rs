extern crate mrubyedge;

mod helpers;
use helpers::*;

use mrubyedge::Error;

#[test]
fn a_handler_answers_for_the_body_that_raised_test() {
    let code = "
def run
  begin
    raise \"boom\"
  rescue => e
    e.message
  end
end

run
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "boom");
}

#[test]
fn a_body_that_does_not_raise_skips_the_handler_test() {
    let code = "
def run
  begin
    1 + 1
  rescue => e
    99
  end
end

run
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 2);
}

#[test]
fn ensure_runs_when_the_body_answers_test() {
    let code = "
$log = \"\"

def run
  begin
    1
  ensure
    $log = \"ensured\"
  end
end

run
$log
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "ensured");
}

#[test]
fn ensure_runs_when_the_body_raises_test() {
    let code = "
$log = \"\"

def run
  begin
    raise \"boom\"
  rescue => e
    \"rescued\"
  ensure
    $log = \"ensured\"
  end
end

run + \"/\" + $log
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "rescued/ensured");
}

#[test]
fn a_class_the_handler_does_not_name_is_not_rescued_test() {
    let code = "
def run
  begin
    raise \"boom\"
  rescue ArgumentError => e
    \"wrong handler\"
  end
end

run
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let err = vm.run().unwrap_err();
    let err = err.downcast_ref::<Error>().expect("a VM error");

    // Assert
    assert!(
        matches!(err, Error::RuntimeError(msg) if msg == "boom"),
        "{:?}",
        err
    );
}

#[test]
fn the_inner_handler_takes_the_exception_first_test() {
    let code = "
def run
  begin
    begin
      raise \"inner\"
    rescue => e
      \"inner:\" + e.message
    end
  rescue => e
    \"outer:\" + e.message
  end
end

run
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "inner:inner");
}

#[test]
fn raising_inside_a_handler_leaves_the_method_test() {
    let code = "
def run
  begin
    raise \"first\"
  rescue => e
    raise \"second\"
  end
end

run
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let err = vm.run().unwrap_err();
    let err = err.downcast_ref::<Error>().expect("a VM error");

    // Assert
    assert!(
        matches!(err, Error::RuntimeError(msg) if msg == "second"),
        "{:?}",
        err
    );
}

#[test]
fn an_exception_crosses_a_method_boundary_test() {
    let code = "
def inner
  raise \"deep\"
end

def outer
  begin
    inner
  rescue => e
    \"caught:\" + e.message
  end
end

outer
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "caught:deep");
}

#[test]
fn an_exception_raised_in_an_ensure_leaving_a_loop_is_rescued_test() {
    let code = "
def run
  i = 0
  while i < 3
    begin
      break
    ensure
      raise \"from ensure\"
    end
  end
  \"dropped\"
end

begin
  run
rescue => e
  \"p:\" + e.message
end
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "p:from ensure");
}
