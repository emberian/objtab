# Object File Anatomy: ELF Deep Dive

In 1995, the Unix world was fragmented. Different systems used different binary formats: a.out on older systems, COFF on System V, Mach-O on NeXT. Porting software meant wrestling with format differences. Debugging tools had to understand multiple formats. It was a mess.

ELF—the Executable and Linkable Format—was designed to end that fragmentation. It succeeded beyond anyone's expectations. Today, ELF runs on Linux, FreeBSD, OpenBSD, NetBSD, Solaris, PlayStation, Android, and countless embedded systems. When you run a program on a Linux server, you're running an ELF file. When your phone launches an app, ELF is involved. It's one of the most successful binary formats ever designed.

Understanding ELF isn't just historical curiosity. It's practical knowledge. When `npm install` fails with a mysterious native module error, ELF knowledge helps you debug it. When you're optimizing a Docker image, knowing what's in those binaries helps you shrink them. When you're investigating a security vulnerability, ELF structure tells you what's exploitable.

Let's open one up.

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
  Type:                              DYN (Position-Independent Executable file)
  Machine:                           AArch64
  Version:                           0x1
  Entry point address:               0x65c0
  Start of program headers:          64 (bytes into file)
  Start of section headers:          197720 (bytes into file)
  Flags:                             0x0
  Size of this header:               64 (bytes)
  Size of program headers:           56 (bytes)
  Number of program headers:         12
  Size of section headers:           64 (bytes)
  Number of section headers:         29
  Section header string table index: 28
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

math.o:     file format elf64-littleaarch64


Disassembly of section .text:

