extern crate mrubyedge;

mod helpers;
use helpers::*;

#[test]
fn attr_reader_test() {
    let code = "
    class Hello
      attr_reader :world

      def update_world
        @world = 123
      end
    end

    def test_main
      w = Hello.new
      w.update_world
      w.world
    end
    ";
    let binary = mrbc_compile("attr_reader", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    // Assert
    let args = vec![];
    let result: i32 = mrb_funcall(&mut vm, None, "test_main", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, 123);
}

#[test]
fn attr_reader_2_test() {
    let code = "
    class Hello
      attr_reader :world
    end

    def test_main
      w = Hello.new
      w.world
    end
    ";
    let binary = mrbc_compile("attr_reader_2", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    // Assert
    let args = vec![];
    let result = mrb_funcall(&mut vm, None, "test_main", &args).unwrap();
    assert!(result.is_nil());
}

#[test]
fn attr_accessor_test() {
    let code = "
    class Hello
      attr_accessor :world
    end

    def test_main
      w = Hello.new
      w.world = \"Hola, attr\"
      w.world
    end
    ";
    let binary = mrbc_compile("attr_accessor", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    // Assert
    let args = vec![];
    let result: String = mrb_funcall(&mut vm, None, "test_main", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(&result, "Hola, attr");
}

#[test]
fn class_definition_isolation_test() {
    let code = "
    class Test1
      def hello
        123
      end
    end

    class Test2
      def hello
        456
      end
    end

    def test_main1
      Test1.new.hello
    end

    def test_main2
      Test2.new.hello
    end
    ";
    let binary = mrbc_compile("class_definition_isolation", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    // Assert
    let args = vec![];
    let val1: i32 = mrb_funcall(&mut vm, None, "test_main1", &args)
        .unwrap()
        .try_into()
        .unwrap();
    let val2: i32 = mrb_funcall(&mut vm, None, "test_main2", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(val1, 123);
    assert_eq!(val2, 456);
}

#[test]
fn class_inheritance_super_test() {
    let code = "
    class Test1
      def hello
        123
      end
    end

    class Test3 < Test1
      def hello
        super + 1
      end
    end

    def test_main
      Test3.new.hello
    end
    ";
    let binary = mrbc_compile("class_inheritance_super", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    // Assert
    let args = vec![];
    let result: i32 = mrb_funcall(&mut vm, None, "test_main", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, 124);
}

#[test]
fn class_define_class_method_test() {
    let code = "
    class Test
      def self.hello
        123
      end
    end

    def test_main
      Test.hello
    end
    ";
    let binary = mrbc_compile("class_define_class_method", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    // Assert
    let args = vec![];
    let result: i32 = mrb_funcall(&mut vm, None, "test_main", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, 123);
}

#[test]
fn class_inheritance_class_method_test() {
    let code = "
    class Test1
      def self.hello
        123
      end
    end

    class Test2 < Test1
      def self.hello
        super + 1
      end
    end

    def test_main
      Test2.hello
    end
    ";
    let binary = mrbc_compile("class_inheritance_class_method", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    // Assert
    let args = vec![];
    let result: i32 = mrb_funcall(&mut vm, None, "test_main", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, 124);
}

#[test]
fn class_can_have_singleton_instance_variables() {
    let code = r#"
    class Hello
      def self.set_world(value)
        @world = value
      end

      def self.get_world
        @world
      end
    end

    def test_main_0
      Hello.get_world
    end

    def test_main_1
      Hello.set_world("hello")
      Hello.get_world
    end
    "#;
    let binary = mrbc_compile("class_singleton_ivar", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    let args = vec![];
    let result = mrb_funcall(&mut vm, None, "test_main_0", &args).unwrap();
    assert!(result.is_nil());

    let result = mrb_funcall(&mut vm, None, "test_main_1", &args).unwrap();
    let value: String = result.try_into().expect("get_world should return string");
    assert_eq!(value, "hello");
}

#[test]
fn attr_accessor_independent_instances() {
    let code = r#"
    class Hello
      attr_accessor :world
    end

    def test_main
      a = Hello.new
      b = Hello.new
      a.world = "one"
      b.world = "two"
      a.world + ":" + b.world
    end
    "#;
    let binary = mrbc_compile("attr_independent", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    let result: String = mrb_funcall(&mut vm, None, "test_main", &[])
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, "one:two");
}

#[test]
fn attr_writer_returns_value() {
    let code = r#"
    class Hello
      attr_accessor :world
    end

    def test_main
      h = Hello.new
      r = (h.world = 7)
      r
    end
    "#;
    let binary = mrbc_compile("attr_writer_ret", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    let result: i32 = mrb_funcall(&mut vm, None, "test_main", &[])
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, 7);
}

#[test]
fn attr_accessor_multiple_symbols() {
    let code = r#"
    class Point
      attr_accessor :x, :y
    end

    def test_main
      p = Point.new
      p.x = 1
      p.y = 2
      p.x + p.y
    end
    "#;
    let binary = mrbc_compile("attr_multi", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    let result: i32 = mrb_funcall(&mut vm, None, "test_main", &[])
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, 3);
}
