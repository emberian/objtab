# Static Linking

There's something satisfying about a statically linked binary. One file. No dependencies. Copy it to any machine with the right architecture, and it runs. No "library not found" errors. No version mismatches. No debugging why the production server has a different libc than your laptop.

For decades, this was the only kind of linking. You compiled your code, linked in the libraries you needed, and shipped a self-contained executable. The simplicity was the point.

Static linking fell out of favor as systems grew. When fifty programs all use the same C library, having fifty copies in memory seemed wasteful. Dynamic linking emerged to share code. But static linking never went away—it just became a choice rather than the default. Go statically links everything. Rust can. And when you need a Docker image that "just works," static linking starts looking attractive again.

Let's understand how it works.

Static linking is the simplest form of linking. Take some object files, combine them into one executable, resolve all symbols, apply all relocations. Done.

No runtime dependencies. No version mismatches. No "works on my machine." Just one file that runs.

## The Basic Process

```bash
# Compile to object files
gcc -c main.c -o main.o
gcc -c math.c -o math.o
gcc -c utils.c -o utils.o

# Link into executable
gcc main.o math.o utils.o -o program
```

Behind that `gcc` command, the linker (`ld`) does:

1. **Read** all input object files
2. **Merge** sections (all `.text` together, all `.data` together, etc.)
3. **Assign** addresses to all sections
4. **Resolve** symbols (match undefined to defined)
5. **Apply** relocations (patch code with final addresses)
6. **Write** the output executable

Let's trace through each step.

## Step 1: Reading Input Files

The linker reads each `.o` file and catalogs:
- All sections and their contents
- All symbols (with definitions and references)
- All relocations

At this point, it has a complete picture of what's available and what's needed.

## Step 2: Section Merging

Each object file has its own `.text`, `.data`, `.rodata`, etc. The linker concatenates them:

```
main.o:                    math.o:
┌──────────────┐          ┌──────────────┐
│ .text (main) │          │ .text (add)  │
│ 50 bytes     │          │ .text (mult) │
├──────────────┤          │ 30 bytes     │
│ .rodata      │          └──────────────┘
│ "hello"      │
└──────────────┘

                Combined:
         ┌────────────────────┐
         │ .text (merged)     │
         │ main: 0-50         │
         │ add:  50-70        │
         │ mult: 70-100       │
         ├────────────────────┤
         │ .rodata (merged)   │
         │ "hello"            │
         └────────────────────┘
```