0000000000000000 <add>:
   0:	d10043ff 	sub	sp, sp, #0x10
   4:	b9000fe0 	str	w0, [sp, #12]
   8:	b9000be1 	str	w1, [sp, #8]
   c:	b9400fe1 	ldr	w1, [sp, #12]
  10:	b9400be0 	ldr	w0, [sp, #8]
  14:	0b000020 	add	w0, w1, w0
  18:	910043ff 	add	sp, sp, #0x10
  1c:	d65f03c0 	ret

0000000000000020 <multiply>:
  20:	d10043ff 	sub	sp, sp, #0x10
  24:	b9000fe0 	str	w0, [sp, #12]
  28:	b9000be1 	str	w1, [sp, #8]
  2c:	b9400fe1 	ldr	w1, [sp, #12]
  30:	b9400be0 	ldr	w0, [sp, #8]
  34:	1b007c20 	mul	w0, w1, w0
  38:	910043ff 	add	sp, sp, #0x10
  3c:	d65f03c0 	ret
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

### `.rel.text` and `.rela.text` — Relocations

When code references something that isn't known yet (a function in another file, a global variable), the compiler emits a **relocation entry**. We'll cover these in depth in Chapter 5.

```bash
$ readelf -r main.o

Relocation section '.rela.text' at offset 0x218 contains 2 entries:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000010  000b0000011b R_AARCH64_CALL26  0000000000000000 add + 0
000000000020  000c0000011b R_AARCH64_CALL26  0000000000000000 multiply + 0

Relocation section '.rela.eh_frame' at offset 0x248 contains 1 entry:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
00000000001c  000200000105 R_AARCH64_PREL32  0000000000000000 .text + 0
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
There are 11 section headers, starting at offset 0x2a8:

Section Headers:
  [Nr] Name              Type             Address           Offset
       Size              EntSize          Flags  Link  Info  Align
  [ 0]                   NULL             0000000000000000  00000000
       0000000000000000  0000000000000000           0     0     0
  [ 1] .text             PROGBITS         0000000000000000  00000040
       0000000000000040  0000000000000000  AX       0     0     4
  [ 2] .data             PROGBITS         0000000000000000  00000080
       0000000000000000  0000000000000000  WA       0     0     1
  [ 3] .bss              NOBITS           0000000000000000  00000080
       0000000000000000  0000000000000000  WA       0     0     1
  [ 4] .comment          PROGBITS         0000000000000000  00000080
       0000000000000013  0000000000000001  MS       0     0     1
  [ 5] .note.GNU-stack   PROGBITS         0000000000000000  00000093
       0000000000000000  0000000000000000           0     0     1
  [ 6] .eh_frame         PROGBITS         0000000000000000  00000098
       0000000000000048  0000000000000000   A       0     0     8
  [ 7] .rela.eh_frame    RELA             0000000000000000  00000220
       0000000000000030  0000000000000018   I       8     6     8
  [ 8] .symtab           SYMTAB           0000000000000000  000000e0
       0000000000000120  0000000000000018           9    10     8
  [ 9] .strtab           STRTAB           0000000000000000  00000200
       000000000000001b  0000000000000000           0     0     1
  [10] .shstrtab         STRTAB           0000000000000000  00000250
       0000000000000054  0000000000000000           0     0     1
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), p (processor specific)
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

Elf file type is DYN (Position-Independent Executable file)
Entry point 0x65c0
There are 12 program headers, starting at offset 64

Program Headers:
  Type           Offset             VirtAddr           PhysAddr
                 FileSiz            MemSiz              Flags  Align
  PHDR           0x0000000000000040 0x0000000000000040 0x0000000000000040
                 0x00000000000002a0 0x00000000000002a0  R      0x8
  INTERP         0x0000000000000324 0x0000000000000324 0x0000000000000324
                 0x000000000000001b 0x000000000000001b  R      0x1
      [Requesting program interpreter: /lib/ld-linux-aarch64.so.1]
  LOAD           0x0000000000000000 0x0000000000000000 0x0000000000000000
                 0x0000000000024f58 0x0000000000024f58  R E    0x10000
  LOAD           0x000000000002ef20 0x000000000003ef20 0x000000000003ef20
                 0x0000000000001390 0x0000000000002698  RW     0x10000
  DYNAMIC        0x000000000002f908 0x000000000003f908 0x000000000003f908
                 0x0000000000000230 0x0000000000000230  RW     0x8
  NOTE           0x00000000000002e0 0x00000000000002e0 0x00000000000002e0
                 0x0000000000000020 0x0000000000000020  R      0x8
  NOTE           0x0000000000000300 0x0000000000000300 0x0000000000000300
                 0x0000000000000024 0x0000000000000024  R      0x4
  NOTE           0x0000000000024f38 0x0000000000024f38 0x0000000000024f38
                 0x0000000000000020 0x0000000000000020  R      0x4
  GNU_PROPERTY   0x00000000000002e0 0x00000000000002e0 0x00000000000002e0
                 0x0000000000000020 0x0000000000000020  R      0x8
  GNU_EH_FRAME   0x0000000000020c8c 0x0000000000020c8c 0x0000000000020c8c
                 0x00000000000009fc 0x00000000000009fc  R      0x4
  GNU_STACK      0x0000000000000000 0x0000000000000000 0x0000000000000000
                 0x0000000000000000 0x0000000000000000  RW     0x10
  GNU_RELRO      0x000000000002ef20 0x000000000003ef20 0x000000000003ef20
                 0x00000000000010e0 0x00000000000010e0  R      0x1

 Section to Segment mapping:
  Segment Sections...
   00     
   01     .interp 
   02     .note.gnu.property .note.gnu.build-id .interp .gnu.hash .dynsym .dynstr .gnu.version .gnu.version_r .rela.dyn .rela.plt .init .plt .text .fini .rodata .eh_frame_hdr .eh_frame .note.ABI-tag 
   03     .init_array .fini_array .data.rel.ro .dynamic .got .data .bss 
   04     .dynamic 
   05     .note.gnu.property 
   06     .note.gnu.build-id 
   07     .note.ABI-tag 
   08     .note.gnu.property 
   09     .eh_frame_hdr 
   10     
   11     .init_array .fini_array .data.rel.ro .dynamic .got
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

Elf file type is DYN (Position-Independent Executable file)
Entry point 0x65c0
There are 12 program headers, starting at offset 64

Program Headers:
  Type           Offset             VirtAddr           PhysAddr
                 FileSiz            MemSiz              Flags  Align
  PHDR           0x0000000000000040 0x0000000000000040 0x0000000000000040
                 0x00000000000002a0 0x00000000000002a0  R      0x8
  INTERP         0x0000000000000324 0x0000000000000324 0x0000000000000324
                 0x000000000000001b 0x000000000000001b  R      0x1
      [Requesting program interpreter: /lib/ld-linux-aarch64.so.1]
  LOAD           0x0000000000000000 0x0000000000000000 0x0000000000000000
                 0x0000000000024f58 0x0000000000024f58  R E    0x10000
  LOAD           0x000000000002ef20 0x000000000003ef20 0x000000000003ef20
                 0x0000000000001390 0x0000000000002698  RW     0x10000
  DYNAMIC        0x000000000002f908 0x000000000003f908 0x000000000003f908
                 0x0000000000000230 0x0000000000000230  RW     0x8
  NOTE           0x00000000000002e0 0x00000000000002e0 0x00000000000002e0
                 0x0000000000000020 0x0000000000000020  R      0x8
  NOTE           0x0000000000000300 0x0000000000000300 0x0000000000000300
                 0x0000000000000024 0x0000000000000024  R      0x4
  NOTE           0x0000000000024f38 0x0000000000024f38 0x0000000000024f38
                 0x0000000000000020 0x0000000000000020  R      0x4
  GNU_PROPERTY   0x00000000000002e0 0x00000000000002e0 0x00000000000002e0
                 0x0000000000000020 0x0000000000000020  R      0x8
  GNU_EH_FRAME   0x0000000000020c8c 0x0000000000020c8c 0x0000000000020c8c
                 0x00000000000009fc 0x00000000000009fc  R      0x4
  GNU_STACK      0x0000000000000000 0x0000000000000000 0x0000000000000000
                 0x0000000000000000 0x0000000000000000  RW     0x10
  GNU_RELRO      0x000000000002ef20 0x000000000003ef20 0x000000000003ef20
                 0x00000000000010e0 0x00000000000010e0  R      0x1

 Section to Segment mapping:
  Segment Sections...
   00     
   01     .interp 
   02     .note.gnu.property .note.gnu.build-id .interp .gnu.hash .dynsym .dynstr .gnu.version .gnu.version_r .rela.dyn .rela.plt .init .plt .text .fini .rodata .eh_frame_hdr .eh_frame .note.ABI-tag 
   03     .init_array .fini_array .data.rel.ro .dynamic .got .data .bss 
   04     .dynamic 
   05     .note.gnu.property 
   06     .note.gnu.build-id 
   07     .note.ABI-tag 
   08     .note.gnu.property 
   09     .eh_frame_hdr 
   10     
   11     .init_array .fini_array .data.rel.ro .dynamic .got
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

## A Format Born of Its Time

ELF was designed for a world of desktop workstations and servers—machines with megabytes of RAM, spinning disks, and no security sandbox. It assumes the operating system will protect processes from each other, so the format itself doesn't need to enforce safety. Code can jump anywhere. Pointers can point to anything. The format trusts you.

Twenty years later, the world looked different. Browsers ran code from untrusted websites. Mobile devices ran apps from unknown developers. Edge servers ran code from customers. The old assumptions—that code could be trusted, that the OS provided enough isolation—no longer held.

WebAssembly was designed for this new world. It's a binary format, like ELF, with sections and symbols and relocations. But it makes fundamentally different choices. Memory is sandboxed. Control flow is structured. Types are mandatory. The format doesn't trust you—and that's the point.

In the next chapter, we'll explore WASM's object format. You'll see familiar concepts—sections, imports, exports—implemented in unfamiliar ways. Understanding both formats will show you the design space of binary formats: what's essential, what's historical accident, and what's deliberate trade-off.
