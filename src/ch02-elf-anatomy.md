# Object File Anatomy: ELF Deep Dive

ELF stands for **Executable and Linkable Format**. It's the standard binary format on Linux, BSD, Solaris, and many embedded systems. If you've ever run a program on Linux, you've run an ELF file.

But ELF isn't just for executables. The same format is used for:
- **Object files** (`.o`) — compiler output, input to linker
- **Shared libraries** (`.so`) — dynamically linked code
- **Executables** — the final runnable program
- **Core dumps** — memory snapshots for debugging

Understanding ELF means understanding how all these fit together.

## The ELF Header

Every ELF file starts with a header. Let's look at one:

```bash
$ readelf -h /bin/ls
ELF Header:
  Magic:   7f 45 4c 46 02 01 01 00 00 00 00 00 00 00 00 00
  Class:                             ELF64
  Data:                              2's complement, little endian
  Version:                           1 (current)
  OS/ABI:                            UNIX - System V
  ABI Version:                       0
  Type:                              DYN (Position-Independent Executable)
  Machine:                           Advanced Micro Devices X86-64
  Version:                           0x1
  Entry point address:               0x6ab0
  Start of program headers:          64 (bytes into file)
  Start of section headers:          140224 (bytes into file)
  Flags:                             0x0
  Size of this header:               64 (bytes)
  Size of program headers:           56 (bytes)
  Number of program headers:         13
  Size of section headers:           64 (bytes)
  Number of section headers:         30
  Section header string table index: 29
```

Let's break this down:

### Magic Number
```
7f 45 4c 46
```
That's `\x7fELF` in ASCII. Every ELF file starts with these four bytes. It's how programs quickly identify "this is an ELF file."

### Class (32-bit vs 64-bit)
`ELF64` means this is a 64-bit binary. `ELF32` would be 32-bit. The class determines pointer sizes and structure layouts throughout the file.

### Data Encoding
`2's complement, little endian` — how numbers are stored. x86 and ARM are little-endian. Some older architectures (SPARC, older PowerPC) are big-endian.

### Type
The type tells us what kind of ELF file this is:

| Type | Meaning |
|------|---------|
| `REL` | Relocatable file (object file, `.o`) |
| `EXEC` | Executable (traditional fixed-address binary) |
| `DYN` | Shared object (`.so`, or a PIE executable) |
| `CORE` | Core dump |

Modern executables are often `DYN` (Position-Independent Executable) for security reasons (ASLR).

### Entry Point
`0x6ab0` — the address where execution begins. For executables, this is where the OS jumps to start your program. For object files, this is 0 (no entry point yet).

## Two Views of an ELF File

Here's the crucial insight: **ELF files have two parallel structures**:

1. **Sections** — for linking (static view)
2. **Segments** — for execution (runtime view)

```
                    ELF FILE
    ┌─────────────────────────────────────┐
    │           ELF Header                │
    ├─────────────────────────────────────┤
    │        Program Headers              │ ← Describes segments
    │     (for execution/loading)         │   (runtime view)
    ├─────────────────────────────────────┤
    │                                     │
    │            Sections                 │
    │   .text, .data, .rodata, .bss...   │
    │                                     │
    ├─────────────────────────────────────┤
    │        Section Headers              │ ← Describes sections
    │     (for linking/debugging)         │   (static view)
    └─────────────────────────────────────┘
```

**Sections** are the linker's view. Each section has a name, type, and content. The linker combines sections from multiple object files.

**Segments** are the loader's view. They describe how to map the file into memory. A segment might contain multiple sections.

Object files (`.o`) have sections but no segments—they're not loadable yet.

Executables have both—sections for debugging/stripping, segments for loading.

## Essential Sections

Let's explore the sections you'll encounter most often:

### `.text` — Executable Code

