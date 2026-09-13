# mruby/edge — Built-in Class & Method Coverage

A list of currently supported classes and methods, based on the implementations in `mrubyedge/src/yamrb/prelude/`.

> **Legend**
> - `.method` — class method (`self.method`)
> - `#method` — instance method
> - `(alias: x)` — also available under this name
> - `[feature: xxx]` — requires the corresponding Cargo feature flag

---

## Object (base of all classes)

`prelude/object.rs`

### Instance methods

| Method | Notes |
|---|---|
| `#initialize` | |
| `#==` | |
| `#!=` | |
| `#===` | |
| `#object_id` | alias: `__id__` |
| `#to_s` | |
| `#inspect` | |
| `#raise` | |
| `#nil?` | |
| `#lambda` | alias: `proc` |
| `#is_a?` | alias: `kind_of?` |
| `#class` | |
| `#<=>` | |
| `#method_missing` | |
| `#extend` | |
| `#loop` | |
| `#block_given?` | |
| `#respond_to?` | |
| `#public_send` | |
| `#wasm?` | mruby/edge specific |
| `#puts` | `[feature: wasi]` only |
| `#p` | `[feature: wasi]` only |
| `#debug` | `[feature: wasi]` only |

### Predefined constants

| Constant | Value |
|---|---|
| `RUBY_VERSION` | VM VERSION string |
| `MRUBY_VERSION` | same as above |
| `MRUBY_EDGE_VERSION` | same as above |
| `RUBY_ENGINE` | VM ENGINE string |

---

## Exception hierarchy

`prelude/exception.rs`

Defined classes (with inheritance):

```
Exception
├── InternalError
├── NoMemoryError
├── ScriptError
│   └── LoadError
├── SyntaxError
├── SignalException
│   └── Interrupt
├── SystemExit
├── SystemStackError
└── StandardError
    ├── RuntimeError
    ├── TypeError
    ├── ArgumentError
    ├── RangeError
    ├── ZeroDivisionError
    ├── NotImplementedError
    ├── SecurityError
    ├── SystemCallError
    ├── NoMethodError
    └── NameError
```

### Instance methods (Exception)

| Method | Notes |
|---|---|
| `#message` | |

---

## Module

`prelude/module.rs`

| Method | Notes |
|---|---|
| `#include` | |
| `#ancestors` | |

---

## Class (subclass of Module)

`prelude/class.rs`

| Method | Notes |
|---|---|
| `#new` | creates a new instance |
| `#attr_reader` | |
| `#attr_writer` | |
| `#attr_accessor` | alias: `attr` |
| `#ancestors` | |
| `#inspect` | defined on the Module side |

---

## Integer

`prelude/integer.rs`

| Method | Notes |
|---|---|
| `#[]` | bit reference |
| `#-@` | unary minus |
| `#+` | mixed arithmetic with Float |
| `#-` | mixed arithmetic with Float |
| `#**` | mixed arithmetic with Float |
| `#%` | |
| `#&` | bitwise AND |
| `#\|` | bitwise OR |
| `#^` | bitwise XOR |
| `#~` | bitwise NOT |
| `#<<` | left shift |
| `#>>` | right shift |
| `#abs` | |
| `#to_i` | |
| `#to_f` | |
| `#chr` | |
| `#times` | takes a block |
| `#inspect` | alias: `to_s` |
| `#clamp` | |

---

## Float

`prelude/float.rs`

| Method | Notes |
|---|---|
| `#to_i` | |
| `#to_f` | |
| `#+` | mixed arithmetic with Integer |
| `#-` | mixed arithmetic with Integer |
| `#*` | mixed arithmetic with Integer |
| `#/` | mixed arithmetic with Integer |
| `#+@` | unary plus |
| `#-@` | unary minus |
| `#**` | mixed arithmetic with Integer |
| `#abs` | |
| `#inspect` | alias: `to_s` |
| `#clamp` | |

---

## NilClass

`prelude/nilclass.rs`

| Method | Notes |
|---|---|
| `#to_s` | returns `""` |
| `#inspect` | returns `"nil"` |
| `#nil?` | returns `true` |

---

## TrueClass

`prelude/trueclass.rs`

| Method | Notes |
|---|---|
| `#to_s` | alias: `inspect` |
| `#&` | |
| `#\|` | |
| `#^` | |

---

## FalseClass

