mod helpers;

use helpers::*;
use mrubyedge::yamrb::vm::VM;

fn check(code: &'static str) {
    let binary = mrbc_compile("block_arguments", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = VM::open(&mut rite);
    let value = vm.run().unwrap();
    let result: bool = value.as_ref().try_into().unwrap();
    assert!(result, "unexpected result: {:?}", value.value);
}

#[test]
fn rest_arguments_and_named_block() {
    check(
        r#"
      def f(*args, &block)
        [args, block ? block.call : nil]
      end
      [f { 7 }, f(1) { 8 }, f(1, 2, 3) { 9 }, f(4, 5), f] ==
        [[[], 7], [[1], 8], [[1, 2, 3], 9], [[4, 5], nil], [[], nil]]
    "#,
    );
}

#[test]
fn optional_arguments_and_named_block() {
    check(
        r#"
      def f(a = 10, b = 20, &block)
        [a, b, block ? block.call : nil]
      end
      [f { 7 }, f(1) { 8 }, f(1, 2) { 9 }, f, f(3, 4)] ==
        [[10, 20, 7], [1, 20, 8], [1, 2, 9], [10, 20, nil], [3, 4, nil]]
    "#,
    );
}

#[test]
fn optional_rest_and_post_arguments() {
    check(
        r#"
      def f(first, option = 2, *rest, last, &block)
        [first, option, rest, last, block ? block.call : nil]
      end
      [f(1, 9) { 7 }, f(1, 3, 9) { 8 }, f(1, 3, 4, 5, 9) { 6 }, f(1, 9)] ==
        [[1, 2, [], 9, 7], [1, 3, [], 9, 8], [1, 3, [4, 5], 9, 6], [1, 2, [], 9, nil]]
    "#,
    );
}

#[test]
fn keywords_and_block() {
    check(
        r#"
      def f(a = 1, *rest, key: 2, **keywords, &block)
        [a, rest, key, keywords, block ? block.call : nil]
      end
      [f { 7 }, f(3, 4, 5, key: 6, other: 8) { 9 }, f(key: 10)] ==
        [[1, [], 2, {}, 7], [3, [4, 5], 6, {other: 8}, 9], [1, [], 10, {}, nil]]
    "#,
    );
}

#[test]
fn keywords_without_keyword_rest_and_block() {
    check(
        r#"
      def f(a = 1, key: 2, &block)
        [a, key, block ? block.call : nil]
      end
      [f { 7 }, f(3, key: 4) { 8 }, f(key: 5)] == [[1, 2, 7], [3, 4, 8], [1, 5, nil]]
    "#,
    );
}

#[test]
fn yield_after_argument_normalization() {
    check(
        r#"
      def f(a = 1, *rest, key: 2)
        yield(a, rest, key)
      end
      [f { |a, r, k| [a, r, k] }, f(3, 4, 5, key: 6) { |a, r, k| [a, r, k] }] ==
        [[1, [], 2], [3, [4, 5], 6]]
    "#,
    );
}

#[test]
fn nested_and_escaped_yield() {
    check(
        r#"
      def f(a = 1, *rest)
        Proc.new { Proc.new { yield(a, rest) }.call }
      end
      saved = f(3, 4, 5) { |a, r| [a, r] }
      saved.call == [3, [4, 5]]
    "#,
    );
}

#[test]
fn nil_block_and_block_given() {
    check(
        r#"
      def f(a = 1, *rest, &block)
        [block_given?, block.nil?]
      end
      [f { 1 }, f, f(2, 3, &nil)] == [[true, false], [false, true], [false, true]]
    "#,
    );
}

#[test]
fn yield_without_block_raises_local_jump_error() {
    check(
        r#"
      def f(a = 1, *rest)
        yield
      end
      begin
        f
        false
      rescue LocalJumpError
        true
      end
    "#,
    );
}

#[test]
fn forwarded_and_symbol_blocks() {
    check(
        r#"
      def target(a = 3, &block)
        block.call(a)
      end
      def forward(*args, &block)
        target(4, &block)
      end
      [forward(1, 2) { |a| a + 5 }, target(&:to_s)] == [9, '3']
    "#,
    );
}

#[test]
fn native_funcall_does_not_inherit_caller_block() {
    let code = r#"
      def target(a = 10, &block)
        [a, block.nil?, block_given?]
      end
      def outer(&block)
        [probe, block_given?, block.call]
      end
      outer { 7 } == [[10, true, false], true, 7]
    "#;
    let binary = mrbc_compile("native_block_arguments", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = VM::open(&mut rite);
    let object = vm.object_class.clone();
    mrubyedge::yamrb::helpers::mrb_define_cmethod(
        &mut vm,
        object,
        "probe",
        Box::new(|vm, _| mrb_funcall(vm, None, "target", &[])),
    );
    let result = vm.run().unwrap();
    assert!(bool::try_from(result.as_ref()).unwrap());
}

#[test]
fn converted_block_preserves_keywords() {
    check(
        r#"
      class Callable
        def to_proc
          Proc.new { |n| n + 1 }
        end
      end
      def f(first, option = 3, *rest, key: 3, &block)
        [first, option, rest, block.call(key)]
      end
      f(1, 2, 4, 5, key: 8, &Callable.new) == [1, 2, [4, 5], 9]
    "#,
    );
}

#[test]
fn nil_block_is_allowed_by_no_block_parameter() {
    check(
        r#"
      def f(&nil)
        7
      end
      f(&nil) == 7
    "#,
    );
}

#[test]
fn ruby_each_receives_native_block_with_optional_arguments() {
    check(
        r#"
      class Collection
        include Enumerable
        def each(start = 1, *rest, &block)
          if block_given?
            block.call(start)
            yield(start + 1)
          end
        end
      end
      Collection.new.map { |n| n * 3 } == [3, 6]
    "#,
    );
}

#[test]
fn method_arity_does_not_count_the_block() {
    check(
        r#"
      def f(first, option = 2, last, &block)
        [first, option, last, block.call]
      end
      short = begin
        f(1) { 7 }
        false
      rescue ArgumentError
        true
      end
      long = begin
        f(1, 2, 3, 4) { 7 }
        false
      rescue ArgumentError
        true
      end
      [short, long, f(1, 9) { 7 }] == [true, true, [1, 2, 9, 7]]
    "#,
    );
}

#[test]
fn proc_positional_argument_is_not_a_block() {
    check(
        r#"
      def f(value, &block)
        [value.call, block.nil?, block_given?]
      end
      f(Proc.new { 7 }) == [7, true, false]
    "#,
    );
}

#[test]
fn invalid_block_conversion_raises_type_error() {
    check(
        r#"
      class InvalidBlock
        def to_proc
          nil
        end
      end
      def f(&block)
        false
      end
      begin
        f(&InvalidBlock.new)
      rescue TypeError
        true
      end
    "#,
    );
}
