# Dynamic Linking & Shared Libraries

The year is 1988. Your Sun workstation has 4MB of RAM—a generous amount. You're running a window manager, a text editor, a compiler, and a mail client. Each program uses the C library. With static linking, that's four copies of libc in memory. On a 4MB machine, that's painful.

Sun's engineers had an idea: what if programs could *share* library code? Load libc once, map it into every process that needs it. One copy in physical memory, appearing in many virtual address spaces. Suddenly your 4MB machine feels roomier.

This was the birth of shared libraries. The idea spread to System V, then to Linux, then to everywhere. Today, virtually every program on your system uses them. That's why `ls` is 130KB instead of 2MB—it doesn't include libc, just a reference to it.

But sharing introduces complexity. If libc can load at different addresses in different processes, how does code find the functions it needs? If libraries can be updated independently of programs, how do you handle version mismatches? If the library isn't loaded until runtime, when do symbol errors appear?

Dynamic linking solves these problems—elegantly, if you understand it; mysteriously, if you don't. Let's understand it.

Static linking has a problem: duplication. If 50 programs use libc, you have 50 copies of libc in memory. That's wasteful.

Dynamic linking solves this. Shared libraries are loaded once and mapped into every process that needs them. Memory saved. Updates applied once. The world rejoices.

But there's complexity. Let's understand how it works.

## Shared Libraries (.so files)

A shared library is an ELF file designed to be loaded at runtime:

```bash
# Create shared library
gcc -fPIC -shared math.c -o libmath.so

# Link against it
gcc main.c -L. -lmath -o program

# Run (library must be findable)
LD_LIBRARY_PATH=. ./program
```

The executable doesn't contain `libmath.so`'s code. It contains a **reference** to the library.

## What's in a Shared Library?

A shared library is almost like an executable:

```bash
$ file libmath.so
libmath.so: ELF 64-bit LSB shared object, x86-64, version 1 (SYSV),
dynamically linked, not stripped

$ readelf -h libmath.so | grep Type
readelf: Error: 'libmath.so': No such file
```

