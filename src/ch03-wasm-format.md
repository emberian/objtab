# WebAssembly Object Format

If ELF is the grizzled veteran—battle-tested, flexible, trusting—WebAssembly is the paranoid newcomer, designed by people who'd seen what happens when you trust code too much.

The web taught us hard lessons. JavaScript engines spent years hardening against malicious scripts. Browser sandboxes grew ever more complex. Even then, exploits slipped through. When the browser vendors sat down to design a binary format for the web, they asked: what if we built safety into the format itself? What if untrusted code couldn't misbehave, not because we caught it, but because the format made misbehavior impossible to express?

The result is WebAssembly: a binary format that's simultaneously lower-level than JavaScript (it compiles to native code) and safer (memory is sandboxed, control flow is structured, types are checked). It's a format where the things you *can't* do matter as much as the things you can.

WebAssembly wasn't designed to replace ELF. It was designed for the web. But somewhere along the way, it grew up and became a real compilation target—complete with its own object file format, linking conventions, and toolchain.

If you've compiled C or Rust to WASM, you've produced WASM object files. Let's understand what's inside them.

## WASM: Not Just a VM Bytecode

When WebAssembly first arrived, it looked like Java bytecode or .NET IL—a virtual machine instruction set. But WASM has a crucial difference: **it's designed to be fast to compile**.

JIT compilers can turn WASM into native code in milliseconds. This changes everything. WASM isn't interpreted; it's compiled. And that means WASM modules need the same machinery as native object files: imports, exports, relocations, and symbols.

## The WASM Binary Format

A WASM file is a **module**. Its structure:

```
┌─────────────────────────────────────┐
│         Magic Number (4 bytes)      │  0x00 0x61 0x73 0x6D ("\0asm")
├─────────────────────────────────────┤
│         Version (4 bytes)           │  0x01 0x00 0x00 0x00 (version 1)
├─────────────────────────────────────┤
│         Section 1 (Type)            │
├─────────────────────────────────────┤
│         Section 2 (Import)          │
├─────────────────────────────────────┤
│         Section 3 (Function)        │
├─────────────────────────────────────┤
│         ...                         │
├─────────────────────────────────────┤
│         Section N (Custom)          │
└─────────────────────────────────────┘
```

Each section has:
- A **section ID** (1 byte)
- A **size** (LEB128 encoded)
- **Content** (section-specific)

Let's examine the key sections.

## Section Types

### Type Section (ID: 1)

Defines function signatures:

```wat
(type $add_type (func (param i32 i32) (result i32)))
(type $print_type (func (param i32)))
```

Types are defined once and referenced by index. This saves space—a module with 100 functions might only have 10 unique signatures.

### Import Section (ID: 2)

External dependencies—the WASM equivalent of undefined symbols:

```wat
(import "env" "memory" (memory 1))
(import "env" "print" (func $print (type $print_type)))
(import "wasi_snapshot_preview1" "fd_write" (func $fd_write ...))
```

Imports have:
- **Module name**: where the import comes from (`"env"`, `"wasi_snapshot_preview1"`)
- **Field name**: what to import (`"memory"`, `"print"`)
- **Kind**: function, table, memory, or global
- **Type**: the signature (for functions)

This is more structured than ELF. In ELF, a symbol is just a name. In WASM, imports are organized by module and have explicit types.

### Function Section (ID: 3)

Maps functions to their types. Just indices:

```
Function 0 → Type 0
Function 1 → Type 1
Function 2 → Type 0
```

The actual code comes later (in the Code section). This separation enables streaming compilation—you can start compiling functions before the whole module downloads.

### Table Section (ID: 4)

Tables are for indirect calls (function pointers). Think of a table as an array of function references:

```wat
(table $t 10 funcref)
```

This declares a table of 10 function references. Indirect calls use table indices instead of direct addresses.

### Memory Section (ID: 5)

Linear memory declaration:

```wat
(memory $m 1 10)  ; 1 page minimum, 10 pages maximum
```

One page = 64KB. Memory is bounds-checked—no buffer overflows into arbitrary memory.

### Global Section (ID: 6)