Sections of the same name merge together. Section flags must be compatible (you can't merge writable and non-writable).

## Step 3: Address Assignment

Now the linker decides where everything goes. Starting from the base address (often 0x400000 on Linux x86-64):

```
Virtual Address Layout:
0x400000: ELF headers
0x401000: .text (code)
0x402000: .rodata (constants)
0x403000: .data (initialized data)
0x404000: .bss (zero-initialized data)
```

Each symbol gets a final address:
- `main` → 0x401000
- `add` → 0x401032
- `multiply` → 0x401050
- `message` → 0x402000

## Step 4: Symbol Resolution

The linker builds a global symbol table. For each undefined symbol, it searches for a definition:

```
Undefined: add (in main.o)
Searching... found in math.o at offset 0
Result: add = 0x401032

Undefined: multiply (in main.o)
Searching... found in math.o at offset 32
Result: multiply = 0x401050
```

If a symbol is undefined everywhere: **error**.
If a symbol is defined multiple times (and not weak): **error**.

## Step 5: Relocation Processing

Now the linker walks through every relocation and patches the code:

```
Relocation in main.o:
  Type: R_X86_64_PLT32
  Offset: 0x15 in .text
  Symbol: add
  Addend: -4

Calculation:
  P = place being patched = 0x401015 (main.o .text was placed at 0x401000)
  S = symbol value = 0x401032 (add)
  A = addend = -4
  
  Result = S + A - P = 0x401032 + (-4) - 0x401015 = 0x19

Patch: write 0x19 at offset 0x15
```

The `call` instruction now has the correct relative offset.

## Step 6: Output Generation

Finally, the linker writes the executable:
- ELF headers (magic, entry point, etc.)
- Program headers (segments for loading)
- Merged sections
- Section headers (optional, for debugging)
- Symbol table (if not stripped)

## Linker Scripts

The linker script controls the layout. Default scripts work for most cases, but you can customize:

```ld
/* Custom linker script */
SECTIONS {
    . = 0x10000;           /* Start address */
    .text : { *(.text) }   /* All .text sections */
    . = 0x20000;
    .data : { *(.data) }   /* All .data sections */
    .bss : { *(.bss) }     /* All .bss sections */
}
```

Embedded systems often need custom linker scripts for memory-mapped peripherals.

## Static Libraries (.a files)

A static library is an **archive** of object files:

```bash
# Create library from objects
ar rcs libmath.a add.o multiply.o divide.o

# Link against library
gcc main.o -L. -lmath -o program
```

The linker treats `.a` files specially:
1. Scan the archive's table of contents
2. Include only objects that define needed symbols
3. Unused objects are completely excluded

This is **selective linking**—you don't pay for what you don't use.

### Archive Member Selection

```bash
$ ar -t libc.a | wc -l
0
$ nm hello | grep ' T ' | wc -l
0
```

The library has 1552 object files, but only ~42 functions end up in your hello world. The rest are discarded.

### Link Order Matters

With static libraries, **order matters**:

```bash
# This might fail:
gcc -lmath main.o -o program

# This works:
gcc main.o -lmath -o program
```

Why? The linker processes files left to right. When it sees `-lmath` first, it hasn't seen `main.o`'s undefined symbols yet, so it doesn't know it needs anything from the library.

Modern linkers have `--start-group`/`--end-group` for circular dependencies:

```bash
gcc main.o -Wl,--start-group -la -lb -Wl,--end-group -o program
```

This rescans the group until no new symbols are resolved.

## WASM Static Linking

WASM linking works similarly but with indices instead of addresses:

```bash
# Compile WASM objects
clang --target=wasm32 -c main.c -o main.o
clang --target=wasm32 -c math.c -o math.o

# Link (using wasm-ld)
wasm-ld main.o math.o -o program.wasm --no-entry --export-all
```

The WASM linker:
1. Merges function and data sections
2. Renumbers all indices (functions, globals, tables)
3. Merges type sections (deduplicating signatures)
4. Combines memory segments
5. Resolves symbol references

### Index Renumbering

The tricky part of WASM linking. Each object file has its own function index space starting at 0:

```
main.o:                  math.o:
  func 0: main             func 0: add
  func 1: helper           func 1: multiply
  
Combined (after renumbering):
  func 0: main (was 0 in main.o)
  func 1: helper (was 1 in main.o)
  func 2: add (was 0 in math.o)
  func 3: multiply (was 1 in math.o)
```

Every `call` instruction referencing a renumbered function must be patched.

## Dead Code Elimination

Good linkers remove unreachable code:

```c
// utils.c
void used_function(void) { ... }    // Called
void unused_function(void) { ... }  // Never called
```

With `-gc-sections`:

```bash
gcc -ffunction-sections -fdata-sections main.c utils.c \
    -Wl,--gc-sections -o program
```

Each function gets its own section. The linker builds a reachability graph from `main`, and anything unreachable is discarded.

This can dramatically reduce binary size. A library might define 1000 functions, but your program only uses 10—the other 990 are eliminated.

## Link-Time Optimization (LTO)

With LTO, the linker sees the whole program and can optimize across compilation units:

```bash
gcc -flto main.c math.c -o program
```

The object files contain IR (intermediate representation) instead of machine code. The linker runs optimization passes on the combined IR before generating final code.

Benefits:
- Cross-module inlining
- Better dead code elimination
- Whole-program devirtualization
- More constant propagation

Cost: slower builds, larger compiler memory usage.

## Static Linking Trade-offs

### Advantages

1. **No runtime dependencies**: The executable is self-contained
2. **Predictable behavior**: No version conflicts
3. **Faster startup**: No dynamic linking overhead
4. **Easier deployment**: Copy one file
5. **Potential optimizations**: LTO, dead code elimination

### Disadvantages

1. **Larger binaries**: Every executable includes its own copy of libraries
2. **No shared memory**: Ten programs using libc = ten copies in memory
3. **No security updates**: Must recompile to get library fixes
4. **Longer build times**: More code to link
5. **License issues**: Some licenses (LGPL) require dynamic linking

## Static Linking in Practice

### Go

Go statically links by default. A Go binary has no runtime dependencies (except system calls):

```bash
$ ldd hello-go
    not a dynamic executable
$ file hello-go
hello-go: ELF 64-bit LSB executable, statically linked
```

This makes deployment trivial but binaries larger (~2MB minimum).

### Rust

Rust can do either. Static is default for musl target:

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

### C/C++

Usually dynamically linked to libc, but can be static:

```bash
gcc -static main.c -o main
```

### WASM

Always statically linked (within a module). There's no WASM dynamic linker in the traditional sense—imports are resolved at instantiation time.

## The Web Developer Parallel

Static linking is like bundling everything into one file:

```javascript
// Before bundling (many imports)
import { debounce } from 'lodash';
import React from 'react';
import { format } from 'date-fns';

// After bundling (one file)
// Everything needed is included in bundle.js
```

Webpack/Rollup/esbuild do static linking for JavaScript:
- Resolve imports
- Combine modules
- Tree-shake unused exports
- Emit one file

The trade-offs are similar: larger bundles but simpler deployment.

## Key Takeaways

1. **Static linking combines everything** into one executable
2. **The linker resolves all symbols** and applies relocations
3. **Static libraries (.a)** are archives with selective inclusion
4. **Link order matters** for static libraries
5. **Dead code elimination** removes unreachable code
6. **LTO** enables cross-module optimization
7. **Trade-off**: larger files but simpler deployment

## The Cost of Simplicity

Static linking has elegance, but it has costs. Every program carries its own copy of every library it uses. Ten programs using OpenSSL mean ten copies of OpenSSL in memory. Security vulnerability in zlib? You need to rebuild and redeploy every affected binary.

In the 1980s and 1990s, as systems grew more complex and memory remained expensive, this became untenable. The Unix world developed an alternative: shared libraries that could be loaded once and mapped into every process that needed them. Memory saved. Updates applied once. The dream of modularity realized.

But shared libraries introduced new complexity. Code can't use fixed addresses anymore—a library might load at different locations in different processes. Symbols need to be resolved at runtime, not just link time. New data structures (GOT, PLT) emerged to make this work. New errors appeared ("symbol not found," "version GLIBC_2.17 not found").

In the next chapter, we'll explore dynamic linking—the runtime machinery that makes shared libraries possible. You'll understand why position-independent code matters, how the dynamic linker resolves symbols, and what's really happening when you see `LD_LIBRARY_PATH` in a tutorial. Static linking was the foundation; dynamic linking is how the modern world actually works.
