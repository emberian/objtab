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

Relocation section '.rela.text' at offset 0x218 contains 2 entries:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
000000000010  000b0000011b R_AARCH64_CALL26  0000000000000000 add + 0
000000000020  000c0000011b R_AARCH64_CALL26  0000000000000000 multiply + 0

Relocation section '.rela.eh_frame' at offset 0x248 contains 1 entry:
  Offset          Info           Type           Sym. Value    Sym. Name + Addend
00000000001c  000200000105 R_AARCH64_PREL32  0000000000000000 .text + 0
```

Two relocations:
- At offset 0x11 in `.text`, patch with a PLT call to `add`
- At offset 0x20 in `.text`, patch with a PLT call to `multiply`

The `-4` addend accounts for the call instruction encoding on x86.

Let's see the code before linking:

```bash
$ objdump -d main.o

main.o:     file format elf64-littleaarch64


Disassembly of section .text:

0000000000000000 <main>:
   0:	a9be7bfd 	stp	x29, x30, [sp, #-32]!
   4:	910003fd 	mov	x29, sp
   8:	52800061 	mov	w1, #0x3                   	// #3
   c:	52800040 	mov	w0, #0x2                   	// #2
  10:	94000000 	bl	0 <add>
  14:	b9001fe0 	str	w0, [sp, #28]
  18:	52800081 	mov	w1, #0x4                   	// #4
  1c:	b9401fe0 	ldr	w0, [sp, #28]
  20:	94000000 	bl	0 <multiply>
  24:	b9001fe0 	str	w0, [sp, #28]
  28:	b9401fe0 	ldr	w0, [sp, #28]
  2c:	a8c27bfd 	ldp	x29, x30, [sp], #32
  30:	d65f03c0 	ret
```

That `e8 00 00 00 00` is a call instruction with address 0. The relocation fills it in.

After linking:

```bash
$ objdump -d program

program:     file format elf64-littleaarch64


Disassembly of section .init:

0000000000400458 <_init>:
  400458:	d503233f 	paciasp
  40045c:	a9bf7bfd 	stp	x29, x30, [sp, #-16]!
  400460:	910003fd 	mov	x29, sp
  400464:	94000039 	bl	400548 <call_weak_fn>
  400468:	a8c17bfd 	ldp	x29, x30, [sp], #16
  40046c:	d50323bf 	autiasp
  400470:	d65f03c0 	ret

Disassembly of section .plt:

0000000000400480 <.plt>:
  400480:	a9bf7bf0 	stp	x16, x30, [sp, #-16]!
  400484:	f00000f0 	adrp	x16, 41f000 <__abi_tag+0x1e824>
  400488:	f947fe11 	ldr	x17, [x16, #4088]
  40048c:	913fe210 	add	x16, x16, #0xff8
  400490:	d61f0220 	br	x17
  400494:	d503201f 	nop
  400498:	d503201f 	nop
  40049c:	d503201f 	nop

00000000004004a0 <__libc_start_main@plt>:
  4004a0:	90000110 	adrp	x16, 420000 <__libc_start_main@GLIBC_2.34>
  4004a4:	f9400211 	ldr	x17, [x16]
  4004a8:	91000210 	add	x16, x16, #0x0
  4004ac:	d61f0220 	br	x17

00000000004004b0 <__gmon_start__@plt>:
  4004b0:	90000110 	adrp	x16, 420000 <__libc_start_main@GLIBC_2.34>
  4004b4:	f9400611 	ldr	x17, [x16, #8]
  4004b8:	91002210 	add	x16, x16, #0x8
  4004bc:	d61f0220 	br	x17

00000000004004c0 <abort@plt>:
  4004c0:	90000110 	adrp	x16, 420000 <__libc_start_main@GLIBC_2.34>
  4004c4:	f9400a11 	ldr	x17, [x16, #16]
  4004c8:	91004210 	add	x16, x16, #0x10
  4004cc:	d61f0220 	br	x17

Disassembly of section .text:

0000000000400500 <_start>:
  400500:	d503245f 	bti	c
  400504:	d280001d 	mov	x29, #0x0                   	// #0
  400508:	d280001e 	mov	x30, #0x0                   	// #0
  40050c:	aa0003e5 	mov	x5, x0
  400510:	f94003e1 	ldr	x1, [sp]
  400514:	910023e2 	add	x2, sp, #0x8
  400518:	910003e6 	mov	x6, sp
  40051c:	90000000 	adrp	x0, 400000 <_init-0x458>
  400520:	9114d000 	add	x0, x0, #0x534
  400524:	d2800003 	mov	x3, #0x0                   	// #0
  400528:	d2800004 	mov	x4, #0x0                   	// #0
  40052c:	97ffffdd 	bl	4004a0 <__libc_start_main@plt>
  400530:	97ffffe4 	bl	4004c0 <abort@plt>

0000000000400534 <__wrap_main>:
  400534:	d503245f 	bti	c
  400538:	14000033 	b	400604 <main>
  40053c:	d503201f 	nop

0000000000400540 <_dl_relocate_static_pie>:
  400540:	d503245f 	bti	c
  400544:	d65f03c0 	ret

0000000000400548 <call_weak_fn>:
  400548:	f00000e0 	adrp	x0, 41f000 <__abi_tag+0x1e824>
  40054c:	f947ec00 	ldr	x0, [x0, #4056]
  400550:	b4000040 	cbz	x0, 400558 <call_weak_fn+0x10>
  400554:	17ffffd7 	b	4004b0 <__gmon_start__@plt>
  400558:	d65f03c0 	ret
  40055c:	d503201f 	nop

0000000000400560 <deregister_tm_clones>:
  400560:	90000100 	adrp	x0, 420000 <__libc_start_main@GLIBC_2.34>
  400564:	9100a000 	add	x0, x0, #0x28
  400568:	90000101 	adrp	x1, 420000 <__libc_start_main@GLIBC_2.34>
  40056c:	9100a021 	add	x1, x1, #0x28
  400570:	eb00003f 	cmp	x1, x0
  400574:	540000c0 	b.eq	40058c <deregister_tm_clones+0x2c>  // b.none
  400578:	f00000e1 	adrp	x1, 41f000 <__abi_tag+0x1e824>
  40057c:	f947e821 	ldr	x1, [x1, #4048]
  400580:	b4000061 	cbz	x1, 40058c <deregister_tm_clones+0x2c>
  400584:	aa0103f0 	mov	x16, x1
  400588:	d61f0200 	br	x16
  40058c:	d65f03c0 	ret

0000000000400590 <register_tm_clones>:
  400590:	90000100 	adrp	x0, 420000 <__libc_start_main@GLIBC_2.34>
  400594:	9100a000 	add	x0, x0, #0x28
  400598:	90000101 	adrp	x1, 420000 <__libc_start_main@GLIBC_2.34>
  40059c:	9100a021 	add	x1, x1, #0x28
  4005a0:	cb000021 	sub	x1, x1, x0
  4005a4:	d37ffc22 	lsr	x2, x1, #63
  4005a8:	8b810c41 	add	x1, x2, x1, asr #3
  4005ac:	9341fc21 	asr	x1, x1, #1
  4005b0:	b40000c1 	cbz	x1, 4005c8 <register_tm_clones+0x38>
  4005b4:	f00000e2 	adrp	x2, 41f000 <__abi_tag+0x1e824>
  4005b8:	f947f042 	ldr	x2, [x2, #4064]
  4005bc:	b4000062 	cbz	x2, 4005c8 <register_tm_clones+0x38>
  4005c0:	aa0203f0 	mov	x16, x2
  4005c4:	d61f0200 	br	x16
  4005c8:	d65f03c0 	ret

00000000004005cc <__do_global_dtors_aux>:
  4005cc:	a9be7bfd 	stp	x29, x30, [sp, #-32]!
  4005d0:	910003fd 	mov	x29, sp
  4005d4:	f9000bf3 	str	x19, [sp, #16]
  4005d8:	90000113 	adrp	x19, 420000 <__libc_start_main@GLIBC_2.34>
  4005dc:	3940a260 	ldrb	w0, [x19, #40]
  4005e0:	37000080 	tbnz	w0, #0, 4005f0 <__do_global_dtors_aux+0x24>
  4005e4:	97ffffdf 	bl	400560 <deregister_tm_clones>
  4005e8:	52800020 	mov	w0, #0x1                   	// #1
  4005ec:	3900a260 	strb	w0, [x19, #40]
  4005f0:	f9400bf3 	ldr	x19, [sp, #16]
  4005f4:	a8c27bfd 	ldp	x29, x30, [sp], #32
  4005f8:	d65f03c0 	ret
  4005fc:	d503201f 	nop

0000000000400600 <frame_dummy>:
  400600:	17ffffe4 	b	400590 <register_tm_clones>

0000000000400604 <main>:
  400604:	a9be7bfd 	stp	x29, x30, [sp, #-32]!
  400608:	910003fd 	mov	x29, sp
  40060c:	52800061 	mov	w1, #0x3                   	// #3
  400610:	52800040 	mov	w0, #0x2                   	// #2
  400614:	94000009 	bl	400638 <add>
  400618:	b9001fe0 	str	w0, [sp, #28]
  40061c:	52800081 	mov	w1, #0x4                   	// #4
  400620:	b9401fe0 	ldr	w0, [sp, #28]
  400624:	9400000d 	bl	400658 <multiply>
  400628:	b9001fe0 	str	w0, [sp, #28]
  40062c:	b9401fe0 	ldr	w0, [sp, #28]
  400630:	a8c27bfd 	ldp	x29, x30, [sp], #32
  400634:	d65f03c0 	ret

0000000000400638 <add>:
  400638:	d10043ff 	sub	sp, sp, #0x10
  40063c:	b9000fe0 	str	w0, [sp, #12]
  400640:	b9000be1 	str	w1, [sp, #8]
  400644:	b9400fe1 	ldr	w1, [sp, #12]
  400648:	b9400be0 	ldr	w0, [sp, #8]
  40064c:	0b000020 	add	w0, w1, w0
  400650:	910043ff 	add	sp, sp, #0x10
  400654:	d65f03c0 	ret

0000000000400658 <multiply>:
  400658:	d10043ff 	sub	sp, sp, #0x10
  40065c:	b9000fe0 	str	w0, [sp, #12]
  400660:	b9000be1 	str	w1, [sp, #8]
  400664:	b9400fe1 	ldr	w1, [sp, #12]
  400668:	b9400be0 	ldr	w0, [sp, #8]
  40066c:	1b007c20 	mul	w0, w1, w0
  400670:	910043ff 	add	sp, sp, #0x10
  400674:	d65f03c0 	ret

Disassembly of section .fini:

0000000000400678 <_fini>:
  400678:	d503233f 	paciasp
  40067c:	a9bf7bfd 	stp	x29, x30, [sp, #-16]!
  400680:	910003fd 	mov	x29, sp
  400684:	a8c17bfd 	ldp	x29, x30, [sp], #16
  400688:	d50323bf 	autiasp
  40068c:	d65f03c0 	ret
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
