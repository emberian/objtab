# ELF vs WASM: A Detailed Comparison

Two binary formats. Twenty years apart. Both encode compiled code, symbols, and linking metadata. But they feel completely different to work with.

ELF grew from Unix culture: minimal constraints, maximum flexibility, trust the programmer. It's a format that lets you shoot yourself in the foot—and assumes you know how to avoid it. You can jump to arbitrary addresses. You can read and write any memory. You can override any symbol. The power is yours; the responsibility is yours.

WASM grew from web culture: assume hostile code, verify everything, sandbox by default. It's a format that won't let you misbehave—not because it catches you, but because misbehavior is inexpressible. You can't jump to arbitrary addresses because addresses don't exist. You can't escape your memory sandbox because the format doesn't have escape hatches. The constraints are the feature.

Neither is "better." They're optimized for different threat models. ELF assumes the OS provides security; WASM assumes nothing does. ELF optimizes for native performance; WASM optimizes for portable safety.

Let's see exactly how these philosophies manifest in their designs.

ELF was designed around 1989 for Unix System V Release 4. WebAssembly was designed in 2015 for web browsers. Different eras, different constraints, different designs.

Yet both solve the same fundamental problem: representing compiled code and the metadata needed to link and run it.

Let's compare them systematically.

## Design Philosophy

### ELF: Bare Metal Legacy

ELF trusts the code. It's designed for:
- Direct memory access
- System calls
- Shared libraries with symbol interposition
- Debugging and profiling tools
- Maximum flexibility and performance

ELF assumes the OS provides security. Code runs with whatever privileges the process has.

### WASM: Sandboxed by Design

WASM trusts nothing. It's designed for:
- Untrusted code execution (browser tabs, serverless)
- Capability-based security
- Portability across architectures
- Fast validation and compilation
- Deterministic execution

WASM enforces security at the format level. Code can only access what's explicitly provided through imports.

## Memory Model

### ELF: Flat Address Space

ELF programs see the entire address space:

```
0x0000000000000000  ─┐
                     │  Unmapped (null page)
0x0000000000400000  ─┤
                     │  Program code (.text)
0x0000000000600000  ─┤
                     │  Program data (.data, .bss)
0x00007f0000000000  ─┤
                     │  Shared libraries
0x00007fff00000000  ─┤
                     │  Stack
0x00007fffffffffff  ─┘
```

Pointers are raw addresses. Buffer overflows can overwrite arbitrary memory. Memory protection is enforced by the OS, not the format.

### WASM: Linear Memory Sandbox

WASM modules have isolated linear memory:

```
┌─────────────────┐
│ Module A        │
│ Linear Memory   │  Only this module can access
│ 0 to size-1     │
└─────────────────┘

┌─────────────────┐
│ Module B        │
│ Linear Memory   │  Completely separate
│ 0 to size-1     │
└─────────────────┘
```

Memory bounds are checked on every access. You can't read outside your linear memory—the VM traps. No pointer is "to the OS" or "to another module" unless explicitly imported.

## Type System

### ELF: No Type Information

ELF symbols have no type beyond "function" or "object":

```
$ nm math.o
0000000000000000 T add      # Function... what signature?
0000000000000010 T multiply  # Also function... same?
```

The linker doesn't know (or care) that `add` takes two ints and returns an int. It just matches names. Type mismatches are your problem.

Debug info (DWARF) has types, but it's optional and ignored by the linker.

### WASM: Mandatory Type Checking

Every WASM function has a type, and the type system is enforced:

```wat
(type $binary_int (func (param i32 i32) (result i32)))

(func $add (type $binary_int)
  local.get 0
  local.get 1
  i32.add
)
```

Calling a function with wrong types? Validation fails. The module won't even instantiate.

```javascript
// This would be caught at validation, not runtime
const instance = await WebAssembly.instantiate(badModule, imports);
// ValidationError: type mismatch
```

## Symbol Resolution

### ELF: Name-Based, Late Binding

ELF symbols are strings. Resolution happens at link time or load time:

```
main.o:  calls "printf"
libc.so: defines "printf"
↓
Linker matches strings
```

Features enabled by this model:
- Symbol interposition (`LD_PRELOAD`)
- Weak symbols and default implementations
- Symbol versioning
- Lazy binding (PLT)

Drawbacks:
- Typos in symbol names → link errors (not compile errors)
- ABI mismatches → silent corruption or crash
- Symbol lookup has overhead

### WASM: Structured Imports, Eager Resolution

WASM imports are structured: module name + field name + type:

```wat
(import "env" "print" (func $print (param i32)))
(import "wasi" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
```

Resolution happens at instantiation:

```javascript
const imports = {
  env: {
    print: (x) => console.log(x),  // Must match signature
  },
  wasi: {
    fd_write: wasi.fd_write,       // Must match signature
  },
};

const instance = await WebAssembly.instantiate(module, imports);
```

Features:
- Type errors caught at instantiation
- No name collisions (namespaced by module)
- Host provides capabilities explicitly