`prelude/falseclass.rs`

| Method | Notes |
|---|---|
| `#to_s` | alias: `inspect` |
| `#&` | |
| `#\|` | |
| `#^` | |

---

## Symbol

`prelude/symbol.rs`

| Method | Notes |
|---|---|
| `#to_s` | |
| `#inspect` | `:sym` format |
| `#to_proc` | converts symbol to a proc that calls the method |

---

## Proc

`prelude/proc.rs`

| Method | Notes |
|---|---|
| `.new` | class method |
| `#call` | |

---

## String

`prelude/string.rs`

| Method | Notes |
|---|---|
| `.new` | class method |
| `#+` | string concatenation |
| `#*` | repetition |
| `#<<` | destructive append |
| `#[]` | alias: `slice` |
| `#[]=` | cf. `slice!` |
| `#b` | returns a binary (byte) string |
| `#clear` | |
| `#chomp` | |
| `#chomp!` | |
| `#dup` | |
| `#empty?` | |
| `#getbyte` | |
| `#setbyte` | |
| `#index` | |
| `#ord` | |
| `#slice` | |
| `#slice!` | |
| `#split` | |
| `#lstrip` | |
| `#lstrip!` | |
| `#rstrip` | |
| `#rstrip!` | |
| `#strip` | |
| `#strip!` | |
| `#to_sym` | alias: `intern` |
| `#start_with?` | |
| `#end_with?` | |
| `#include?` | |
| `#bytes` | |
| `#chars` | |
| `#upcase` | |
| `#upcase!` | |
| `#downcase` | |
| `#downcase!` | |
| `#to_i` | |
| `#to_f` | |
| `#unpack` | pack format: `Q q L l I i S s C c` |
| `#size` | alias: `bytesize`, `length` |
| `#inspect` | |
| `#to_s` | |
| `#=~` | added by `[feature: mruby-regexp]` |
| `#!~` | added by `[feature: mruby-regexp]` |

---

## Enumerable (module)

`prelude/enumerable.rs`  
Included in Array, Hash, and Range.

| Method | Notes |
|---|---|
| `#map` | |
| `#find` | |
| `#select` | |
| `#all?` | |
| `#any?` | |
| `#delete_if` | |
| `#each_with_index` | |
| `#sort` | |
| `#sort_by` | |
| `#max` | |
| `#min` | |
| `#minmax` | |
| `#compact` | |
| `#count` | |
| `#to_a` | |
| `#uniq` | |
| `#reduce` | |
| `#sum` | |

---

## Array

`prelude/array.rs`  
Includes Enumerable.

| Method | Notes |
|---|---|
| `.new` | class method |
| `#+` | returns a new array containing elements from both arrays |
| `#push` | alias: `<<` |
| `#[]` | alias: `at` |
| `#[]=` | |
| `#clear` | |
| `#delete_at` | |
| `#each` | |
| `#empty?` | |
| `#size` | alias: `length` |
| `#include?` | |
| `#&` | set intersection |
| `#\|` | set union |
| `#first` | |
| `#last` | |
| `#pop` | |
| `#shift` | |
| `#unshift` | |
| `#dup` | |
| `#uniq!` | |
| `#map!` | |
| `#select!` | |
| `#reject!` | |
| `#sort!` | |
| `#sort_by!` | |
| `#pack` | format: `Q q L l I i S s C c` |
| `#inspect` | alias: `to_s` |
| `#join` | |
| `#flatten` | returns a new flattened array (recursive) |
| `#flatten!` | flattens self in place (recursive), returns self or nil |

---

## Hash

`prelude/hash.rs`  
Includes Enumerable.

| Method | Notes |
|---|---|
| `.new` | class method |
| `#[]` | |
| `#[]=` | |
| `#clear` | |
| `#dup` | |
| `#delete` | |
| `#empty?` | |
| `#has_key?` | |
| `#has_value?` | |
| `#key` | reverse lookup: value → key |
| `#keys` | |
| `#each` | block receives key and value |
| `#size` | alias: `length`, `count` |
| `#merge` | |
| `#merge!` | |
| `#to_h` | |
| `#values` | |
| `#inspect` | alias: `to_s` |
| `#flatten` | returns an array of [key1, value1, key2, value2, ...] |

---

## Range

`prelude/range.rs`  
Includes Enumerable. Integer ranges only.

