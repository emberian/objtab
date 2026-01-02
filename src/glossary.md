# Glossary

## A

**ABI (Application Binary Interface)**
: The low-level interface between two binary program modules. Defines calling conventions, data layout, and system call numbers. If two binaries have incompatible ABIs, they can't work together even if the source looks compatible.

**Address Space Layout Randomization (ASLR)**
: A security technique that randomizes where code and data are loaded in memory. Makes exploits harder because attackers can't predict addresses.

**Addend**
: A constant value added during relocation calculation. For example, accessing a struct member at offset 8 might use the struct's symbol address plus addend 8.

## B

**Binding**
: A symbol property controlling visibility and resolution priority. LOCAL symbols are file-private, GLOBAL are visible everywhere, WEAK can be overridden by GLOBAL.

**BSS (Block Started by Symbol)**
: Section for uninitialized or zero-initialized data. Doesn't occupy file space—just records how much memory to allocate.

## C

**COMMON Symbol**
: A tentative definition—declared but not necessarily defined. Multiple COMMON symbols with the same name are merged by the linker. Largely obsolete; use `-fno-common`.

**Component Model**
: A proposed WASM standard for defining structured interfaces between modules, enabling better composition than raw imports/exports.

## D

**Data Section (.data)**
: Contains initialized global and static variables. Takes space in the file proportional to the data size.

**Dynamic Linker (ld.so, ld-linux.so)**
: The program that loads shared libraries at runtime, resolves symbols, and performs relocations. Runs before your main() function.

**Dynamic Symbol Table (.dynsym)**
: The symbol table consulted at runtime for dynamic linking. Survives `strip` because it's needed for shared library resolution.

## E

**ELF (Executable and Linkable Format)**
: The standard binary format on Linux and many Unix systems. Used for executables, shared libraries, object files, and core dumps.

**Entry Point**
: The address where execution begins. In ELF, specified in the header. In WASM, an optional start function.

**Export**
: A symbol or function made available to other modules. In WASM, explicitly declared; in ELF, controlled by visibility attributes.

## G

**Global Offset Table (GOT)**
: A table of addresses filled at runtime. PIC code accesses external data through the GOT, enabling position-independence.

**GNU Hash**
: A modern hash table format for fast symbol lookup in ELF files. Faster than the original SysV hash.

## I

**Import**
: A symbol or function required from another module. In WASM, explicitly typed and namespaced. In ELF, just an undefined symbol.

**Interposition**
: The ability to override a symbol from one library with a definition from another. Enabled by LD_PRELOAD. Useful for debugging, dangerous for security.

## L

**Lazy Binding**
: Resolving function addresses on first call rather than at load time. Implemented via PLT. Faster startup but first-call overhead.

**LEB128 (Little Endian Base 128)**
: Variable-length integer encoding used in WASM. Small numbers use fewer bytes.

**Linker (ld)**
: The tool that combines object files into executables or libraries. Resolves symbols and applies relocations.

**Linker Script**
: Configuration file controlling how the linker arranges sections in memory. Essential for embedded systems.

**Linear Memory**
: WASM's memory model—a contiguous, bounds-checked array of bytes. Isolated per module.

**LTO (Link-Time Optimization)**
: Optimization performed by the linker across all compilation units. Enables whole-program analysis.

## M

**Mangling**
: Encoding function signatures into symbol names. C++ uses mangling to support overloading. `_Z3addii` is mangled; `add(int, int)` is demangled.

## N

**Name Section**
: A WASM custom section containing human-readable names for functions and variables. Like debug info, optional but helpful.

## O

**Object File (.o)**
: Compiler output containing code, data, symbols, and relocations. Not directly executable—must be linked.

## P

**PIC (Position-Independent Code)**
: Code that works at any memory address. Required for shared libraries. Uses GOT for data access and PC-relative addressing for code.

**PLT (Procedure Linkage Table)**
: A jump table enabling lazy binding and position-independent function calls. Each PLT entry redirects through the GOT.

**Program Header**
: ELF metadata describing memory segments—how to load the file for execution.

## R

**RELRO (Relocation Read-Only)**
: Security hardening that makes GOT read-only after relocations are applied. Prevents GOT overwrite attacks.

**Relocation**
: An instruction to patch code or data with a final address. Contains: where to patch, which symbol, and how to calculate.

**Rodata Section (.rodata)**
: Read-only data—string literals, constants. Mapped as non-writable for security.

## S

**Section**
: A named chunk of an object file (.text, .data, .rodata, etc.). The linker's view of the file.

**Section Header**
: ELF metadata describing sections—name, type, flags, size.

**Segment**
: A loadable chunk of an executable. The loader's view of the file. Multiple sections can be in one segment.

**Shared Library (.so)**
: Code loaded at runtime and shared between processes. Requires PIC.

**SONAME**
: A shared library's canonical name, embedded in the file. Used for versioning: libfoo.so.1 is the soname even if the file is libfoo.so.1.2.3.

**Static Library (.a)**
: An archive of object files. The linker extracts only needed objects.

**String Table (.strtab)**
: A section containing null-terminated strings. Symbol names are offsets into this table.

**Symbol**
: A named reference to a function, variable, or other code element. Has properties: name, value (address), size, binding, type, visibility.

**Symbol Table (.symtab)**
: A list of all symbols in a file. Used for linking and debugging.

## T

**Text Section (.text)**
: Executable code. Marked as readable and executable, but not writable.

**Tree Shaking**
: Dead code elimination for JavaScript modules. Same concept as linker `--gc-sections`.

**Type Section**
: WASM section defining function signatures. All functions reference a type by index.

## V

**Visibility**
: Controls symbol export from shared libraries. DEFAULT exports, HIDDEN doesn't, PROTECTED exports but prevents interposition.

## W

**WASI (WebAssembly System Interface)**
: A standard API for WASM to interact with the outside world—files, network, etc. Like POSIX for WASM.

**WASM (WebAssembly)**
: A portable binary format for safe, sandboxed code execution. Runs in browsers and increasingly on servers.

**Weak Symbol**
: A symbol that can be overridden by a strong (GLOBAL) definition. Used for default implementations.
