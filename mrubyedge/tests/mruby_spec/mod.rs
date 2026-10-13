use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use mrubyedge::yamrb::{helpers::mrb_define_cmethod, value::RObject};
use mrubyedge::yamrb::{helpers::mrb_funcall, vm::VM};
use std::io::Write;
use std::rc::Rc;

mod pattern;

// Isolate VM panics, stack overflows, and nonterminating upstream cases.
pub fn run(file: &str, test: &str) {
    if std::env::var("MRUBYEDGE_SPEC_CHILD").as_deref() == Ok(test) {
        execute(file);
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", test, "--nocapture"])
        .env("MRUBYEDGE_SPEC_CHILD", test)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "{file}: child exited with {status}");
            return;
        }
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("{file}: exceeded 30 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn eval(vm: &mut VM, source: &str, label: &str) {
    let binary = unsafe {
        mruby_compiler2_sys::MRubyCompiler2Context::new()
            .compile(source)
            .unwrap_or_else(|e| panic!("{label}: compile failed: {e:?}"))
    };
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    vm.eval_rite(&mut rite)
        .unwrap_or_else(|e| panic!("{label}: {e}; backtrace: {:?}", vm.backtrace()));
}

fn execute(file: &str) {
    let mut vm = VM::empty();
    install(&mut vm);
    eval(&mut vm, include_str!("helpers.rb"), "helpers.rb");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/mruby_spec/upstream/t")
        .join(file);
    let source = std::fs::read_to_string(path).unwrap();
    eval(
        &mut vm,
        &format!("{source}\n$spec_completed = true\n"),
        file,
    );
    assert!(
        vm.globals
            .get("$spec_completed")
            .is_some_and(|v| v.is_truthy()),
        "{file}: VM returned before the end of the test file"
    );
    let result = mrb_funcall(&mut vm, None, "report", &[]).expect("report failed");
    assert!(result.is_truthy(), "{file}: upstream assertions failed");
}

fn install(vm: &mut VM) {
    mrb_define_cmethod(
        vm,
        vm.object_class.clone(),
        "t_print",
        Box::new(|_, args| {
            for arg in args {
                match &arg.value {
                    mrubyedge::yamrb::value::RValue::Nil => {}
                    mrubyedge::yamrb::value::RValue::String(s, _) => {
                        print!("{}", String::from_utf8_lossy(&s.borrow()))
                    }
                    mrubyedge::yamrb::value::RValue::Integer(i) => print!("{i}"),
                    _ => return Err(mrubyedge::Error::TypeMismatch),
                }
            }
            std::io::stdout().flush().unwrap();
            Ok(Rc::new(RObject::nil()))
        }),
    );
    mrb_define_cmethod(
        vm,
        vm.object_class.clone(),
        "_str_match?",
        Box::new(|_, args| {
            if args.len() != 2 {
                return Err(mrubyedge::Error::ArgumentError(
                    "expected two arguments".into(),
                ));
            }
            let pattern: String = args[0].as_ref().try_into()?;
            let value: String = args[1].as_ref().try_into()?;
            Ok(Rc::new(RObject::boolean(pattern::matches(
                &pattern, &value,
            ))))
        }),
    );
}

#[test]
fn assertion_driver() {
    let mut vm = VM::empty();
    install(&mut vm);
    eval(&mut vm, include_str!("helpers.rb"), "helpers.rb");
    eval(
        &mut vm,
        r#"
      assert('passing') do
        assert_equal 2, 1 + 1
        assert_equal(3) { 1 + 2 }
        assert_equal(3, 'block with message') { 1 + 2 }
        assert_not_equal(4) { 1 + 2 }
        error = assert_raise(RuntimeError) { raise 'expected' }
        assert_equal 'expected', error.message
        assert_raise(RuntimeError, ArgumentError, 'multiple classes') { raise 'expected' }
        assert_raise_with_message(RuntimeError, 'expected') { raise 'expected' }
        assert_raise_with_message_pattern(RuntimeError, 'exp*') { raise 'expected' }
        assert_nothing_raised('no exception') { 1 + 1 }
        assert_false false
        assert_match 'a{b,c}*', 'acdef'
      end
      assert('failing') { assert_equal 1, 2 }
      assert('exception') { raise 'unexpected' }
      assert('skipped') { skip 'reason' }
      assert('stress') { stress }
      assert('warning') { true }
      assert('wrong exception') do
        $wrong_exception_result = assert_raise(ArgumentError) { raise 'wrong' }
      end
      assert('missing exception') do
        $missing_exception_result = assert_raise(RuntimeError) { 1 }
      end
      $driver_completed = true
    "#,
        "driver self-test",
    );
    assert!(vm.globals["$driver_completed"].is_truthy());
    assert!(vm.globals["$wrong_exception_result"].is_nil());
    assert!(vm.globals["$missing_exception_result"].is_nil());
    for (name, expected) in [
        ("$ok_test", 1),
        ("$ko_test", 3),
        ("$kill_test", 1),
        ("$skip_test", 1),
        ("$stress_test", 1),
        ("$warning_test", 1),
    ] {
        let actual: i64 = vm.globals[name].as_ref().try_into().unwrap();
        assert_eq!(actual, expected, "{name}");
    }
    assert!(
        !mrb_funcall(&mut vm, None, "report", &[])
            .unwrap()
            .is_truthy()
    );
}