Type `DYN` (not `EXEC`). It has:
- Code and data sections
- Symbol table (`.dynsym`)—exported functions
- Relocation entries—for runtime patching
- No entry point (it's a library, not a program)

## Position-Independent Code (PIC)

Shared libraries must work at any address. Why? Because multiple programs load them, and each program has different memory layouts.

```
Process A:               Process B:
0x400000: program A      0x400000: program B
0x7f0000: libc.so        0x7f0000: libX.so
0x7f1000: libmath.so     0x7f1000: libc.so  ← different address!
                         0x7f2000: libmath.so
```

The same `libmath.so` is at 0x7f1000 in process A but 0x7f2000 in process B. The code must work at both addresses.

### Global Offset Table (GOT)

PIC code accesses global data through the GOT:

```c
// With PIC
extern int global_var;
int read_global(void) {
    return global_var;  // Goes through GOT
}
```

Generated assembly (simplified):

```asm
read_global:
    mov    global_var@GOTPCREL(%rip), %rax   # Load GOT entry address
    mov    (%rax), %eax                       # Load value from GOT entry
    ret
```

The GOT entry contains `global_var`'s actual address, filled in by the dynamic linker.

### Why Not Just Relocate Everything?

You might ask: why not just patch all addresses at load time?

The answer is **code sharing**. Shared libraries use a clever trick: the code is read-only and shared between processes. Only the data (including GOT) is private per-process.

```
Physical Memory:
┌─────────────────┐
│ libmath.so      │
│ .text (shared)  │ ← Read-only, same physical pages for all
│                 │
│ .data (COW)     │ ← Copy-on-write, per-process
│ .got (private)  │ ← Each process has own GOT
└─────────────────┘
```

If we patched `.text` with process-specific addresses, we couldn't share it.

## The Dynamic Linker

When you run a dynamically linked program, the kernel doesn't just jump to `main`. It:

1. Loads the executable
2. Reads the `INTERP` segment to find the dynamic linker
3. Loads the dynamic linker
4. Jumps to the dynamic linker's entry point

The dynamic linker (`ld-linux.so` on Linux) then:

1. Reads the executable's dynamic section
2. Loads all required shared libraries
3. Performs relocations
4. Calls constructors
5. Finally jumps to the executable's entry point

You can see this:

```bash
$ readelf -l /bin/ls | grep INTERP
  INTERP         0x0000000000000324 0x0000000000000324 0x0000000000000324
$ readelf -p .interp /bin/ls

String dump of section '.interp':
  [     0]  /lib/ld-linux-aarch64.so.1
```

## Finding Shared Libraries

How does the dynamic linker find libraries? In order:

1. **`DT_RPATH`** / **`DT_RUNPATH`**: Paths embedded in the executable
2. **`LD_LIBRARY_PATH`**: Environment variable
3. **`/etc/ld.so.cache`**: Cached library locations
4. **Default paths**: `/lib`, `/usr/lib`, etc.

```bash
$ ldd /bin/ls
    linux-vdso.so.1 (0x00007ffd4b5fc000)
    libselinux.so.1 => /lib/x86_64-linux-gnu/libselinux.so.1 (0x00007f4e2d800000)
    libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007f4e2d400000)
    ...
```

## Symbol Resolution

Dynamic linking resolves symbols at load time (or lazily at call time):

### Eager Binding

By default on some systems, or with `LD_BIND_NOW=1`:

```bash
# All symbols resolved at load time
LD_BIND_NOW=1 ./program
```

Slower startup but no first-call overhead.

### Lazy Binding (PLT)

Default on most Linux systems. Functions are resolved on first call:

```asm
printf@plt:
    jmp    *printf@GOTPLT(%rip)  # Jump through GOT
    push   $0                     # First call: push index
    jmp    .plt                   # Jump to resolver
```

First call:
1. GOT entry points back to PLT (the push/jmp)
2. Resolver is called
3. Dynamic linker finds `printf`
4. GOT entry updated to point to real `printf`

Second call:
1. GOT entry points to real `printf`
2. Direct jump, no resolver involved

## Symbol Interposition

One powerful feature of dynamic linking: **symbol interposition**. A symbol in a later library can override one in an earlier library:

```bash
# malloc_debug.so defines malloc
LD_PRELOAD=./malloc_debug.so ./program
```

Now every `malloc` call goes to your debug version. This enables:
- Memory debugging (Valgrind, AddressSanitizer)
- Performance profiling
- Mocking for tests

### Symbol Visibility

If you don't want interposition (for performance):

```c
__attribute__((visibility("protected")))
void internal_function(void);
```

Protected visibility means calls within the library always use the local definition.

## Version Scripts

Large libraries maintain ABI compatibility with **symbol versioning**:

```
// version.script
MYLIB_1.0 {
    global: public_function;
    local: *;
};

MYLIB_2.0 {
    global: new_function;
} MYLIB_1.0;
```

```bash
gcc -shared lib.c -Wl,--version-script=version.script -o libmy.so
```

Programs linked against MYLIB_1.0 continue working even when MYLIB_2.0 changes internal functions.

## Dynamic Section

The `.dynamic` section contains tags the dynamic linker needs:

```bash
$ readelf -d /bin/ls

Dynamic section at offset 0x2f908 contains 31 entries:
  Tag        Type                         Name/Value
 0x0000000000000001 (NEEDED)             Shared library: [libselinux.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libcap.so.2]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [ld-linux-aarch64.so.1]
 0x000000000000000c (INIT)               0x3970
 0x000000000000000d (FINI)               0x1bb14
 0x0000000000000019 (INIT_ARRAY)         0x3ef20
 0x000000000000001b (INIT_ARRAYSZ)       8 (bytes)
 0x000000000000001a (FINI_ARRAY)         0x3ef28
 0x000000000000001c (FINI_ARRAYSZ)       8 (bytes)
 0x000000006ffffef5 (GNU_HASH)           0x340
 0x0000000000000005 (STRTAB)             0x10a0
 0x0000000000000006 (SYMTAB)             0x380
 0x000000000000000a (STRSZ)              1584 (bytes)
 0x000000000000000b (SYMENT)             24 (bytes)
 0x0000000000000015 (DEBUG)              0x0
 0x0000000000000003 (PLTGOT)             0x3fb38
 0x0000000000000002 (PLTRELSZ)           2760 (bytes)
 0x0000000000000014 (PLTREL)             RELA
 0x0000000000000017 (JMPREL)             0x2ea8
 0x0000000000000007 (RELA)               0x1888
 0x0000000000000008 (RELASZ)             5664 (bytes)
 0x0000000000000009 (RELAENT)            24 (bytes)
 0x0000000070000001 (AARCH64_BTI_PLT)    
 0x000000000000001e (FLAGS)              BIND_NOW
 0x000000006ffffffb (FLAGS_1)            Flags: NOW PIE
 0x000000006ffffffe (VERNEED)            0x17e8
 0x000000006fffffff (VERNEEDNUM)         3
 0x000000006ffffff0 (VERSYM)             0x16d0
 0x000000006ffffff9 (RELACOUNT)          218
 0x0000000000000000 (NULL)               0x0
```

Key tags:
- `NEEDED`: Shared libraries this file depends on
- `INIT`/`FINI`: Constructor/destructor functions
- `SYMTAB`/`STRTAB`: Symbol and string tables
- `RELA`/`RELASZ`: Relocation information
- `SONAME`: Library's canonical name

## SONAME and Versioning

Libraries have a **soname** (shared object name):

```bash
$ objdump -p /lib/x86_64-linux-gnu/libc.so.6 | grep SONAME
  SONAME               libc.so.6
```

This enables version management:

```
Filesystem:
libfoo.so.1.2.3  (actual file)
libfoo.so.1   -> libfoo.so.1.2.3  (soname symlink)
libfoo.so     -> libfoo.so.1      (development symlink)
```

- Programs link against `libfoo.so` (resolves to current version)
- At runtime, they request `libfoo.so.1` (the soname)
- Compatible updates (`1.2.3` → `1.2.4`) just update the symlink

Major version bumps (incompatible changes) get a new soname (`libfoo.so.2`).

## Constructors and Destructors

Shared libraries can have initialization code:

```c
__attribute__((constructor))
void init(void) {
    printf("Library loaded\n");
}

__attribute__((destructor))
void cleanup(void) {
    printf("Library unloading\n");
}
```

These run automatically at load/unload time. Useful for:
- Registering with a framework
- Allocating global resources
- Setting up logging

## WASM "Dynamic Linking"

WASM doesn't have traditional dynamic linking, but it has something similar: **module instantiation with imports**.

```javascript
// JavaScript host
const mathModule = await WebAssembly.instantiate(mathBytes, {
    env: {
        memory: new WebAssembly.Memory({ initial: 1 }),
        log: console.log,
    }
});

const mainModule = await WebAssembly.instantiate(mainBytes, {
    env: {
        memory: mathModule.instance.exports.memory,
        add: mathModule.instance.exports.add,
    }
});
```

Imports are resolved at instantiation time. This is more like dynamic linking than static linking, but:
- No lazy binding (all imports resolved upfront)
- No interposition (imports are explicit)
- No versioning (the host controls what's provided)

### WASM Dynamic Linking Conventions

There's ongoing work on WASM dynamic linking:

```c
// Shared library exporting from WASM
__attribute__((import_module("env"), import_name("add")))
extern int add(int, int);

__attribute__((export_name("public_function")))
int public_function(void);
```

The Emscripten toolchain has `-sMAIN_MODULE` and `-sSIDE_MODULE` for building dynamically-linked WASM:

```bash
emcc -sMAIN_MODULE=1 main.c -o main.js
emcc -sSIDE_MODULE=1 lib.c -o lib.wasm
```

## Performance Considerations

Dynamic linking has overhead:

1. **Load time**: Finding and loading libraries
2. **Relocation time**: Patching GOT entries
3. **Call overhead**: PLT indirection (first call)
4. **Memory overhead**: GOT, PLT, dynamic sections

For performance-critical code:
- Use `-fvisibility=hidden` and export only what's needed
- Consider `-fno-plt` (direct GOT calls, removes PLT)
- Use `LD_BIND_NOW` for deterministic latency

## Debugging Dynamic Linking

```bash
# See library search
LD_DEBUG=libs ./program

# See symbol resolution
LD_DEBUG=symbols ./program

# See relocations
LD_DEBUG=reloc ./program

# See everything
LD_DEBUG=all ./program 2>&1 | head -100
```

This is invaluable for debugging "symbol not found" errors.

## The Trade-off Summary

| Aspect | Static | Dynamic |
|--------|--------|---------|
| Binary size | Larger | Smaller |
| Memory usage | Higher (no sharing) | Lower (sharing) |
| Startup time | Faster | Slower |
| Updates | Requires recompile | Just update library |
| Deployment | Single file | Manage dependencies |
| Security updates | Manual | Automatic |

## Key Takeaways

1. **Shared libraries are loaded at runtime**, not linked in
2. **PIC** enables code sharing between processes
3. **GOT/PLT** enable position-independent access to external symbols
4. **The dynamic linker** resolves symbols and performs relocations
5. **Symbol interposition** allows overriding functions at runtime
6. **SONAME versioning** enables backward-compatible updates
7. **WASM has import-based linking** at instantiation time

## Beyond Startup

Dynamic linking happens when your program starts. The dynamic linker loads libraries, resolves symbols, and by the time `main()` runs, everything is wired up. For most programs, this is enough.

But what if you don't know which code you'll need until the program is running? A text editor loading syntax highlighters based on file type. A server loading authentication modules from a config file. A game engine loading mods that didn't exist when the game shipped.

This is *runtime* linking—the ability to load code while the program runs, look up symbols by name, and call functions that weren't known at compile time. It's the foundation of plugin architectures, hot reloading, and adaptive systems.

In the next chapter, we'll explore the `dlopen` API and its WASM equivalents. You'll learn how to build plugin systems, wrap library functions for debugging, and understand the security implications of loading arbitrary code. If dynamic linking is linking at program start, runtime linking is linking whenever you want.
