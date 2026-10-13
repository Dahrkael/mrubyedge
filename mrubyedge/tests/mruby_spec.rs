#[path = "mruby_spec/mod.rs"]
mod mruby_spec;

// These are compatibility probes, not a claim that all upstream specs pass.
macro_rules! upstream_specs {
    ($($name:ident => $file:literal),* $(,)?) => { $(
        #[test]
        #[ignore = "upstream mruby compatibility suite; run explicitly with --ignored"]
        fn $name() { mruby_spec::run($file, stringify!($name)); }
    )* };
}

upstream_specs! {
    spec_argumenterror => "argumenterror.rb",
    spec_array => "array.rb",
    spec_basicobject => "basicobject.rb",
    spec_bs_block => "bs_block.rb",
    spec_bs_literal => "bs_literal.rb",
    spec_class => "class.rb",
    spec_codegen => "codegen.rb",
    spec_comparable => "comparable.rb",
    spec_ensure => "ensure.rb",
    spec_enumerable => "enumerable.rb",
    spec_env => "env.rb",
    spec_exception => "exception.rb",
    spec_false => "false.rb",
    spec_float => "float.rb",
    spec_gc => "gc.rb",
    spec_hash => "hash.rb",
    spec_indexerror => "indexerror.rb",
    spec_integer => "integer.rb",
    spec_iterations => "iterations.rb",
    spec_kernel => "kernel.rb",
    spec_lang => "lang.rb",
    spec_literals => "literals.rb",
    spec_localjumperror => "localjumperror.rb",
    spec_methods => "methods.rb",
    spec_module => "module.rb",
    spec_nameerror => "nameerror.rb",
    spec_nil => "nil.rb",
    spec_nomethoderror => "nomethoderror.rb",
    spec_numeric => "numeric.rb",
    spec_object => "object.rb",
    spec_platform => "platform.rb",
    spec_proc => "proc.rb",
    spec_range => "range.rb",
    spec_rangeerror => "rangeerror.rb",
    spec_refinement => "refinement.rb",
    spec_regexperror => "regexperror.rb",
    spec_runtimeerror => "runtimeerror.rb",
    spec_standarderror => "standarderror.rb",
    spec_string => "string.rb",
    spec_superclass => "superclass.rb",
    spec_symbol => "symbol.rb",
    spec_syntax => "syntax.rb",
    spec_sysfail => "sysfail.rb",
    spec_true => "true.rb",
    spec_typeerror => "typeerror.rb",
    spec_unicode => "unicode.rb",
    spec_version => "version.rb",
    spec_vformat => "vformat.rb",
}
