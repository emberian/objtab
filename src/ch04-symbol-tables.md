# Symbol Tables Demystified

Every programmer has seen a symbol table error. "Undefined reference to `foo`." "Multiple definition of `bar`." "Symbol not found: `baz`." These messages come from the linker, and they're telling you that something went wrong in the symbol table—the data structure at the heart of linking.

But what *is* a symbol table? Not conceptually—we covered that in Chapter 1. What is it *physically*? What bytes are in the file? What does the linker actually read when it resolves your function calls?

This chapter answers those questions. We'll look at the raw structure of symbol table entries, see how names are stored, and understand the flags that control visibility and binding. By the end, you'll be able to read `readelf -s` output like a native speaker, and debug symbol problems by understanding exactly what went wrong.

We've mentioned symbol tables throughout this book. Now let's crack them open and see exactly how they work.

## The Symbol Table: A Database of Names

A symbol table is a structured list of symbols. Each entry contains:

1. **Name** (or pointer to name string)
2. **Value** (address/offset)
3. **Size** (bytes)
4. **Type** (function, object, etc.)
5. **Binding** (local, global, weak)
6. **Section** (where it lives)
7. **Visibility** (default, hidden, protected)

Let's look at the raw structure.

## ELF Symbol Table Structure

In ELF, a symbol entry (64-bit) looks like:

```c
typedef struct {
    Elf64_Word    st_name;   // Offset into string table
    unsigned char st_info;   // Type and binding
    unsigned char st_other;  // Visibility
    Elf64_Half    st_shndx;  // Section index
    Elf64_Addr    st_value;  // Symbol value (address)
    Elf64_Xword   st_size;   // Size of symbol
} Elf64_Sym;
```

That's 24 bytes per symbol. Let's decode each field.

### `st_name`: The Name

This isn't the actual string—it's an **offset into the string table** (`.strtab`). Why?

Strings have variable length. Fixed-size symbol entries enable random access. You can jump directly to symbol N without scanning through N-1 variable-length strings.

```
String table (.strtab):
Offset 0:   '\0'              (null string)
Offset 1:   'add\0'           (offset 1)
Offset 5:   'multiply\0'      (offset 5)
Offset 14:  'main\0'          (offset 14)

Symbol 3: st_name = 5 → "multiply"
```

### `st_info`: Type and Binding Combined

One byte, split in two:
- Low 4 bits: **type**
- High 4 bits: **binding**

```c
#define ELF64_ST_TYPE(i)    ((i)&0xf)
#define ELF64_ST_BIND(i)    ((i)>>4)
```

**Type values:**

| Value | Name | Meaning |
|-------|------|---------|
| 0 | `STT_NOTYPE` | Unspecified |
| 1 | `STT_OBJECT` | Data object (variable) |
| 2 | `STT_FUNC` | Function/executable code |
| 3 | `STT_SECTION` | Section symbol |
| 4 | `STT_FILE` | Source file name |
| 10 | `STT_GNU_IFUNC` | Indirect function (resolver) |

**Binding values:**

| Value | Name | Meaning |
|-------|------|---------|
| 0 | `STB_LOCAL` | Visible only in this file |
| 1 | `STB_GLOBAL` | Visible everywhere |
| 2 | `STB_WEAK` | Lower priority global |

### `st_other`: Visibility

Controls symbol visibility beyond binding:

| Value | Name | Meaning |
|-------|------|---------|
| 0 | `STV_DEFAULT` | Normal rules apply |
| 1 | `STV_INTERNAL` | Hidden + cannot be interposed |
| 2 | `STV_HIDDEN` | Not visible outside shared object |
| 3 | `STV_PROTECTED` | Visible but not preemptible |

The difference between hidden and protected:
- **Hidden**: Other shared libraries can't see this symbol at all
- **Protected**: Other libraries see it, but calls within this library always use this definition (no interposition)

### `st_shndx`: Section Index

Which section contains this symbol?

| Value | Meaning |
|-------|---------|
| 0 (`SHN_UNDEF`) | Undefined (external reference) |
| 1-0xfeff | Section index |
| 0xfff1 (`SHN_ABS`) | Absolute value, not relocatable |
| 0xfff2 (`SHN_COMMON`) | COMMON symbol (allocated by linker) |

For defined symbols, this points to `.text`, `.data`, etc. For undefined symbols, it's `SHN_UNDEF`.

### `st_value`: The Address

For **defined symbols**: the symbol's address (or offset in object files).

For **undefined symbols**: typically 0, but for COMMON symbols, it holds alignment requirements.