| Method | Notes |
|---|---|
| `#include?` | supports Integer and Float arguments |
| `#each` | Integer ranges only |

---

## SharedMemory (mruby/edge specific)

`prelude/shared_memory.rs`  
A class for zero-copy sharing with WASM linear memory.

| Method | Notes |
|---|---|
| `.new` | takes a size in bytes |
| `#to_s` | |
| `#offset_in_memory` | alias: `to_i` — returns the memory offset (address) |
| `#[]` | range / index access |
| `#[]=` | |
| `#replace` | |
| `#read_by_size` | |

---

## Random `[feature: mruby-random]`

`prelude/rand.rs`  
Uses the XorShift PRNG.

| Method | Notes |
|---|---|
| `.new` | seed is optional |
| `.rand` | class method |
| `.srand` | class method |
| `#rand` | instance method |
| `#seed` | returns the current seed |

Added to Kernel (Object):

| Method | Notes |
|---|---|
| `#rand` | uses the global default RNG |

---

## Regexp `[feature: mruby-regexp]`

`prelude/regexp.rs`  
Uses the Rust `regex` crate.

| Method | Notes |
|---|---|
| `.new` | alias: `.compile` |
| `#=~` | returns match position or `nil` |
| `#!~` | |
| `#match` | returns a MatchData object |
| `#inspect` | |

### MatchData `[feature: mruby-regexp]`

| Method | Notes |
|---|---|
| `#[]` | capture group reference |

---

## Ruby stdlib compatibility layer `[feature: ruby-compat]`

`compat/*.rs` — implemented in Rust on top of the public VM API. Enable the
`ruby-compat` feature and call `mrubyedge::compat::register(&mut vm)`.

### Array

`compat/array.rs`

| Method | Notes |
|---|---|
| `#<=>` | lexicographic; `nil` for non-Array or incomparable elements |
| `#concat` | |
| `#fill` | |
| `#delete` | |
| `#index` | alias: `#find_index` |
| `#rindex` | |
| `#insert` | |
| `#reject` | |
| `#first` | optional count |
| `#last` | optional count |
| `#reverse` | |
| `#reverse!` | |
| `#reverse_each` | |
| `#rotate` | negative and overflow offsets |
| `#take` | |
| `#drop` | |
| `#values_at` | |
| `#dig` | negative indices; `nil` short-circuit |
| `#each_with_object` | |
| `#compact!` | returns self or `nil` |
| `#product` | n-ary cartesian combinations |
| `#zip` | `nil` padding |
| `#shuffle` | `[feature: mruby-random]` |
| `#sample` | `[feature: mruby-random]` |
| `#reject!` | returns self or `nil` |

### Comparable

`compat/comparable.rs`

| Method | Notes |
|---|---|
| `#<` | derived from `<=>` |
| `#<=` | derived from `<=>` |
| `#>` | derived from `<=>` |
| `#>=` | derived from `<=>` |
| `#==` | derived from `<=>` |
| `#between?` | |
| `#clamp` | |

Included in `Integer`, `Float`, `String` and `Symbol`.

### Hash

`compat/hash.rs`

| Method | Notes |
|---|---|
| `#fetch` | |
| `#key?` | alias: `#member?` |
| `#value?` | |
| `#dig` | |
| `#values_at` | |
| `#transform_values` | |
| `#transform_keys` | |
| `#invert` | |
| `#store` | |
| `#each_pair` | |
| `#merge` | conflict block supported |
| `#merge!` | conflict block supported |
| `#keep_if` | |

### Integer

`compat/numeric.rs`

| Method | Notes |
|---|---|
| `#upto` | |
| `#downto` | |
| `#step` | |
| `#even?` | |
| `#odd?` | |
| `#zero?` | |
| `#positive?` | |
| `#negative?` | |
| `#succ` | overflow-guarded |
| `#pred` | overflow-guarded |
| `#divmod` | remainder follows the divisor's sign |
| `#%` | alias: `#modulo`; floored semantics |
| `#fdiv` | |
| `#div` | |
| `#pow` | |
| `#digits` | |
| `#gcd` | |
| `#lcm` | |
| `#bit_length` | |
| `#coerce` | |
| `#round` | negative digit counts |

### Float

`compat/numeric.rs`