Global variables:

```wat
(global $stack_pointer (mut i32) (i32.const 1048576))
(global $heap_base i32 (i32.const 2097152))
```

Globals can be mutable or immutable. They have explicit types.

### Export Section (ID: 7)

What the module exposes—the WASM equivalent of defined global symbols:

```wat
(export "add" (func $add))
(export "memory" (memory $m))
(export "_start" (func $_start))
```

Exports have names and point to internal indices. A function, table, memory, or global can be exported.

### Start Section (ID: 8)

Optional. Specifies a function to call on instantiation (like a constructor).

### Element Section (ID: 9)

Initializes tables with function references:

```wat
(elem (i32.const 0) $func_a $func_b $func_c)
```

This populates table indices 0, 1, 2 with the specified functions.

### Code Section (ID: 10)

The actual function bodies—the machine code equivalent:

```wat
(func $add (param $a i32) (param $b i32) (result i32)
  local.get $a
  local.get $b
  i32.add
)
```

This is where the bytes live. Each function entry has:
- Local variable declarations
- The instruction sequence

### Data Section (ID: 11)

Initializes linear memory:

```wat
(data (i32.const 1024) "Hello, World!\00")
```

String literals, initialized arrays, and constant data go here.

### Custom Sections (ID: 0)

Extension mechanism. The **name section** (`.debug_info` equivalent) is a custom section:

```
Custom section "name"
  - Function names
  - Local variable names
  - Module name
```

Other custom sections include:
- **`linking`** — relocation info for object files
- **`reloc.*`** — relocation entries
- **`producers`** — toolchain metadata

## WASM Object Files vs Final Modules

Here's a key distinction: a **WASM object file** (`.o`) is different from a **final WASM module** (`.wasm`).

The difference is like ELF `.o` vs ELF executable:

| Property | WASM Object File | Final WASM Module |
|----------|------------------|-------------------|
| Relocations | Yes (in custom sections) | No |
| Undefined symbols | Yes | No (all imports resolved) |
| Multiple data segments | Yes | Merged |
| Linking metadata | Yes (`linking` section) | Stripped |
| Can run directly | No | Yes |

When you compile C to WASM:

```bash
clang --target=wasm32 -c file.c -o file.o
```

You get a WASM object file. The `wasm-ld` linker combines object files into a final module:

```bash
wasm-ld file.o other.o -o output.wasm
```

## The Linking Section

WASM object files contain a custom `linking` section with:

### Symbol Table

```
Symbol 0: "add" (function, defined, index 3)
Symbol 1: "printf" (function, undefined)
Symbol 2: "global_var" (data, defined, offset 1024)
Symbol 3: "external_data" (data, undefined)
```

Like ELF symbols: names, types, definitions vs undefined.

### Segment Info

Data segments with names and alignment:

```
Segment 0: ".rodata.str" (alignment 1, flags STRINGS)
Segment 1: ".data" (alignment 4, flags 0)
Segment 2: ".bss" (alignment 8, flags BSS)
```

### Init Functions

Functions to call at initialization:

```
Init function 0: priority 100, symbol "init_globals"
Init function 1: priority 200, symbol "init_runtime"
```

## WASM Relocations

Object files have relocation sections (`reloc.CODE`, `reloc.DATA`):

```
Relocation 0: type R_WASM_FUNCTION_INDEX_LEB, offset 0x15, symbol "printf"
Relocation 1: type R_WASM_MEMORY_ADDR_LEB, offset 0x23, symbol "message"
```

Types of WASM relocations:

| Type | Meaning |
|------|---------|
| `R_WASM_FUNCTION_INDEX_LEB` | Reference to function index |
| `R_WASM_TABLE_INDEX_SLEB` | Reference to table index |
| `R_WASM_MEMORY_ADDR_LEB` | Memory address (in data section) |
| `R_WASM_GLOBAL_INDEX_LEB` | Reference to global index |
| `R_WASM_TYPE_INDEX_LEB` | Reference to type index |

Note the `_LEB` suffix. WASM uses LEB128 encoding for integers, so relocations must patch LEB128-encoded values.

## Inspecting WASM Files