Drawbacks:
- No lazy binding (all imports resolved upfront)
- No interposition (you get what you're given)
- Less flexible

## Relocations

### ELF: Address Patching

ELF relocations patch actual addresses into machine code:

```
Before: e8 00 00 00 00        (call with placeholder)
After:  e8 15 01 00 00        (call with real offset)
```

Relocation types are architecture-specific. x86-64 has dozens. Each encodes:
- Where to patch
- What size
- What calculation (PC-relative? GOT-relative? Absolute?)

### WASM: Index Renumbering

WASM relocations patch indices:

```
Before: call 0        (function index 0)
After:  call 5        (function index 5, after combining modules)
```

Relocation types are architecture-independent:
- `R_WASM_FUNCTION_INDEX_LEB`: Function call
- `R_WASM_MEMORY_ADDR_LEB`: Memory address
- `R_WASM_TYPE_INDEX_LEB`: Type reference

Much simpler because WASM uses structured indices, not raw addresses.

## Control Flow

### ELF: Unstructured (Arbitrary Jumps)

Machine code can jump anywhere:

```asm
    jmp    label        ; Forward jump
    jmp    *%rax        ; Indirect jump (anywhere!)
    call   *(%rsp)      ; Return to anywhere
```

This enables:
- Computed gotos
- Tail calls
- JIT compilation
- Return-oriented programming (ROP) attacks 😈

### WASM: Structured Control Flow

WASM only has structured control flow:

```wat
(block $outer
  (block $inner
    br_if $inner       ; Conditional break to $inner
    br $outer          ; Unconditional break to $outer
  )
)

(loop $repeat
  ;; ...
  br_if $repeat        ; Conditional continue
)
```

No arbitrary jumps. No goto. All control flow is explicitly nested.

This:
- Enables streaming compilation
- Guarantees CFI (Control Flow Integrity)
- Simplifies validation
- Prevents certain exploit classes

But limits:
- Must transform arbitrary control flow to structured form
- Some patterns require "relooper" algorithms in compilers

## Dynamic Linking

### ELF: Full Dynamic Linking

ELF has comprehensive dynamic linking:

```
Executable → Dynamic Linker → Shared Libraries
    ↓              ↓               ↓
NEEDED tags    Loads libs      GOT/PLT
               Resolves        Symbol
               symbols         tables
```

Features:
- Lazy binding
- Symbol interposition
- Library updates without recompiling
- Memory sharing between processes

### WASM: Import-Based Linking

WASM's "dynamic linking" is import resolution:

```javascript
// Host provides imports
const instance = await WebAssembly.instantiate(module, imports);

// Imports are fixed at instantiation
// No lazy binding, no interposition
```

Emscripten simulates dynamic linking with:
- A shared memory between modules
- Function pointer tables
- A JavaScript "dynamic linker" shim

But it's not the same. There's no OS-level library sharing.

## Comparison Table

| Aspect | ELF | WASM |
|--------|-----|------|
| **Year introduced** | ~1989 (SVR4) | 2015 |
| **Primary platform** | Unix (Linux, BSD, Solaris) | Web browsers, edge, embedded |
| **Security model** | OS-enforced | Format-enforced (sandbox) |
| **Memory model** | Flat address space | Linear memory sandbox |
| **Type system** | None (optional debug info) | Mandatory, validated |
| **Symbol names** | Arbitrary strings | Module.field pairs |
| **Relocation** | Address patching | Index renumbering |
| **Control flow** | Arbitrary jumps | Structured only |
| **Dynamic linking** | Full (GOT, PLT, ld.so) | Import-based only |
| **Shared libraries** | Yes (memory sharing) | No (instance isolation) |
| **Lazy binding** | Yes | No |
| **Symbol interposition** | Yes | No |
| **Architecture** | Specific (per-arch) | Portable (one format) |
| **Compilation speed** | N/A (native code) | Designed for fast JIT |
| **Binary size** | Larger (native code) | Smaller (compact encoding) |

## When to Use What

### Use ELF When:

- Building native Linux/Unix applications
- Need maximum performance
- Need shared library memory sharing
- Need symbol interposition for debugging/profiling
- Building kernel modules or system components

### Use WASM When:

- Running untrusted code safely
- Need portability across architectures
- Running in browsers or edge environments
- Want deterministic, reproducible execution
- Building plugins for existing applications

## The Convergence

Despite different designs, they're converging:

**WASM is gaining ELF-like features:**
- Component Model: structured interfaces between modules
- WASI: standardized system interface
- Threads: shared memory between workers
- Exception handling: structured unwinding

**ELF toolchains are adopting WASM concepts:**
- CFI (Control Flow Integrity) adds WASM-like control flow guarantees
- Memory tagging adds bounds checking
- Sandboxing technologies (seccomp, Landlock) limit capabilities

## A Practical Perspective

For a web developer, here's the key insight:

**ELF** is what your server-side code becomes. When you deploy Node.js, Python, or Go, you're running ELF binaries with all their flexibility and danger.

**WASM** is what your client-side native code becomes. When you compile Rust to run in the browser, or use Figma's WASM-compiled engine, you're using WASM's safety guarantees.

And increasingly, **WASM on the server** (Cloudflare Workers, Fastly Compute, etc.) brings browser-style sandboxing to backend code.

## Key Takeaways

1. **ELF optimizes for performance and flexibility** at the cost of safety
2. **WASM optimizes for safety and portability** at the cost of features
3. **ELF's flat memory model** enables pointer tricks; WASM's sandbox prevents them
4. **ELF has no type checking**; WASM enforces types at validation
5. **ELF's dynamic linking** is richer but more complex than WASM's imports
6. **Structured control flow** makes WASM safer but sometimes awkward
7. **Both formats are evolving** toward each other's strengths

## From Theory to Practice

Nine chapters of formats, data structures, and design philosophy. You now understand object files at a level most developers never reach. But knowledge without application is just trivia.

The final chapter is about using this knowledge. What do you actually *do* when npm install fails with a native module error? How do you shrink a 2MB WASM bundle to something reasonable? When your Node.js app has mysterious latency spikes, how do you check if it's library loading?

We'll walk through real scenarios: debugging symbol errors, optimizing binary sizes, building native addons, and deploying WASM in production. The theory you've learned becomes the toolkit you'll use.

Let's make this practical.
