# PBLang — Rust compiler
## Requirements Document and Project Plan

**Student:** [Pedro Belém]
**Discipline:** Compilers
**Teacher:** Sheila Tirony
**Start:** 05/18/2026 · **Delivery:** 06/08/2026 · **Duration:** 21 days

---

## 1. Overview

The **PBLang** project consists of the development of a functional compiler for an imperative programming language for didactic purposes, implemented in **Rust**. The compilation pipeline goes through the classic phases: lexical, syntactic, semantic analysis, IR generation via LLVM (Inkwell) and production of a native executable.

---

## 2. Language Specification

### 2.1 Lexical and Grammatical Syntax

```
program → declaration* command*
declaration → 'var' ID ':' type ';'
type → 'int' | 'bool'

block → '{' command* '}'
command → assignment | stmt_if | stmt_while | stmt_print | stmt_read | block

assignment → ID '=' expression ';'
stmt_if → 'if' '(' expression ')' block ('else' block)?
stmt_while → 'while' '(' expression ')' block
stmt_print → 'print' '(' expression ')' ';'
stmt_read → 'read' '(' ID ')' ';'

expression → exp_or
exp_or → exp_and ('||' exp_and)*
exp_and → exp_equality ('&&' exp_equality)*
equality_exp → relational_exp (('==' | '!=') relational_exp)*
relational_exp → additive_exp (('>' | '<' | '>=' | '<=') additive_exp)*
additive_exp → multiplicative_exp (('+' | '-') multiplicative_exp)*
multiplicative_exp → unaria_exp (('*' | '/' | '%') unaria_exp)*
exp_unaria → ('!' | '-') exp_unaria | primary
primary → INTEGER | 'true' | 'false' | ID | '(' expression ')'

comments → '//' to end of line
```

### 2.2 Recognized Tokens

| Category | Examples |
|---|---|
| Reserved words | `int`, `bool`, `true`, `false`, `if`, `else`, `while`, `print`, `read` |
| Identifiers | `[a-zA-Z_][a-zA-Z0-9_]*` |
| Integer literals | `[0-9]+` |
| Arithmetic operators | `+`, `-`, `*`, `/`, `%` |
| Relational operators | `>`, `<`, `>=`, `<=` |
| Equality operators | `==`, `!=` |
| Logical operators | `&&`, `\|\|`, `!` |
| Delimiters | `(`, `)`, `{`, `}`, `;`, `:`, `=` |
| Comments | `// text` (ignored) |
| Blanks | space, tab, newline (ignored) |

---

## 3. Semantic Rules

### 3.1 Scope and Declaration
- Every variable (`ID`) must be declared with `int` or `bool` before any reading or assignment.
- There are no nested scopes: all declarations are global to the main function.

### 3.2 Type Checking

| Context | Rule | Resulting Type |
|---|---|---|
| `stmt_if` / `stmt_while` | Condition expression **must** be `bool` | — |
| `+`, `-`, `*`, `/`, `%`, `-` unary | Both `int` | `int` |
| `>`, `<`, `>=`, `<=` | Both `int` | `bool` |
| `==`, `!=` | Operands of **identical types** | `bool` |
| `&&`, `\|\|`, `!` unary | `bool` operands | `bool` |
| Attribution | Expression type == declared type of `ID` | — |

---

## 4. Compiler Architecture

### 4.1 Directory Structure

```
pbelang/
├── Cargo.toml
└── src/ 
├── main.rs ← reads .pb file, invokes full pipeline 
├── error/ ← unified error types (lexical, syntactic, semantic) 
├── lexer/ ← tokens + scanner via logos 
├── ast/ ← definition of AST structs/enums 
├── parser/ ← descending recursive parser 
├── symbol_table/ ← symbol table (HashMap<String, Type>) 
├── semantic/ ← type checking and scope 
└── codegen/ ← LLVM IR generation via inkwell
```

### 4.2 Compilation Pipeline

```
Source Code (.pb) 
↓ 
[Phase A] Lexer → Token sequence 
↓ 
[Phase B] Parser → Abstract Syntax Tree (AST) 
↓ 
[Phase C] Semantics → AST validated + Symbol Table 
↓ 
[Phase D] Codegen → LLVM IR in Memory (via Inkwell) 
↓ 
[Phase E] LLVM Backend → Optimization (opt) + Native executable
```

### 4.3 Rust Dependencies

```toml
[dependencies]
logos = "0.16.1" # Phase A — tokenization via DFA
inkwell = { git = "[https://github.com/TheDan64/inkwell](https://github.com/TheDan64/inkwell)", branch = "master", features = ["llvm18-1"] } # D/E Phases — LLVM IR

---

## 5. Code Generation Guidelines (Phases D and E)

### 5.1 AST → LLVM IR Mapping

All code generation is done **directly in memory** via Inkwell, without an intermediate TAC textual step.

| AST Construction | LLVM IR Instruction |
|---|---|
| `var x : int` | `allocates i32` |
| `var b : bool` | `allocate i1` |
| `x = expr` | `store <value>, <pointer>` |
| `ID` in expression | `load <type>, <pointer>` |
| `+`, `-`, `*`, `/`, `%` | `add`, `sub`, `mul`, `sdiv`, `srem` |
| `>`, `<`, `>=`, `<=` | `icmp sgt/slt/sge/sle` |
| `==`, `!=` | `icmp eq/ne` |
| `&&`, `\|\|` | `and`, `or` |
| `!` | `xor <val>,
