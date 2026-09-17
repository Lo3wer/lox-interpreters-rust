# LLOX LLVM Frontend & JIT: Implementation Roadmap and Manual

This document provides a comprehensive, step-by-step engineering roadmap for taking `llox-llvm-jit` from its current prototype state to a fully conforming, high-performance Lox compiler and JIT engine powered by LLVM 22 and [Inkwell](https://crates.io/crates/inkwell).

---

## 1. Current State Assessment

### 1.1 What Is Currently Implemented
- **Lexer (`src/lexer.rs`)**: Complete. Scans full Lox lexical grammar into tokens, tracks line numbers and errors.
- **Parser (`src/parser.rs`)**: Complete. Recursive-descent parser producing full Lox AST (`Stmt` and `Expr`), including desugaring `for` loops into `while` loops and assigning unique integer IDs to all `Expr` nodes.
- **Resolver (`src/resolver.rs`)**: Complete. Performs static resolution pass, enforcing compile-time rules (e.g. no top-level `return`, no self-inheritance, no reading variable in its own initializer) and populating variable depth mappings (`locals: HashMap<Expr, usize>`).
- **Basic CodeGen (`src/codegen.rs`)**:
  - Initializes LLVM native target and target machine.
  - Constructs `llox_main` function (`fn() -> i32`).
  - Implements `LoxValue` as `{ i8, i64 }` (1-byte tag, 8-byte payload).
  - Implements literals: Number (`f64` bits), Bool, Nil, String (as global string literal pointer).
  - Implements `Expr::Grouping`.
  - Implements `Expr::Unary`: only `TokenType::Minus` without operand type checks.
  - Implements `Expr::Binary`: only `+`, `-`, `*`, `/` unconditionally cast to `f64` arithmetic without operand type checks.
  - Implements `Stmt::Print` via `extern "C" fn lox_print_value(v: LoxValue)`.
  - JIT execution via LLVM execution engine.

### 1.2 What Is Missing / Deficient
| Category | Missing Functionality | Impact |
| :--- | :--- | :--- |
| **Type Safety & Errors** | Dynamic type checks, error exit code `70`, line number tracking in IR | Invalid operations crash or misbehave rather than raising runtime errors |
| **Expressions** | `!`, `and`, `or`, `? :`, `<`, `<=`, `>`, `>=`, `==`, `!=`, string concat | Most Lox expressions fail compilation or evaluation |
| **Variables & Scope** | `Stmt::Var`, `Stmt::Expression`, `Stmt::Block`, `Expr::Variable`, `Expr::Assign` | No mutable state, scoping, or local/global variables |
| **Control Flow** | `Stmt::If`, `Stmt::While` | No branching or iteration |
| **Heap & Strings** | Heap-allocated strings, string concatenation, equality comparison | String manipulation not functional |
| **Functions** | `Stmt::Function`, `Expr::Call`, `Stmt::Return`, `clock()` native function | No user-defined procedures or recursion |
| **Closures** | Upvalue capture, environment escaping stack lifetime | Closures and lexical scoping across functions fail |
| **Object Orientation** | `Stmt::Class`, `Expr::Get`, `Expr::Set`, `this`, `super`, constructors | No classes, instances, methods, or inheritance |
| **Memory Management** | Allocation tracking (arena or GC) | Leaks memory on dynamically allocated objects |

---

## 2. Core Architecture & Runtime Design

### 2.1 Value Representation (`LoxValue`)
To represent dynamic Lox values in statically typed LLVM IR:

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LoxValue {
    pub tag: u8,
    pub bits: u64,
}

pub const TAG_NUMBER: u8       = 0;
pub const TAG_BOOL: u8         = 1;
pub const TAG_NIL: u8          = 2;
pub const TAG_STRING: u8        = 3; // bits: *const LoxString
pub const TAG_CLOSURE: u8       = 4; // bits: *const LoxClosure
pub const TAG_CLASS: u8         = 5; // bits: *const LoxClass
pub const TAG_INSTANCE: u8      = 6; // bits: *const LoxInstance
pub const TAG_BOUND_METHOD: u8  = 7; // bits: *const LoxBoundMethod
```

In LLVM IR, this corresponds to:
```llvm
%LoxValue = type { i8, i64 }
```

### 2.2 LLVM JIT & Runtime ABI Architecture
Compiled code interacts with runtime support functions written in Rust using `extern "C"` linkage. The JIT execution engine registers these via `execution_engine.add_global_mapping`:

```
┌─────────────────────────────────────────────────────────────────┐
│                    JIT-Compiled Code (LLVM IR)                  │
│                                                                 │
│  - Evaluates expressions (SSA registers & stack allocas)        │
│  - Checks dynamic tags (fast-path branches)                     │
│  - Executes control flow (br, br i1, phi)                       │
│  - Calls runtime functions on complex/heap operations           │
└───────────────────────────────┬─────────────────────────────────┘
                                │ Calls extern "C"
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Rust Runtime Subsystem                      │
│                                                                 │
│  - lox_runtime_error(line, message) -> terminates with exit 70  │
│  - lox_string_concat(val_a, val_b) -> *const LoxString          │
│  - lox_values_equal(val_a, val_b) -> bool                       │
│  - lox_get_global / lox_set_global / lox_define_global          │
│  - lox_call(callee, args, arg_count, line)                      │
│  - Heap allocation & garbage collection / memory arena          │
└─────────────────────────────────────────────────────────────────┘
```

### 2.3 Runtime Error Protocol
The Lox test runner requires:
1. Compile errors: stderr `[line N] Error ...`, exit code `65`.
2. Runtime errors: stderr line `[line N] Error: <msg>` (or message followed by `[line N]`), exit code `70`.
3. Non-error exits: exit code `0`.

In LLVM IR, when a type check fails:
```rust
pub fn emit_runtime_error(&self, line: usize, message: &str) -> Result<(), CodeGenError> {
    let fn_error = self.get_or_declare_runtime_error();
    let line_val = self.context.i32_type().const_int(line as u64, false);
    let msg_ptr = self.builder.build_global_string_ptr(message, "err_msg").unwrap();
    self.builder.build_call(fn_error, &[line_val.into(), msg_ptr.as_pointer_value().into()], "")?;
    self.builder.build_unreachable()?;
    Ok(())
}
```
And in `src/runtime.rs`:
```rust
#[no_mangle]
pub extern "C" fn lox_runtime_error(line: u32, message: *const libc::c_char) -> ! {
    let msg = unsafe { std::ffi::CStr::from_ptr(message).to_string_lossy() };
    eprintln!("[line {line}] Error: {msg}");
    std::process::exit(70);
}
```

---

## 3. Detailed Step-by-Step Implementation Plan

---

### Phase 1: Dynamic Type Safety, Runtime Errors, and Full Operators

#### Step 1.1: Runtime Error Infrastructure
- **Objective**: Provide a safe runtime trap for invalid operations that matches Lox error reporting and exit code 70.
- **Actions**:
  1. Add `lox_runtime_error(line: u32, msg: *const c_char) -> !` in `src/runtime.rs`.
  2. Map `lox_runtime_error` in `CodeGen::run()` using `add_global_mapping`.
  3. Create a helper method in `CodeGen`:
     ```rust
     fn build_check_type(
         &self,
         val: StructValue<'ctx>,
         expected_tag: u8,
         line: usize,
         error_msg: &str
     ) -> Result<(), CodeGenError>;
     ```
     This extracts the tag (`extract_value(0)`), compares it (`icmp eq`), and conditionally branches to an error block calling `lox_runtime_error` or continues to a merge block.

#### Step 1.2: Unary Operators & Truthiness
- **Lox Semantics**:
  - `!`: Falsey values are `nil` and `false`. All other values are truthy. Returns `LoxValue::Bool`.
  - `-`: Requires operand to be `TAG_NUMBER`. Otherwise runtime error: `"Operand must be a number."`.
- **Implementation**:
  - Implement `build_is_truthy(val: StructValue) -> IntValue<'ctx>`:
    - Check if tag == `TAG_NIL` (if true -> false).
    - Check if tag == `TAG_BOOL` and bits == 0 (if true -> false).
    - Else -> true.
  - Implement `TokenType::Bang` in `compile_expr`:
    - Truthiness check -> invert result (`xor 1`) -> pack into `TAG_BOOL`.
  - Implement `TokenType::Minus` in `compile_expr`:
    - Check operand tag == `TAG_NUMBER`, raise `"Operand must be a number."` on line `operator.line()` if not.
    - Negate float and pack into `TAG_NUMBER`.

#### Step 1.3: Binary Arithmetic Operators (`-`, `*`, `/`)
- **Lox Semantics**: Both operands must be numbers. Otherwise: `"Operands must be numbers."`.
- **Implementation**:
  - Verify `left.tag == TAG_NUMBER` and `right.tag == TAG_NUMBER`.
  - On mismatch, branch to error block with message `"Operands must be numbers."`.
  - On match, perform `fsub`, `fmul`, `fdiv` and return `TAG_NUMBER`. Note: Lox follows IEEE 754 float division (div by zero produces infinity/NaN without error).

#### Step 1.4: Binary Addition (`+`)
- **Lox Semantics**: Operands must be either two numbers or two strings. Otherwise: `"Operands must be two numbers or two strings."`.
- **Implementation**:
  - Generate basic blocks: `check_nums`, `check_strings`, `error_block`, `done`.
  - If both `TAG_NUMBER`: perform `fadd`.
  - If both `TAG_STRING`: call `lox_string_concat(left, right)` in runtime.
  - Otherwise: call `lox_runtime_error(line, "Operands must be two numbers or two strings.")`.

#### Step 1.5: Comparisons (`<`, `<=`, `>`, `>=`)
- **Lox Semantics**: Operands must be numbers. Otherwise: `"Operands must be numbers."`.
- **Implementation**:
  - Validate tags are `TAG_NUMBER`.
  - Emit `build_float_compare` (`OLT`, `OLE`, `OGT`, `OGE`).
  - Cast resulting `i1` to `i64` and pack as `TAG_BOOL`.

#### Step 1.6: Equality (`==`, `!=`)
- **Lox Semantics**:
  - `nil == nil` -> `true`.
  - Different types -> `false`.
  - Numbers equal if `f64` equal.
  - Bools equal if bits equal.
  - Strings equal if character sequences match (content comparison, not pointer comparison).
- **Implementation**:
  - Provide runtime helper `lox_values_equal(a: LoxValue, b: LoxValue) -> bool`.
  - Call from IR for general equality, or inline fast-path when tags differ.

**Unlocked Test Suites**: `test/bool/`, `test/nil/`, `test/number/`, `test/operator/`.

---

### Phase 2: Statements, Local Variables, and Global Variables

#### Step 2.1: `Stmt::Expression`
- **Implementation**: In `compile_stmt`:
  ```rust
  Stmt::Expression { expression } => {
      self.compile_expr(expression)?;
      Ok(())
  }
  ```

#### Step 2.2: Global Variable Storage
- **Architecture**: In Lox, globals can be referenced inside functions before they are declared in top-level order, as long as they are defined before invocation.
- **Implementation**:
  - Maintain a global environment in `src/runtime.rs`:
    ```rust
    static GLOBALS: Mutex<HashMap<String, LoxValue>> = Mutex::new(HashMap::new());
    ```
  - Provide C runtime helpers:
    - `lox_define_global(name: *const c_char, val: LoxValue)`
    - `lox_get_global(name: *const c_char, line: u32) -> LoxValue`: checks if key exists; if not, calls `lox_runtime_error(line, "Undefined variable '...'.")`.
    - `lox_set_global(name: *const c_char, val: LoxValue, line: u32)`: checks if key exists; if not, calls `lox_runtime_error(line, "Undefined variable '...'.")`.

#### Step 2.3: Local Variables & Block Scopes (`Stmt::Block`)
- **Architecture**:
  - Local variables in LLVM are best represented as stack allocations (`alloca`) in the function's entry block. LLVM's `mem2reg` optimization pass transforms allocas into SSA registers.
  - The Resolver computes the static scope depth for every `Expr::Variable` and `Expr::Assign`.
  - Maintain a symbol table in `CodeGen`:
    ```rust
    pub struct ScopeTable<'ctx> {
        // Stack of scopes: variable name -> PointerValue (alloca)
        locals: Vec<HashMap<String, PointerValue<'ctx>>>,
    }
    ```
- **Implementation**:
  - `Stmt::Block { statements }`:
    1. Push a new scope map.
    2. Compile each statement.
    3. Pop the scope map.
  - `Stmt::Var { name, initializer }`:
    1. Compile `initializer` expression.
    2. If top-level (no local scope): call `lox_define_global`.
    3. If local scope:
       - Allocate stack space: `let ptr = self.create_entry_block_alloca(name.lexeme())`.
       - Store value: `self.builder.build_store(ptr, val)`.
       - Record in current scope map: `scope.insert(name.lexeme().to_string(), ptr)`.
  - `Expr::Variable { name, id }`:
    - Look up in `locals` stack from innermost to outermost scope.
    - If found in local scopes: `self.builder.build_load(ptr, name.lexeme())`.
    - Else: call `lox_get_global(name.lexeme(), token.line())`.
  - `Expr::Assign { name, value, id }`:
    - Compile `value`.
    - Look up in local scopes: if found, `self.builder.build_store(ptr, val)` and return `val`.
    - Else: call `lox_set_global(name.lexeme(), val, token.line())` and return `val`.

**Unlocked Test Suites**: `test/variable/`, `test/assignment/`, `test/block/`.

---

### Phase 3: Control Flow & Logical Operators

#### Step 3.1: Logical `and` and `or` with Short-Circuiting
- **Lox Semantics**:
  - `a and b`: Evaluates `a`. If falsey, returns `a` immediately without evaluating `b`. If truthy, evaluates and returns `b`.
  - `a or b`: Evaluates `a`. If truthy, returns `a` immediately without evaluating `b`. If falsey, evaluates and returns `b`.
  - **Important**: Lox logical operators do not return a boolean; they return the operand value itself.
- **Implementation**:
  - Allocate a stack temporary: `let result_alloca = self.create_entry_block_alloca("logical_tmp")`.
  - Compile left expression, store to `result_alloca`.
  - Evaluate truthiness of left.
  - For `or`: if truthy, branch to `merge_bb`; if falsey, branch to `eval_right_bb`.
  - For `and`: if truthy, branch to `eval_right_bb`; if falsey, branch to `merge_bb`.
  - In `eval_right_bb`: compile right expression, store to `result_alloca`, branch to `merge_bb`.
  - In `merge_bb`: load and return `result_alloca`.

#### Step 3.2: Ternary Operator (`condition ? then_branch : else_branch`)
- **Implementation**: Similar to `if` expression. Test condition truthiness; branch to `then_bb` or `else_bb`; store result in temporary `alloca` or use `phi` node in `merge_bb`.

#### Step 3.3: `Stmt::If`
- **Implementation**:
  ```rust
  Stmt::If { condition, then_branch, else_branch } => {
      let cond_val = self.compile_expr(condition)?;
      let is_true = self.build_is_truthy(cond_val)?;

      let current_fn = self.current_function();
      let then_bb = self.context.append_basic_block(current_fn, "then");
      let else_bb = self.context.append_basic_block(current_fn, "else");
      let merge_bb = self.context.append_basic_block(current_fn, "if_merge");

      self.builder.build_conditional_branch(is_true, then_bb, else_bb)?;

      // Then branch
      self.builder.position_at_end(then_bb);
      self.compile_stmt(then_branch)?;
      self.builder.build_unconditional_branch(merge_bb)?;

      // Else branch
      self.builder.position_at_end(else_bb);
      if let Some(else_stmt) = else_branch {
          self.compile_stmt(else_stmt)?;
      }
      self.builder.build_unconditional_branch(merge_bb)?;

      self.builder.position_at_end(merge_bb);
  }
  ```

#### Step 3.4: `Stmt::While`
- **Implementation**:
  - Create basic blocks: `cond_bb`, `body_bb`, `after_bb`.
  - Branch from previous block to `cond_bb`.
  - In `cond_bb`: compile condition, check truthiness, conditional branch to `body_bb` or `after_bb`.
  - In `body_bb`: compile body statement, branch back to `cond_bb`.
  - In `after_bb`: set builder position for subsequent statements.
- *(Note: `for` loops are desugared into blocks containing a `while` loop by the parser, so supporting `Stmt::While` and `Stmt::Block` automatically completes `for` loop support).*

**Unlocked Test Suites**: `test/if/`, `test/while/`, `test/for/`, `test/logical_operator/`.

---

### Phase 4: Dynamic Strings & Heap Objects

#### Step 4.1: Heap-Allocated String Representation
```rust
#[repr(C)]
pub struct LoxString {
    pub length: usize,
    pub chars: *mut u8,
}
```
- In `src/runtime.rs`:
  - `lox_string_alloc(data: *const u8, len: usize) -> *mut LoxString`
  - `lox_string_concat(a: LoxValue, b: LoxValue) -> LoxValue`
  - `lox_string_equal(a: LoxValue, b: LoxValue) -> bool`
  - Update `lox_print_value` to extract string length and slice from `LoxString`.

#### Step 4.2: String Concatenation and Equality
- In `compile_expr` for `TokenType::Plus`: when operands are strings, call `lox_string_concat`.
- In `compile_expr` for `TokenType::EqualEqual`: call `lox_string_equal` when tags are `TAG_STRING`.

**Unlocked Test Suites**: `test/string/`.

---

### Phase 5: Functions, Call Stack, and Returns

#### Step 5.1: Function Definition (`Stmt::Function`)
- **Structure**:
  - In LLVM, each Lox function becomes a separate compiled LLVM function:
    ```llvm
    define %LoxValue @user_func(%LoxValue* %args, i32 %arg_count)
    ```
    or with individual typed arguments:
    ```llvm
    define %LoxValue @user_func(%LoxValue %arg0, %LoxValue %arg1)
    ```
- **Implementation**:
  1. Save current builder insertion point and enclosing function.
  2. Create function type in LLVM module: returns `%LoxValue`, parameters matching `params.len()`.
  3. Create basic block `entry`.
  4. For each parameter: allocate stack slot (`alloca`), store parameter value, insert into function's local scope table.
  5. Compile body statements.
  6. If function falls off the end without an explicit return: emit `ret { TAG_NIL, 0 }`.
  7. Restore enclosing builder state.
  8. Wrap the compiled function pointer and arity in a `LoxFunction` heap object:
     ```rust
     #[repr(C)]
     pub struct LoxFunction {
         pub name: *const c_char,
         pub arity: usize,
         pub code_ptr: *const (),
     }
     ```
  9. Pack pointer into `LoxValue` with `TAG_CLOSURE` (or `TAG_FUNCTION`) and bind in current scope/globals.

#### Step 5.2: Function Calls (`Expr::Call`)
- **Lox Semantics**:
  - Callee must be callable (a function, class, or bound method). If not: runtime error `"Can only call functions and classes."`.
  - Argument count must match arity. If not: runtime error `"Expected X arguments but got Y."`.
- **Implementation**:
  1. Compile callee expression.
  2. Compile argument expressions.
  3. Check callee tag == `TAG_CLOSURE` (or `TAG_FUNCTION`). If not -> runtime error on `paren.line()`.
  4. Load function arity and compare with argument count. If mismatch -> runtime error on `paren.line()`.
  5. Cast function pointer to LLVM function type and emit `call`.
  6. Return the resulting `LoxValue`.

#### Step 5.3: Return Statements (`Stmt::Return`)
- **Implementation**:
  - If `value` is present: compile expression -> emit `build_return(Some(&val))`.
  - If `value` is absent: emit `build_return(Some(&nil_val))`.
  - Any code following a return in the same block is dead code; create a dummy unlinked basic block to absorb any further instructions until the block closes.

#### Step 5.4: Native Functions (`clock()`)
- In `src/runtime.rs`, implement `lox_native_clock() -> f64`:
  ```rust
  #[no_mangle]
  pub extern "C" fn lox_native_clock() -> f64 {
      use std::time::{SystemTime, UNIX_EPOCH};
      SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64()
  }
  ```
- Register `clock` in global environment during runtime initialization.

**Unlocked Test Suites**: `test/function/`, `test/call/`, `test/return/`.

---

### Phase 6: Lexical Closures (Finalized Architecture: Boxed Heap Cells)

#### Step 6.1: The Boxed Heap Cell Architecture
Based on Strategy B, captured variables are stored directly in heap-allocated cells (`HeapCell`) instead of stack slots. This eliminates the need for open-upvalue linked lists, stack-to-heap migration, and return hooks, while integrating seamlessly with the Mark-and-Sweep GC.

```
       Is variable captured by any nested closure?
                     /          \
                   No            Yes
                  /                \
        Allocated on Stack     Allocated on Heap
       (LLVM SSA Register)     (HeapCell with ObjHeader)
```

#### Step 6.2: Struct Layouts in `src/runtime.rs`
```rust
#[repr(C)]
pub struct HeapCell {
    pub header: ObjHeader, // obj_type = OBJ_HEAP_CELL
    pub value: LoxValue,   // current value of the variable
}

#[repr(C)]
pub struct LoxClosure {
    pub header: ObjHeader, // obj_type = OBJ_CLOSURE
    pub function_ptr: *const (),
    pub arity: usize,
    pub cell_count: usize,
    pub cells: [*mut HeapCell; 0], // trailing array of pointers to captured HeapCells
}
```

In LLVM IR:
```llvm
%HeapCell = type { %ObjHeader, %LoxValue }
%LoxClosure = type { %ObjHeader, i8*, i64, i64, %HeapCell** }
```

#### Step 6.3: Static Capture Analysis in `src/resolver.rs`
1. In `Resolver`, track function nesting level as a counter (`function_depth: usize`).
2. Add a set to `Resolver`:
   ```rust
   pub captured_locals: HashSet<usize>, // AST IDs of Stmt::Var and param tokens that are captured
   ```
3. When resolving a variable reference in `resolve_local(name)`:
   - If the variable's declaration scope is in an outer function (`declaring_function_depth < self.function_depth`):
     - Insert the variable's declaration ID into `captured_locals`.
4. Expose `pub fn captured_locals(&self) -> &HashSet<usize>`.

#### Step 6.4: CodeGen for Captured vs Uncaptured Variables
In `CodeGen`:
1. **Variable Declaration (`Stmt::Var`)**:
   - If `!captured_locals.contains(&var_id)`:
     - Normal stack slot: `let ptr = self.create_entry_block_alloca("local")`.
     - `self.builder.build_store(ptr, init_val)`.
     - Record in scope map: `LocalBinding::Stack(ptr)`.
   - If `captured_locals.contains(&var_id)`:
     - Allocate a heap cell: call runtime helper `@lox_alloc_cell(init_val) -> %HeapCell*`.
     - Record in scope map: `LocalBinding::HeapCell(cell_ptr)`.

2. **Variable Access (`Expr::Variable`)**:
   - `LocalBinding::Stack(ptr)`: emit `build_load(ptr)`.
   - `LocalBinding::HeapCell(cell_ptr)`:
     - GEP into field 1 (`value`) of `%HeapCell`.
     - Emit `build_load` to retrieve the `LoxValue`.
   - Captured from enclosing function:
     - Load the $i$-th `*mut HeapCell` from the function's hidden `%env` parameter.
     - Load the `LoxValue` from `cell->value`.

3. **Variable Assignment (`Expr::Assign`)**:
   - `LocalBinding::Stack(ptr)`: emit `build_store(ptr, new_val)`.
   - `LocalBinding::HeapCell(cell_ptr)`: GEP to `cell->value` and emit `build_store`.
   - Captured from enclosing function: load $i$-th cell from `%env` and emit `build_store`.

#### Step 6.5: Instantiating Closures at Runtime
1. When compiling `Stmt::Function`:
   - Identify the list of `HeapCell` pointers that this function captures from the surrounding scope.
   - Emit an array of `%HeapCell*` pointers in the caller's IR.
   - Call runtime helper:
     ```rust
     #[no_mangle]
     pub extern "C" fn lox_alloc_closure(
         fn_ptr: *const (),
         arity: usize,
         cells: *const *mut HeapCell,
         cell_count: usize,
     ) -> *mut LoxClosure
     ```
   - Pack the returned closure pointer into a `LoxValue` with `TAG_CLOSURE`.

#### Step 6.6: Garbage Collector Integration
In the GC Mark phase (Phase 9):
- When tracing `OBJ_CLOSURE`: for each cell in `closure.cells`, call `mark_object(cell)`.
- When tracing `OBJ_HEAP_CELL`: call `mark_value(cell.value)`.

**Unlocked Test Suites**: `test/closure/`.

---

### Phase 7: Classes, Instances, and Methods

#### Step 7.1: Object Model Structs
```rust
#[repr(C)]
pub struct LoxClass {
    pub name: *const c_char,
    pub superclass: *mut LoxClass,
    pub methods: *mut HashMap<String, LoxValue>, // mapping method name -> LoxClosure
}

#[repr(C)]
pub struct LoxInstance {
    pub class: *mut LoxClass,
    pub fields: *mut HashMap<String, LoxValue>,
}

#[repr(C)]
pub struct LoxBoundMethod {
    pub receiver: LoxValue, // Instance
    pub method: *mut LoxClosure,
}
```

#### Step 7.2: Class Declaration (`Stmt::Class`)
- Compile each method as a `LoxFunction`.
- Allocate `LoxClass` heap object.
- Populate class's method map.
- Bind class name in scope.

#### Step 7.3: Instantiation & Constructor
- Calling a class (`Expr::Call` on `TAG_CLASS`):
  1. Allocate new `LoxInstance`.
  2. If the class has an `init` method:
     - Bind `init` to the new instance.
     - Call `init` with provided arguments.
  3. Return the `LoxInstance` wrapped in `TAG_INSTANCE`.

#### Step 7.4: Property Access & Assignment (`Expr::Get`, `Expr::Set`)
- `Expr::Get { object, name }`:
  - Runtime helper: `lox_instance_get(object: LoxValue, name: *const c_char, line: u32) -> LoxValue`.
  - Checks if field exists on instance; if so, returns field.
  - If field does not exist, looks up method on class; if found, creates and returns a `LoxBoundMethod`.
  - If neither exists, calls `lox_runtime_error(line, "Undefined property '...'.")`.
- `Expr::Set { object, name, value }`:
  - Runtime helper: `lox_instance_set(object: LoxValue, name: *const c_char, val: LoxValue, line: u32) -> LoxValue`.
  - Inserts/updates field in instance's field map and returns `val`.

#### Step 7.5: Method Invocation & `this`
- When invoking a method or accessing `this`:
  - `this` is passed as argument 0 to the bound method.
  - In constructor `init`: the return value is always `this`, even if an empty `return;` is executed.

**Unlocked Test Suites**: `test/class/`, `test/field/`, `test/method/`, `test/constructor/`, `test/this/`.

---

### Phase 8: Inheritance and `super`

#### Step 8.1: Subclasses
- In `Stmt::Class`:
  - If `superclass` is specified, evaluate it.
  - Check that superclass has `TAG_CLASS`; otherwise raise runtime error `"Superclass must be a class."`.
  - Link subclass to superclass.
  - Inherited method resolution: if method not found on subclass, walk `class->superclass` chain.

#### Step 8.2: `Expr::Super`
- Compile-time resolution: the Resolver records the depth of `super` and `this`.
- At runtime:
  - Look up the superclass method on the statically resolved superclass.
  - Bind to the current `this` instance.
  - If method does not exist: raise runtime error `"Undefined property '...'.`".

**Unlocked Test Suites**: `test/inheritance/`, `test/super/`.

---

### Phase 9: Mark-and-Sweep Garbage Collection & REPL

#### Step 9.1: GC Architecture Overview in an LLVM JIT
Because LLVM JIT compiles directly to machine code, a garbage collector must solve two problems that interpreted virtual machines get for free:
1. **How to find all allocated objects?** -> Solved by an intrusive linked list of all heap objects (`all_objects`).
2. **How to find the root set (live references)?** ->
   - **Global Roots**: Stored in the runtime's global hash table (`GLOBALS`).
   - **Stack Roots**: Temporary registers and stack slots in active JIT functions. In LLVM, we track these using an explicit **Shadow Stack**.

---

#### Step 9.2: Intrusive Object Header (`ObjHeader`)
Every heap object begins with the identical 16-byte header:
```rust
pub const OBJ_STRING: u8       = 1;
pub const OBJ_CLOSURE: u8      = 2;
pub const OBJ_CLASS: u8        = 3;
pub const OBJ_INSTANCE: u8     = 4;
pub const OBJ_BOUND_METHOD: u8 = 5;
pub const OBJ_HEAP_CELL: u8    = 6;

#[repr(C)]
pub struct ObjHeader {
    pub is_marked: bool,
    pub obj_type: u8,
    pub next: *mut ObjHeader, // linked list connecting all active heap allocations
}
```

Whenever any object is allocated (e.g. `lox_string_alloc`, `lox_alloc_closure`, `lox_alloc_instance`, `lox_alloc_cell`):
```rust
fn allocate_raw(size: usize, obj_type: u8) -> *mut ObjHeader {
    // 1. Check if threshold exceeded, trigger GC if needed
    if GC_STATE.bytes_allocated > GC_STATE.next_gc {
        collect_garbage();
    }

    // 2. Allocate memory via libc::malloc or Rust Layout
    let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) as *mut ObjHeader };

    // 3. Initialize header and prepend to intrusive linked list
    unsafe {
        (*ptr).is_marked = false;
        (*ptr).obj_type = obj_type;
        (*ptr).next = GC_STATE.all_objects;
        GC_STATE.all_objects = ptr;
        GC_STATE.bytes_allocated += size;
    }
    ptr
}
```

---

#### Step 9.3: Shadow Stack for JIT Stack Roots
To allow the collector to find active values on the JIT execution stack without complex DWARF stack unwinding:
1. The runtime maintains a thread-local shadow stack of pointers to `LoxValue`:
   ```rust
   pub struct ShadowStack {
       pub roots: Vec<*const LoxValue>,
   }
   ```
2. In LLVM IR, before a function calls any runtime allocator or other function that might allocate, any live local variables that hold heap pointers are registered:
   ```llvm
   call void @lox_gc_push_root(%LoxValue* %local_alloca)
   ; ... call function or allocation ...
   call void @lox_gc_pop_roots(i32 1)
   ```
3. C runtime helpers:
   ```rust
   #[no_mangle]
   pub extern "C" fn lox_gc_push_root(slot: *const LoxValue) {
       GC_STATE.shadow_stack.push(slot);
   }

   #[no_mangle]
   pub extern "C" fn lox_gc_pop_roots(count: u32) {
       for _ in 0..count {
           GC_STATE.shadow_stack.pop();
       }
   }
   ```

---

#### Step 9.4: Mark Phase
1. **Trace Globals**: Walk every `(key, value)` in `GLOBALS`. If `value` has a heap tag (`TAG_STRING`, `TAG_CLOSURE`, `TAG_CLASS`, `TAG_INSTANCE`, etc.), call `mark_value(value)`.
2. **Trace Shadow Stack**: Walk every `*const LoxValue` currently on `shadow_stack`. Call `mark_value(*slot)`.
3. **Trace Children (Gray Stack / Worklist)**:
   - When marking an object, set `header.is_marked = true` and push it to a `gray_stack: Vec<*mut ObjHeader>`.
   - Pop objects from `gray_stack` and mark their references:
     - `OBJ_CLOSURE`: mark `closure.function`, and each `upvalue`/`heap_cell`.
     - `OBJ_INSTANCE`: mark `instance.class` and each value in `instance.fields`.
     - `OBJ_CLASS`: mark `class.superclass` and each closure in `class.methods`.
     - `OBJ_BOUND_METHOD`: mark `bound.receiver` and `bound.method`.
     - `OBJ_HEAP_CELL`: mark `cell.value`.
     - `OBJ_STRING`: no child pointers to trace.

---

#### Step 9.5: Sweep Phase
Walk the intrusive linked list `all_objects`:
```rust
unsafe fn sweep() {
    let mut prev: *mut *mut ObjHeader = &mut GC_STATE.all_objects;
    let mut curr = GC_STATE.all_objects;

    while !curr.is_null() {
        if (*curr).is_marked {
            // Survived: unmark for the next GC cycle
            (*curr).is_marked = false;
            prev = &mut (*curr).next;
            curr = (*curr).next;
        } else {
            // Unreached: unlink and free
            let unreached = curr;
            curr = (*curr).next;
            *prev = curr;
            free_object(unreached);
        }
    }

    // Adjust threshold for next collection
    GC_STATE.next_gc = GC_STATE.bytes_allocated * 2;
}
```

---

#### Step 9.6: REPL Mode Integration
- Persist global symbol table across REPL prompt lines.
- Expressions evaluated at top level in prompt mode print their results automatically.
- Reset the error flag between lines.

---

## 4. Summary of Test Milestone Tracking

You can track your manual implementation progress against the standard Lox test suite using `test-runner`:

```sh
# Build llox binary
cargo build --manifest-path llox-llvm-jit/Cargo.toml

# Run tests for a specific phase
cargo run --manifest-path test-runner/Cargo.toml -- \
  llox-llvm-jit/target/debug/llox-llvm-jit \
  test/operator

# Run full test suite skipping non-compiler tests
cargo run --manifest-path test-runner/Cargo.toml -- \
  llox-llvm-jit/target/debug/llox-llvm-jit \
  test \
  --skip test/scanning \
  --skip test/expressions \
  --skip test/limit \
  --skip test/benchmark
```

---

## 5. Architectural Decisions Finalized

1. **Memory Management**: **Mark-and-Sweep Garbage Collection** (detailed in Phase 9).
   - Intrusive object headers on all heap allocations (`ObjHeader`).
   - Shadow stack tracking for JIT stack roots.
   - Sweep phase frees dead objects and unlinks from the intrusive linked list.
2. **Global Variables**: **Runtime Hash Table via C-ABI** (detailed in Phase 2).
   - Dynamic late-binding compliant with Lox semantics and REPL persistence.
   - Access via `lox_get_global`, `lox_set_global`, `lox_define_global`.
3. **Closures**: **Strategy B (Boxed Heap Cells)** (detailed in Phase 6).
   - Compile-time captured analysis via `Resolver`.
   - Direct heap cell allocation for captured variables (`HeapCell`).
   - Clean LLVM IR with zero stack-escaping hooks on return, and seamless Mark-and-Sweep GC tracing.