| Method | Notes |
|---|---|
| `#step` | |
| `#coerce` | |
| `#%` | alias: `#modulo`; floored semantics |
| `#divmod` | |
| `#remainder` | |
| `#round` | negative digit counts |
| `#floor` | negative digit counts |
| `#ceil` | negative digit counts |
| `#truncate` | negative digit counts |

### Math

`compat/math.rs`

| Method | Notes |
|---|---|
| `.sin` | |
| `.cos` | |
| `.tan` | |
| `.asin` | |
| `.acos` | |
| `.atan` | |
| `.atan2` | |
| `.sinh` | |
| `.cosh` | |
| `.tanh` | |
| `.sqrt` | |
| `.cbrt` | |
| `.exp` | |
| `.log` | optional base |
| `.log2` | |
| `.log10` | |
| `.hypot` | |
| `.pow` | |
| `.abs` | |
| `PI` | constant |
| `E` | constant |

### Object / Kernel

`compat/object_ext.rs`

| Method | Notes |
|---|---|
| `#instance_variable_get` | accepts `"x"`, `"@x"` or `:x` |
| `#instance_variable_set` | accepts bare or `@`-prefixed names |
| `#instance_variable_defined?` | |
| `#instance_variables` | |
| `#tap` | |
| `#then` | alias: `#yield_self` |
| `#send` | alias: `#__send__` |
| `#eql?` | |
| `#hash` | |
| `#freeze` | |
| `#frozen?` | |
| `#print` | |
| `#Integer` | conversion |
| `#Float` | conversion |
| `#String` | conversion |
| `Class.new` | constructs native exceptions for `Exception` subclasses |

### Proc

`compat/proc_ext.rs`

| Method | Notes |
|---|---|
| `#===` | alias of `#call`; case/when dispatch |
| `#to_proc` | identity |
| `#arity` | |

### Range

`compat/range_extras.rs`

| Method | Notes |
|---|---|
| `#begin` | |
| `#end` | |
| `#first` | optional count; exclusive ends honored |
| `#last` | optional count; exclusive ends honored |
| `#exclude_end?` | |
| `#cover?` | alias: `#===` |
| `#size` | |
| `#step` | overflow-guarded |

### String

`compat/string.rs`

| Method | Notes |
|---|---|
| `#%` | |
| `#gsub` | literal pattern and block replacement |
| `#sub` | literal pattern and block replacement |
| `#tr` | character sets with ranges |
| `#delete` | character sets with ranges |
| `#squeeze` | character sets with ranges |
| `#count` | character sets with ranges |
| `#reverse` | UTF-8 safe |
| `#ljust` | |
| `#rjust` | |
| `#center` | |
| `#swapcase` | |
| `#capitalize` | |
| `#casecmp` | |
| `#casecmp?` | |
| `#<=>` | byte ordering |
| `#chop` | |
| `#prepend` | |
| `#replace` | |
| `#concat` | |
| `#partition` | |
| `#each_char` | |
| `#each_byte` | |
| `#each_line` | |
| `#lines` | |
| `#hex` | |
| `#oct` | |
| `#succ` | |
| `Kernel#sprintf` | alias: `#format` |

### Symbol

`compat/symbol_ext.rs`

| Method | Notes |
|---|---|
| `#length` | alias: `#size` |
| `#upcase` | |
| `#downcase` | |
| `#capitalize` | |
| `#swapcase` | |
| `#succ` | |
| `#empty?` | |
| `#[]` | |
| `#<=>` | |
| `#casecmp` | |

### Exceptions

`compat/exceptions.rs`

| Class / Method | Notes |
|---|---|
| `KeyError` | `StandardError` subclass |
| `IndexError` | `StandardError` subclass |
| `StopIteration` | `StandardError` subclass |
| `LocalJumpError` | `StandardError` subclass |
| `FrozenError` | `StandardError` subclass |
| `IOError` | `StandardError` subclass |
| `Exception#initialize` | stores the message |
| `Exception#message` | |
| `Exception#to_s` | |
| `Exception#inspect` | |

---

## Notes

- Some arithmetic operators (`*`, `/`) for Integer are not defined as instance methods in this prelude; they are handled directly by the VM bytecode interpreter (`eval.rs`).
- Comparison operators (`<`, `<=`, `>`, `>=`) are similarly handled on the VM side.
- `String#=~` and `#!~` are only added when `[feature: mruby-regexp]` is enabled.