The WebAssembly Binary Toolkit (WABT) is essential:

```bash
# Convert binary to text format
wasm2wat module.wasm -o module.wat

# Inspect structure
wasm-objdump -x module.wasm    # All sections
wasm-objdump -h module.wasm    # Section headers
wasm-objdump -j Import module.wasm  # Just imports
wasm-objdump -j Export module.wasm  # Just exports

# For object files specifically
wasm-objdump -r module.o       # Relocations
wasm-objdump -t module.o       # Symbol table
```

Let's see a real example:

```bash
$ wasm-objdump -x simple.wasm

simple.wasm:    file format wasm 0x1

Section Details:

Type[2]:
 - type[0] () -> i32
 - type[1] (i32, i32) -> i32

Import[1]:
 - func[0] sig=0 <env.print> <- env.print

Function[2]:
 - func[1] sig=1 <add>
 - func[2] sig=0 <main>

Export[2]:
 - func[1] <add> -> "add"
 - func[2] <main> -> "main"

Code[2]:
 - func[1] size=7 <add>
 - func[2] size=15 <main>
```

## ELF vs WASM: First Impressions

| Aspect | ELF | WASM |
|--------|-----|------|
| Structure | Sections + Segments | Sections only |
| Symbol names | Arbitrary strings | Module.field pairs |
| Type information | Optional (debug info) | Required (type section) |
| Memory model | Flat address space | Sandboxed linear memory |
| Relocations | Machine-specific | Platform-independent |
| Entry point | Address in header | Optional start function |

WASM is **safer by design**:
- Type-checked at load time
- Memory bounds-checked
- No arbitrary pointers
- Capabilities-based (imports are explicit)

But also **more constrained**:
- No raw pointers
- No inline assembly
- Limited host interaction

## A Practical Example

Let's trace compilation from C to WASM object file to final module:

```c
// math.c
int add(int a, int b) {
    return a + b;
}
```

Compile to object file:

```bash
$ clang --target=wasm32 -c math.c -o math.o
```

Inspect the object file:

```bash
$ wasm-objdump -t math.o

math.o:    file format wasm 0x1

SYMBOL TABLE:
 - F d <add> func=0
```

One symbol: `add`, a defined function (`d`).

Now with an undefined reference:

```c
// main.c
extern int add(int, int);
int main() {
    return add(2, 3);
}
```

```bash
$ clang --target=wasm32 -c main.c -o main.o
$ wasm-objdump -t main.o

SYMBOL TABLE:
 - F d <__main_void> func=0
 - F U <add>
```

Two symbols: `__main_void` (defined) and `add` (undefined, `U`).

Link them:

```bash
$ wasm-ld math.o main.o --no-entry --export-all -o combined.wasm
$ wasm-objdump -x combined.wasm

Export[2]:
 - func[0] <__main_void> -> "__main_void"
 - func[1] <add> -> "add"
```

No more undefined symbols. The linker resolved `add` from `math.o`.

## Key Takeaways

1. **WASM modules have sections**, like ELF, but simpler—no segments for loading
2. **Imports and exports are explicit** with module.field naming and types
3. **Type information is mandatory**, enabling ahead-of-time validation
4. **Object files use custom sections** for linking metadata and relocations
5. **WASM's sandboxed model** means no raw pointers or arbitrary memory access

## The Foundation Is Set

We've now seen two binary formats: ELF, the veteran of native computing, and WASM, the sandboxed newcomer. Both have sections. Both have something like symbols. Both need to connect code that references things to code that defines things.

But we've been hand-waving about the details. When we say "symbol table," what's actually in it? When we say a symbol is "undefined," where is that recorded and how? When the linker "resolves" a symbol, what data structures is it manipulating?

Part II of this book digs into those data structures. We'll start with symbol tables—the registries that make linking possible. You'll see the exact bytes that encode a symbol's name, type, binding, and location. You'll understand why C++ symbols look like `_ZN4math3addEii` and how to decode them. You'll learn why ELF needs *two* symbol tables and what happens to each when you strip a binary.

This is where we transition from "what does it look like" to "how does it actually work."
