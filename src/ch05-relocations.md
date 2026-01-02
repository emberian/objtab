# Relocations: The Glue That Binds

When the compiler generates code, it doesn't know the final addresses of functions and data. It can't—the final layout depends on:
- What other object files get linked
- Library load addresses (unknown until runtime for shared libs)
- Kernel decisions about memory layout (ASLR)

So the compiler leaves **placeholders**. Relocations are the instructions for filling those placeholders in.

## The Problem

Consider this code:

```c
extern int global_counter;
extern void helper_function(void);

void my_function(void) {
    global_counter++;
    helper_function();
}
```

The compiler generates assembly that needs to:
1. Load `global_counter` from memory
2. Call `helper_function`

But what addresses should it use? It has no idea where these will end up.

## The Placeholder Solution

The compiler emits **temporary values** (often 0) and creates **relocation entries** that say:

> "At offset X in section Y, there's a placeholder. Replace it with the address of symbol Z, using calculation method W."

When the linker runs, it:
1. Decides final addresses for everything
2. Walks through all relocations
3. Patches each placeholder with the correct value

## ELF Relocation Structure

A relocation entry (64-bit) looks like:

```c
typedef struct {
    Elf64_Addr r_offset;    // Where to patch
    Elf64_Xword r_info;     // Symbol index + relocation type
    Elf64_Sxword r_addend;  // Constant to add
} Elf64_Rela;
```

### `r_offset`
The location to patch. In object files, it's an offset within the section. In executables, it's a virtual address.

### `r_info`
Two pieces packed together:
- High 32 bits: **symbol index** (which symbol provides the address)
- Low 32 bits: **relocation type** (how to calculate the patch)

```c
#define ELF64_R_SYM(i)    ((i) >> 32)
#define ELF64_R_TYPE(i)   ((i) & 0xffffffff)
```

### `r_addend`
A constant to add to the symbol's address. Useful for accessing struct members or array elements:

```c
extern struct { int a; int b; } data;
int x = data.b;  // Need address of data + offset to b
```

## Relocation Types (x86-64)

There are dozens of relocation types. Here are the most common on x86-64:

### `R_X86_64_64` — Absolute 64-bit

```
S + A
```

Where S = symbol value, A = addend.

Used for: absolute addresses in data sections (pointers).

```c
void (*fptr)(void) = &my_function;  // Needs absolute address
```

### `R_X86_64_PC32` — PC-Relative 32-bit

```
S + A - P
```

Where P = place being patched.

Used for: relative calls and jumps.

```asm
call some_function   ; Offset to function, relative to next instruction
```

This is position-independent—the code works regardless of where it's loaded.

### `R_X86_64_PLT32` — PLT Call

```
L + A - P
```

Where L = PLT entry address.

Used for: calls to functions that might be in shared libraries.

```c
printf("hello");  // Goes through PLT for dynamic linking
```

### `R_X86_64_GOT32` — GOT Offset

```
G + A
```

Where G = offset in Global Offset Table.

Used for: loading addresses from GOT.

### `R_X86_64_GOTPCREL` — GOT Entry, PC-Relative

```
G + GOT + A - P
```

Used for: accessing globals through the GOT.

```c
extern int some_global;
int x = some_global;  // Load from GOT entry
```

## Seeing Relocations

```bash
$ readelf -r main.o

Relocation section '.rela.text' at offset 0x1d0 contains 2 entries:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000011  000500000004 R_X86_64_PLT32    0000000000000000 add - 4
000000000020  000600000004 R_X86_64_PLT32    0000000000000000 multiply - 4
```

Two relocations:
- At offset 0x11 in `.text`, patch with a PLT call to `add`
- At offset 0x20 in `.text`, patch with a PLT call to `multiply`

The `-4` addend accounts for the call instruction encoding on x86.

Let's see the code before linking:

```bash
$ objdump -d main.o

0000000000000000 <main>:
   0:   55                      push   %rbp
   1:   48 89 e5                mov    %rsp,%rbp
   ...
   c:   be 03 00 00 00          mov    $0x3,%esi
  11:   e8 00 00 00 00          call   16 <main+0x16>
                        ^^^^
                        Placeholder: 00 00 00 00
```

That `e8 00 00 00 00` is a call instruction with address 0. The relocation fills it in.

After linking:

```bash
$ objdump -d program

0000000000001149 <main>:
   ...
  115c:   e8 e3 ff ff ff          call   1144 <add>
```

The placeholder became a real offset to the `add` function.

## GOT and PLT: Dynamic Linking Machinery

When linking against shared libraries, we can't know final addresses at link time. The library might load at different addresses each run (ASLR). Two structures enable this:

### Global Offset Table (GOT)

The GOT is a table of addresses, filled at runtime. Code accesses external globals through the GOT:

```asm
# Without GOT (can't work with shared libraries):
mov    (0x601030), %eax       # Absolute address, breaks if library moves

# With GOT (position-independent):
mov    GOT(%rip), %rax        # Load GOT address (PC-relative, works anywhere)
mov    some_global@GOTPCREL(%rip), %rax   # Load from GOT entry
```

The GOT entry initially contains a placeholder. The dynamic linker patches it with the real address when the library loads.

### Procedure Linkage Table (PLT)

The PLT enables lazy function binding. Instead of resolving all functions at load time, each function is resolved on first call.

A PLT entry looks like:

```asm
printf@plt:
    jmp    *printf@GOTPLT(%rip)    # Jump through GOT
    push   $index                   # If GOT not filled, push index
    jmp    resolver                 # Jump to dynamic linker
```

First call:
1. Jump through GOT → GOT has address of "push; jmp resolver"
2. Dynamic linker resolves `printf`
3. GOT entry updated to point to real `printf`