Your compiled functions live here. This section is:
- Readable and executable
- Usually not writable (code shouldn't modify itself)
- Contains machine instructions

```bash
$ objdump -d math.o

math.o:     file format elf64-x86-64

Disassembly of section .text:

0000000000000000 <add>:
   0:   8d 04 37                lea    (%rdi,%rsi,1),%eax
   3:   c3                      ret

0000000000000004 <multiply>:
   4:   89 f8                   mov    %edi,%eax
   6:   0f af c6                imul   %esi,%eax
   9:   c3                      ret
```

Notice the addresses start at 0. These are relative offsets—final addresses are determined during linking.

### `.data` — Initialized Data

Global and static variables with initial values:

```c
int global_counter = 42;
static int file_counter = 100;
```

Both live in `.data`. The section contains the actual bytes of the initial value.

### `.bss` — Uninitialized Data

```c
int uninitialized_global;
static int uninitialized_static;
```

The `.bss` section is special: it doesn't occupy space in the file. The section header just records *how much* memory to allocate (filled with zeros at load time).

Why separate? A program with `int array[1000000];` would have a 4MB `.data` section. With `.bss`, the file just says "allocate 4MB of zeros" — much smaller.

### `.rodata` — Read-Only Data

String literals and constants:

```c
const char* message = "Hello, world!";
const int magic = 0xDEADBEEF;
```

The string `"Hello, world!"` lives in `.rodata`. The *pointer* `message` lives in `.data` (it's an initialized global pointing to rodata).

### `.symtab` and `.strtab` — Symbol Table

The symbol table! We covered this in Chapter 1. `.symtab` contains the structured symbol entries; `.strtab` contains the actual name strings (symbols reference them by offset).

```bash
$ readelf -s math.o

Symbol table '.symtab' contains 5 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT  UND
     1: 0000000000000000     0 FILE    LOCAL  DEFAULT  ABS math.c
     2: 0000000000000000     0 SECTION LOCAL  DEFAULT    1 .text
     3: 0000000000000000     4 FUNC    GLOBAL DEFAULT    1 add
     4: 0000000000000004     6 FUNC    GLOBAL DEFAULT    1 multiply
```

### `.rel.text` and `.rela.text` — Relocations

When code references something that isn't known yet (a function in another file, a global variable), the compiler emits a **relocation entry**. We'll cover these in depth in Chapter 5.

```bash
$ readelf -r main.o

Relocation section '.rela.text' at offset 0x1d0 contains 2 entries:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000011  000500000004 R_X86_64_PLT32    0000000000000000 add - 4
000000000020  000600000004 R_X86_64_PLT32    0000000000000000 multiply - 4
```

The `main.o` file has two relocations: calls to `add` and `multiply` that need to be resolved.

### `.debug_*` — Debug Information

If you compile with `-g`, you get DWARF debug sections:
- `.debug_info` — type information, variable locations
- `.debug_line` — source line mappings
- `.debug_abbrev` — abbreviation tables
- `.debug_str` — debug strings

These can make binaries huge. `strip` removes them for release builds.

## Section Flags

Each section has flags describing its properties:

```bash
$ readelf -S math.o
Section Headers:
  [Nr] Name    Type    Address          Off    Size   ES Flg Lk Inf Al
  [ 1] .text   PROGBITS 0000000000000000 000040 00000a 00  AX  0   0  1
  [ 2] .data   PROGBITS 0000000000000000 00004a 000000 00  WA  0   0  1
  [ 3] .bss    NOBITS   0000000000000000 00004a 000000 00  WA  0   0  1
```

Flags:
- `A` — Allocate: takes up memory at runtime
- `X` — Executable: contains runnable code
- `W` — Writable: can be modified at runtime
- `S` — Strings: contains null-terminated strings
- `M` — Merge: identical content can be merged

`.text` is `AX` (allocated + executable). `.data` is `WA` (writable + allocated).

## Program Headers (Segments)

For executables and shared libraries, program headers describe memory mapping:

```bash
$ readelf -l /bin/ls
Program Headers:
  Type           Offset   VirtAddr           PhysAddr           FileSiz  MemSiz   Flg Align
  PHDR           0x000040 0x0000000000000040 0x0000000000000040 0x0002d8 0x0002d8 R   0x8
  INTERP         0x000318 0x0000000000000318 0x0000000000000318 0x00001c 0x00001c R   0x1
  LOAD           0x000000 0x0000000000000000 0x0000000000000000 0x003510 0x003510 R   0x1000
  LOAD           0x004000 0x0000000000004000 0x0000000000004000 0x013471 0x013471 R E 0x1000
  LOAD           0x018000 0x0000000000018000 0x0000000000018000 0x004fe8 0x004fe8 R   0x1000
  LOAD           0x01d900 0x000000000001e900 0x000000000001e900 0x001288 0x002548 RW  0x1000
  DYNAMIC        0x01e3f8 0x000000000001f3f8 0x000000000001f3f8 0x000200 0x000200 RW  0x8
  NOTE           0x000338 0x0000000000000338 0x0000000000000338 0x000030 0x000030 R   0x8
  GNU_EH_FRAME   0x01b28c 0x000000000001b28c 0x000000000001b28c 0x0003ec 0x0003ec R   0x4
  GNU_STACK      0x000000 0x0000000000000000 0x0000000000000000 0x000000 0x000000 RW  0x10
  GNU_RELRO      0x01d900 0x000000000001e900 0x000000000001e900 0x001700 0x001700 R   0x1
```

Key segment types:

- `LOAD` — Mapped into memory. The `RWE` flags determine permissions (Read/Write/Execute).
- `INTERP` — Path to the dynamic linker (`/lib64/ld-linux-x86-64.so.2`)
- `DYNAMIC` — Information for dynamic linking
- `GNU_STACK` — Stack permissions (non-executable stack for security)
- `GNU_RELRO` — Read-only after relocation (security hardening)

Notice how there are multiple `LOAD` segments with different permissions. One is `R E` (read + execute) for code. Another is `RW` (read + write) for data.

## Section to Segment Mapping

The linker groups sections into segments:

```bash
$ readelf -l /bin/ls
...
 Section to Segment mapping:
  Segment Sections...
   00
   01     .interp
   02     .interp .note.gnu.property .note.ABI-tag .gnu.hash .dynsym .dynstr .gnu.version .gnu.version_r .rela.dyn .rela.plt
   03     .init .plt .text .fini
   04     .rodata .eh_frame_hdr .eh_frame
   05     .init_array .fini_array .data.rel.ro .dynamic .got .data .bss
   ...
```

Multiple sections become one segment. All the code sections (`.init`, `.plt`, `.text`, `.fini`) map to segment 03 with `R E` permissions.

## Inspecting ELF Files

Your essential toolkit:

```bash
# Overview
readelf -h file        # ELF header
readelf -S file        # Section headers
readelf -l file        # Program headers (segments)
readelf -s file        # Symbol table
readelf -r file        # Relocations

# Alternative views
objdump -d file        # Disassemble
objdump -t file        # Symbol table
objdump -h file        # Section headers

# Raw hex
hexdump -C file | head # See the raw bytes
xxd file | head        # Another hex viewer

# Symbols specifically
nm file                # Quick symbol listing
nm -C file             # Demangle C++ names
```

## A Web Developer's Perspective

Think of an ELF file like a complex bundle:

| ELF Concept | Bundle Analogy |
|-------------|----------------|
| Sections | Separate chunks (JS, CSS, images) |
| Segments | How chunks are loaded (async vs sync) |
| Symbol table | Export/import declarations |
| Relocations | Import bindings to resolve |
| `.text` | Your JavaScript code |
| `.rodata` | Your string literals |
| `.data` | Your runtime state |

The linker is like a bundler (webpack, rollup): it takes multiple inputs, resolves cross-references, and produces one output.

## Try It Yourself

```bash
# Create a simple C file
echo 'int main() { return 42; }' > simple.c

# Compile to object file (not executable)
gcc -c simple.c -o simple.o

# Examine sections
readelf -S simple.o

# Compile to executable
gcc simple.c -o simple

# Compare sections
readelf -S simple

# Look at segments (only in executable)
readelf -l simple

# Check the entry point
readelf -h simple | grep Entry
```

## Key Takeaways

1. **ELF has two views**: sections (for linking) and segments (for loading)
2. **Object files have sections only**; executables have both
3. **`.text`** is code, **`.data`** is initialized data, **`.bss`** is zero-initialized
4. **The symbol table** lives in `.symtab`; relocations in `.rel*` sections
5. **Segments define memory mapping**: what's readable, writable, executable

Next, we'll look at how WebAssembly does all this differently—and why those differences matter.
