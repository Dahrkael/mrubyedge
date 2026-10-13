mod helpers;
use helpers::mrb_funcall;
use mrubyedge::yamrb::vm::VM;

// Exercise both the outer run loop and a suspended native-to-Ruby boundary.
fn check(body: &str) {
    for native in [false, true] {
        let mut source = format!("def scenario\n{body}\nend\n");
        if !native {
            source.push_str("$result = scenario\n$completed = true\n$result\n");
        }
        let binary = unsafe {
            mruby_compiler2_sys::MRubyCompiler2Context::new()
                .compile(&source)
                .unwrap()
        };
        let mut rite = mrubyedge::rite::load(&binary).unwrap();
        let mut vm = VM::open(&mut rite);
        let value = if native {
            vm.run().unwrap();
            mrb_funcall(&mut vm, None, "scenario", &[]).unwrap()
        } else {
            let result = vm.run().unwrap();
            assert!(
                vm.globals.get("$completed").is_some_and(|v| v.is_truthy()),
                "caller did not continue"
            );
            result
        };
        assert!(
            bool::try_from(value.as_ref()).unwrap(),
            "native={native}: {:?}",
            value.value
        );
    }
}

#[test]
fn rescue_return_continues_caller() {
    check(
        r#"
      def capture
        begin
          raise 'boom'
        rescue RuntimeError
          return 7
        end
      end
      value = capture
      [value, :continued] == [7, :continued]
    "#,
    );
}

#[test]
fn rescue_return_inside_native_block_boundary() {
    check(
        r#"
      $trace = []
      def capture(&block)
        begin
          block.call
        rescue RuntimeError
          $trace << :rescued
          return 7
        ensure
          $trace << :ensured
        end
      end
      def wrapper(&block)
        block.call
        $trace << :wrapper
      end
      wrapper do
        $trace << capture { raise 'boom' }
        $trace << :caller
      end
      $trace << :top
      $trace == [:rescued, :ensured, 7, :caller, :wrapper, :top]
    "#,
    );
}

#[test]
fn nonlocal_return_runs_nested_ensures_but_not_rescues() {
    check(
        r#"
      $trace = []
      def f
        begin
          1.times do
            begin
              return 7
            rescue Exception
              $trace << :wrong_rescue
            ensure
              $trace << :inner
            end
          end
        ensure
          $trace << :outer
        end
        99
      end
      value = f
      [value, $trace] == [7, [:inner, :outer]]
    "#,
    );
}

#[test]
fn ensure_return_overrides_pending_return() {
    check(
        r#"
      def f
        begin
          return 7
        ensure
          return 9
        end
      end
      [f, :continued] == [9, :continued]
    "#,
    );
}

#[test]
fn ensure_exception_overrides_pending_return() {
    check(
        r#"
      def f
        begin
          return 7
        ensure
          raise 'ensure failure'
        end
      end
      message = begin
        f
        'wrong'
      rescue RuntimeError => e
        e.message
      end
      message == 'ensure failure'
    "#,
    );
}

#[test]
fn return_targets_the_defining_recursive_invocation() {
    check(
        r#"
      $trace = []
      def recurse(n, saved = nil)
        if n == 2
          saved = Proc.new { return 7 }
        end
        begin
          if n == 0
            saved.call
          else
            recurse(n - 1, saved)
          end
          $trace << :wrong_continuation
        ensure
          $trace << n
        end
      end
      value = recurse(2)
      [value, $trace] == [7, [0, 1, 2]]
    "#,
    );
}

#[test]
fn expired_return_target_raises_local_jump_error() {
    check(
        r#"
      def make
        Proc.new { return 7 }
      end
      saved = make
      result = begin
        saved.call
        false
      rescue LocalJumpError
        true
      end
      result
    "#,
    );
}

#[test]
fn lambda_return_and_nested_block_return_are_local() {
    check(
        r#"
      fn = -> {
        begin
          1.times { return 7 }
        ensure
          $lambda_ensure = true
        end
      }
      value = fn.call
      [value, $lambda_ensure, :continued] == [7, true, :continued]
    "#,
    );
}

#[test]
fn sibling_closures_survive_recursive_frames() {
    check(
        r#"
      def capture(n)
        a = Proc.new { n }
        b = Proc.new { n + 10 }
        inner = n > 0 ? capture(n - 1) : nil
        [a, b, inner]
      end
      saved = capture(1)
      [saved[0].call, saved[1].call, saved[2][0].call, saved[2][1].call] == [1, 11, 0, 10]
    "#,
    );
}

#[test]
fn return_from_ensure_overrides_loop_break() {
    check(
        r#"
      def f
        while true
          begin
            break
          ensure
            return 7
          end
        end
        99
      end
      f == 7
    "#,
    );
}

#[test]
fn loop_unwind_runs_nested_ensures_and_resumes_after_break() {
    check(
        r#"
      trace = []
      begin
        while true
          begin
            begin
              break
            ensure
              begin
                raise 'handled'
              rescue RuntimeError
                trace << :handled
              end
              trace << :inner
            end
          ensure
            trace << :outer
          end
        end
        trace << :continued
      ensure
        trace << :method
      end
      trace == [:handled, :inner, :outer, :continued, :method]
    "#,
    );
}

#[test]
fn break_from_ruby_yield_runs_ensure_and_continues_caller() {
    check(
        r#"
      $trace = []
      def yielding
        begin
          yield
          $trace << :wrong
        rescue Exception
          $trace << :wrong_rescue
        ensure
          $trace << :ensure
        end
      end
      value = yielding { break 7 }
      $trace << :caller
      [value, $trace] == [7, [:ensure, :caller]]
    "#,
    );
}

#[test]
fn lambda_break_returns_locally() {
    check(
        r#"
      fn = -> { break 7 }
      [fn.call, :continued] == [7, :continued]
    "#,
    );
}

#[test]
fn next_runs_only_the_ensures_it_leaves() {
    check(
        r#"
      trace = []
      i = 0
      begin
        while i < 2
          begin
            i += 1
            next
          ensure
            trace << i
          end
        end
        trace << :continued
      ensure
        trace << :outer
      end
      trace == [1, 2, :continued, :outer]
    "#,
    );
}

#[test]
fn ensure_return_overrides_pending_exception() {
    check(
        r#"
      def f
        begin
          raise 'discarded'
        ensure
          return 7
        end
      end
      f == 7
    "#,
    );
}

#[test]
fn kernel_lambda_return_and_break_are_local() {
    check(
        r#"
      $trace = []
      def yielding
        yield
      ensure
        $trace << :ensure
      end
      fn = lambda { yielding { return 7 } }
      value = fn.call
      other = lambda { break 9 }
      [value, other.call, $trace, :continued] == [7, 9, [:ensure], :continued]
    "#,
    );
}

#[test]
fn kernel_proc_retains_its_nonlocal_return_target() {
    check(
        r#"
      def f
        proc { return 7 }.call
        99
      end
      f == 7
    "#,
    );
}