Subsequent calls:
1. Jump through GOT → goes directly to `printf`

This is **lazy binding**: symbols resolved on demand, not at load time.

## Position-Independent Code (PIC)

Shared libraries must be **position-independent**—they work regardless of load address. This requires:

1. No absolute addresses in code
2. All data accessed through GOT
3. All calls through PLT (or direct for internal calls)

```bash
# Compile for shared library
gcc -fPIC -shared lib.c -o lib.so
```

`-fPIC` makes the compiler generate position-independent code.

## WASM Relocations

WASM relocations are simpler because WASM uses indices, not addresses:

```bash
$ wasm-objdump -r main.o

RELOCATION RECORDS FOR [CODE]:
 - offset: 0x15, type: R_WASM_FUNCTION_INDEX_LEB, index: 1 <add>
 - offset: 0x23, type: R_WASM_FUNCTION_INDEX_LEB, index: 2 <multiply>
```

Common WASM relocation types:

| Type | Meaning |
|------|---------|
| `R_WASM_FUNCTION_INDEX_LEB` | Function index (for calls) |
| `R_WASM_TABLE_INDEX_SLEB` | Table index (for indirect calls) |
| `R_WASM_TABLE_INDEX_I32` | Same, but 32-bit |
| `R_WASM_MEMORY_ADDR_LEB` | Memory address |
| `R_WASM_MEMORY_ADDR_SLEB` | Signed memory address |
| `R_WASM_MEMORY_ADDR_I32` | 32-bit memory address |
| `R_WASM_TYPE_INDEX_LEB` | Type index |
| `R_WASM_GLOBAL_INDEX_LEB` | Global index |

The `_LEB` suffix means the relocation patches LEB128-encoded integers (WASM's variable-length encoding).

### WASM's Simpler Model

WASM doesn't need:
- GOT (no address space sharing between modules)
- PLT (imports are explicit, resolved at instantiation)
- PC-relative addressing (WASM uses structured control flow)

A WASM `call` instruction directly encodes the function index. The linker just needs to renumber indices when combining modules.

## Relocation Sections

In ELF, relocations live in sections named `.rel.X` or `.rela.X`, where X is the section they apply to:

| Section | Relocations for |
|---------|-----------------|
| `.rela.text` | Code |
| `.rela.data` | Initialized data |
| `.rela.dyn` | Dynamic relocations |
| `.rela.plt` | PLT entries |

`.rel` uses implicit addends (stored in the location being patched). `.rela` uses explicit addends (in the relocation entry). Modern x86-64 uses `.rela` exclusively.

## When Relocations Happen

### Link-Time Relocations

The linker processes these when creating the executable:

```c
// Object file has relocation to 'add'
// Linker resolves: add is at 0x1144
// Linker patches call instruction with offset to 0x1144
```

Result: executable has no relocations for these symbols.

### Load-Time Relocations

The dynamic linker processes these when loading shared libraries:

```c
// Executable has relocation for 'printf' (in libc)
// Program starts, dynamic linker loads libc
// Dynamic linker patches GOT entry with printf's address
```

These are `.rela.dyn` and `.rela.plt` relocations.

### RELRO: Relocation Read-Only

Security hardening:
- **Partial RELRO**: GOT is writable (for lazy binding)
- **Full RELRO**: GOT patched at load time, then marked read-only

```bash
# Full RELRO (more secure, slower startup)
gcc -Wl,-z,relro,-z,now main.c -o main

# Check RELRO status
checksec --file=main
```

Full RELRO prevents GOT overwrite attacks.

## Relocation Overflow

Relocations have limited range. `R_X86_64_PC32` can only reach ±2GB from the current location. If a relocation target is too far:

```bash
relocation R_X86_64_PC32 against 'symbol' can not be used when making a PIE object; recompile with -fPIC
```

Solutions:
- Use `-fPIC` (64-bit addressing through GOT)
- Use `-mcmodel=large` (all addresses 64-bit)
- Place related code closer together

## A Web Developer's Analogy

Relocations are like import bindings in ES6 modules:

```javascript
// Before bundling:
import { add } from './math.js';
console.log(add(2, 3));

// After bundling (conceptually):
// The bundler resolves 'add' to its actual location
// The reference is patched to point there
```

The bundler does the same thing a linker does:
1. Collect all modules
2. Find symbol definitions
3. Patch references to point to definitions

## Debugging Relocation Issues

Common problems:

### Relocation Truncated

```bash
relocation truncated to fit: R_X86_64_PC32 against undefined symbol 'foo'
```

The offset doesn't fit in 32 bits. Use `-fPIC` or `-mcmodel=medium`.

### Relocation Against Non-Existent Symbol

```bash
undefined reference to 'bar'
```

The symbol doesn't exist. Check your link inputs.

### Text Relocations

```bash
warning: creating DT_TEXTREL in a PIE
```

A relocation wants to patch code (`.text`). This defeats code sharing and W^X security. Use `-fPIC` to avoid.

## Examining Relocations

```bash
# Show all relocations
readelf -r file.o

# Show dynamic relocations (executables)
readelf -r executable | grep -E '\.rela\.(dyn|plt)'

# See what GOT entries exist
objdump -R executable

# Watch relocations happen (Linux)
LD_DEBUG=reloc ./executable
```

## Key Takeaways

1. **Relocations are instructions** for patching placeholders with addresses
2. **Link-time relocations** are resolved by the static linker
3. **Load-time relocations** are resolved by the dynamic linker
4. **GOT and PLT** enable position-independent access to external symbols
5. **WASM relocations** are simpler—indices instead of addresses
6. **-fPIC** is essential for shared libraries

Now let's see how static linking puts all these pieces together.