For **section symbols**: 0 (the section's start address).

### `st_size`: Symbol Size

How many bytes this symbol occupies. For functions, it's the code size. For variables, it's the variable size.

A size of 0 means unknown or not applicable.

## Reading a Symbol Table

Let's decode a real example:

```bash
$ readelf -s math.o

Symbol table '.symtab' contains 12 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND 
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS math.c
     2: 0000000000000000     0 SECTION LOCAL  DEFAULT    1 .text
     3: 0000000000000000     0 SECTION LOCAL  DEFAULT    2 .data
     4: 0000000000000000     0 SECTION LOCAL  DEFAULT    3 .bss
     5: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT    1 $x
     6: 0000000000000000     0 SECTION LOCAL  DEFAULT    5 .note.GNU-stack
     7: 0000000000000014     0 NOTYPE  LOCAL  DEFAULT    6 $d
     8: 0000000000000000     0 SECTION LOCAL  DEFAULT    6 .eh_frame
     9: 0000000000000000     0 SECTION LOCAL  DEFAULT    4 .comment
    10: 0000000000000000    32 FUNC    GLOBAL DEFAULT    1 add
    11: 0000000000000020    32 FUNC    GLOBAL DEFAULT    1 multiply
```

Decoding each:

**Symbol 0**: Null symbol (required by ELF spec). Always present, always zero.

**Symbol 1**: Source file name. `FILE` type, `ABS` section (not in any section), name is the filename.

**Symbol 2**: Section symbol for `.text`. Used for relocations that reference the section itself.

**Symbol 3**: The `add` function. Offset 0 in `.text`, 4 bytes, global function.

**Symbol 4**: The `multiply` function. Offset 4 in `.text`, 6 bytes, global function.

## Two Symbol Tables: `.symtab` vs `.dynsym`

ELF executables can have **two** symbol tables:

| Table | Purpose | Stripped? |
|-------|---------|-----------|
| `.symtab` | All symbols (debugging, linking) | Yes, by `strip` |
| `.dynsym` | Dynamic symbols only | No (needed at runtime) |

`.symtab` is comprehensive—every function, every variable, internal helpers. It's used by debuggers and disassemblers.

`.dynsym` is minimal—only symbols needed for dynamic linking. It survives `strip` because the dynamic linker needs it at runtime.

```bash
$ nm /usr/bin/python3 | wc -l
0
$ nm -D /usr/bin/python3 | wc -l
2255
```

## Symbol Hashing for Fast Lookup

Looking up a symbol by name in a linear list is O(n). For large symbol tables, ELF uses hash tables.

### SysV Hash (`.hash`)

The original ELF hash format:

```c
unsigned long elf_hash(const unsigned char *name) {
    unsigned long h = 0, g;
    while (*name) {
        h = (h << 4) + *name++;
        if ((g = h & 0xf0000000))
            h ^= g >> 24;
        h &= ~g;
    }
    return h;
}
```

The `.hash` section contains:
- `nbucket`: Number of buckets
- `nchain`: Number of symbols
- `bucket[nbucket]`: Hash buckets
- `chain[nchain]`: Chains for collision resolution

### GNU Hash (`.gnu.hash`)

Modern replacement, faster for typical lookups:

- Bloom filter for fast "definitely not here" checks
- Sorted symbols for binary search within buckets
- Symbols grouped: locals first (not hashed), then globals

Most modern Linux binaries use `.gnu.hash`.

## WASM Symbol Tables

WASM's symbol table lives in a custom `linking` section. The structure is different:

```
WASM Symbol Entry:
- kind: SYMTAB_FUNCTION, SYMTAB_DATA, SYMTAB_GLOBAL, SYMTAB_SECTION, SYMTAB_EVENT, SYMTAB_TABLE
- flags: WASM_SYM_BINDING_WEAK, WASM_SYM_BINDING_LOCAL, WASM_SYM_VISIBILITY_HIDDEN, WASM_SYM_UNDEFINED, etc.
- For functions: index into function space
- For data: segment index + offset
- name: direct string (not offset)
```

Key differences from ELF:

1. **Names are inline**, not in a separate string table
2. **Typed by section kind**: function symbols vs data symbols are distinct
3. **Index-based references**: symbols reference indices, not addresses
4. **Simpler binding model**: just local, global, and weak flags

```bash
$ wasm-objdump -t math.o

SYMBOL TABLE:
 - F d <add> func=0
 - F d <multiply> func=1
```

`F` means function, `d` means defined.

## Symbol Resolution Rules

When the linker encounters symbols, it follows resolution rules:

### Rule 1: Exactly One Strong Definition

A symbol with global binding should be defined exactly once (across all input files). Multiple definitions = error.

```bash
$ gcc -c file1.c -o file1.o  # defines 'foo'
cc1: fatal error: file1.c: No such file or directory
compilation terminated.
$ gcc -c file2.c -o file2.o  # also defines 'foo'
cc1: fatal error: file2.c: No such file or directory
compilation terminated.
$ gcc file1.o file2.o -o out
/usr/bin/ld: cannot find file1.o: No such file or directory
/usr/bin/ld: cannot find file2.o: No such file or directory
collect2: error: ld returned 1 exit status
```

### Rule 2: Weak vs Strong

If both weak and strong definitions exist, strong wins:

```c
// libc provides (weak):
__attribute__((weak)) void malloc_hook(void) { }

// Your program provides (strong):
void malloc_hook(void) { custom_implementation(); }
```

The linker picks your strong definition.

### Rule 3: Weak + Weak

If only weak definitions exist, the linker picks one (unspecified which). This is how default implementations work.

### Rule 4: Undefined + Defined

Undefined symbols must be resolved to a definition. No definition = linker error:

```bash
undefined reference to 'missing_function'
```

### Rule 5: COMMON Symbols

Uninitialized globals in C (without `extern`) can be COMMON:

```c
int foo;  // Might be COMMON (compiler-dependent)
```

COMMON symbols:
- Can appear in multiple files
- Linker allocates storage once, using the largest size
- Considered weak for resolution purposes

Modern best practice: avoid COMMON (use `-fno-common`).

## Visibility in Practice

Visibility controls what escapes a shared library:

```c
// Default: exported from shared library
__attribute__((visibility("default")))
void public_api(void);

// Hidden: internal to shared library
__attribute__((visibility("hidden")))
void internal_helper(void);
```

Or use compiler flags:

```bash
# Everything hidden by default
gcc -fvisibility=hidden -c file.c

# Then explicitly export
__attribute__((visibility("default")))
void exported_function(void);
```

This reduces symbol table size and enables better optimization (the compiler knows internal functions can't be interposed).

## Name Mangling

C++ (and Rust) mangle names to encode type information:

```cpp
int add(int, int);           // _Z3addii
double add(double, double);  // _Z3adddd
```

The symbol table contains mangled names. Tools can demangle:

```bash
$ nm math.o | c++filt
0000000000000000 T add(int, int)
0000000000000014 T add(double, double)
```

Mangling schemes vary by compiler. The Itanium C++ ABI (used by GCC and Clang) is most common.

## JavaScript Analogy

Think of a symbol table like a module's exports and imports registry:

```javascript
// Symbol table for this module:
// DEFINED (exported):
//   - calculateTax (function, global)
//   - TAX_RATE (object, global)
// UNDEFINED (imported):
//   - formatCurrency (from 'utils')
//   - Logger (from 'logging')

export const TAX_RATE = 0.08;
export function calculateTax(amount) {
    return formatCurrency(amount * TAX_RATE);
}
```

The bundler (linker) resolves `formatCurrency` to its definition in `utils.js`.

## Debugging Symbol Issues

Common symbol-related errors:

### Undefined Reference

```bash
undefined reference to 'foo'
```

Meaning: `foo` is used but never defined. Check:
- Is the object file containing `foo` being linked?
- Is `foo` actually defined (not just declared)?
- C++ name mangling issues? Try `extern "C"`.

### Multiple Definition

```bash
multiple definition of 'bar'
```

Meaning: `bar` is defined more than once. Check:
- Header-only functions should be `static inline`
- Did you define a variable in a header (should be `extern` + one definition)
- Link order issues?

### Symbol Not Found (Dynamic)

```bash
symbol lookup error: undefined symbol: baz
```

Meaning: runtime linking failed. Check:
- Is the library providing `baz` loaded? (`ldd` to check)
- Symbol visibility—is it exported?
- Library version—was it compiled with a different ABI?

## Tools for Symbol Investigation

```bash
# List symbols
nm file.o
nm -D file.so           # Dynamic symbols only
nm -C file.o            # Demangle C++ names

# Detailed symbol info
readelf -s file.o       # ELF symbol table
readelf --dyn-syms file.so  # Dynamic symbols

# Find symbol in libraries
nm -A /lib/*.so | grep 'T symbol_name'

# Check if symbol is exported
objdump -T file.so | grep symbol_name
```

## Key Takeaways

1. **Symbol tables map names to addresses** (or to "undefined")
2. **ELF has two tables**: `.symtab` (full, strippable) and `.dynsym` (minimal, required)
3. **Binding** (local/global/weak) controls resolution priority
4. **Visibility** controls export from shared libraries
5. **Hash tables** enable fast symbol lookup
6. **WASM symbol tables** are simpler, type-aware, and live in custom sections

## The Missing Piece

Symbol tables tell us *what* exists and *where* it is. But there's a problem we haven't addressed: how does code actually *use* that information?

When the compiler generates machine code for `call add`, it doesn't know where `add` will be. That address is determined later, by the linker. So the compiler emits a placeholder—often just zeros—and leaves a note saying "hey, linker, please fill this in with the address of `add`."

That note is called a *relocation*. Relocations are the instructions that tell the linker how to patch object files into working executables. They're the glue that binds symbol tables to actual code.

In the next chapter, we'll explore relocations in detail. You'll see the different types (PC-relative, absolute, GOT-relative), understand the calculations involved, and learn why position-independent code needs different relocations than fixed-address code. If symbol tables are the directory, relocations are the wiring diagram.
