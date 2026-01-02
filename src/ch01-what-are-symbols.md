# What Are Symbols Anyway?

Let's start with a simple C program:

```c
// math.c
int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}
```

```c
// main.c
extern int add(int, int);
extern int multiply(int, int);

int main() {
    int result = add(2, 3);
    result = multiply(result, 4);
    return result;
}
```

Two files. Compile them separately:

```bash
gcc -c math.c -o math.o
gcc -c main.c -o main.o
```

Now you have two **object files**. Neither is executable. They're intermediate products—compiled code with some unfinished business.

What's the unfinished business? Let's look at `main.o`:

```bash
$ nm main.o
                 U add
0000000000000000 T main
                 U multiply
```

Three symbols:
- `main` is defined here (that's what `T` means—it's in the `.text` section)
- `add` is **undefined** (`U`)—main.c calls it but doesn't define it
- `multiply` is also undefined

Now `math.o`:

```bash
$ nm math.o
0000000000000000 T add
0000000000000020 T multiply
```

Both `add` and `multiply` are defined here. No undefined symbols.

## The Core Concept

A **symbol** is a named reference to something in your code. Usually it's one of:

1. **A function** (`add`, `main`, `printf`)
2. **A global variable** (`errno`, `stdout`, `my_global_config`)
3. **A static variable** (file-scoped, but still needs a name internally)

Symbols are how separate compilation units talk to each other. When `main.c` says `add(2, 3)`, the compiler doesn't know where `add` is. It just emits a placeholder that says "call the function named `add`." The linker's job is to fill in that placeholder with an actual address.

## Symbol Properties

Every symbol has several properties:

### Name
The identifier. In C, what you see is (mostly) what you get. In C++, names get **mangled**:

```cpp
// C++
int add(int a, int b);           // Mangled: _Z3addii
int add(double a, double b);     // Mangled: _Z3adddd
namespace math {
    int add(int a, int b);       // Mangled: _ZN4math3addEii
}
```

Mangling encodes the function signature into the name, enabling function overloading. Different compilers use different mangling schemes (a fun source of ABI incompatibility).

### Binding
Who can see this symbol?

- **Global** (also called "external"): Visible to other object files. Default for functions and non-static globals.
- **Local**: Only visible within this object file. `static` functions and variables in C.
- **Weak**: Like global, but can be overridden. Default implementations live here.

```c
// Global - other files can call this
int public_function(void) { return 1; }

// Local - only this file can see it
static int private_function(void) { return 2; }

// Weak - can be overridden by a strong symbol
__attribute__((weak)) int maybe_override(void) { return 3; }
```

### Type
What kind of thing is this symbol?

- `NOTYPE`: Unknown/unspecified
- `OBJECT`: A data variable
- `FUNC`: A function
- `SECTION`: A section name
- `FILE`: Source file name

### Section
Where in the object file does this symbol live?

- `.text`: Executable code
- `.data`: Initialized data
- `.bss`: Uninitialized data (Block Started by Symbol—yes, really)
- `.rodata`: Read-only data (string literals, constants)
- `UND` or `*UND*`: Undefined—not in this file at all

### Value
Usually the offset within the section. For an undefined symbol, this is meaningless until linking.

## A JavaScript Analogy

If you've ever written:

```javascript
// utils.js
export function add(a, b) {
    return a + b;
}

// main.js
import { add } from './utils.js';
console.log(add(2, 3));
```

You've worked with symbols! The ES6 module system is conceptually identical:
- `export` makes a symbol global (visible outside the module)
- `import` declares a symbol as undefined locally (defined elsewhere)
- The bundler (or runtime module loader) resolves undefined symbols to their definitions

The difference? JavaScript modules are resolved at runtime (or bundle time by a bundler). Object files are designed to be resolved by a linker, which produces an executable *before* runtime.

## Symbol Resolution: The Linker's Job

When you run:

```bash
gcc main.o math.o -o program
```

The linker:

1. **Collects** all symbols from all input files
2. **Resolves** undefined symbols to their definitions
3. **Relocates** code to final addresses
4. **Emits** the final executable

If `add` is undefined in `main.o` but defined in `math.o`, the linker matches them up. If `add` were undefined everywhere? Linker error:

```
undefined reference to `add'
```

If `add` were defined in *two* files with global binding? Also an error:

```
multiple definition of `add'
```

(Unless one is weak—then the strong definition wins.)

## Why Not Just Use Addresses?

You might wonder: why have named symbols at all? Why not just use addresses?

The answer is **separate compilation**. When `main.c` is compiled, the compiler has no idea where `add` will end up. It might be in a different object file. It might be in a shared library. It might be in a library that doesn't exist yet.

Symbols are a level of indirection. They let us:
- Compile files independently
- Link against libraries without their source code
- Replace implementations (debugging, testing, optimization)
- Share code via dynamic libraries loaded at runtime

## Visibility Beyond Binding

Modern systems add another layer: **visibility**. This is separate from binding and controls whether a symbol is visible outside a shared library:

```c
// Always visible (default)
__attribute__((visibility("default")))
int public_api(void);

// Hidden - only visible within this shared library
__attribute__((visibility("hidden")))
int internal_helper(void);
```

This matters for shared libraries. A function can be global (callable from anywhere within the library) but hidden (not exported to programs using the library).

## Symbol Versioning

Large libraries (like glibc) use **symbol versioning** to maintain backward compatibility:

```c
// Old version
int old_function(void) __asm__("function@VERSION_1");

// New version with different behavior
int new_function(void) __asm__("function@@VERSION_2");
```

Programs linked against VERSION_1 continue getting the old behavior. New programs get VERSION_2. This is how glibc maintains ABI compatibility across decades of Linux distributions.

## Try It Yourself

Create those two files (`main.c` and `math.c`) and experiment:

```bash
# Compile to object files
gcc -c main.c math.c

# Examine symbols
nm main.o
nm math.o

# See more detail
readelf -s main.o

# Link and run
gcc main.o math.o -o program
./program
echo $?  # Should print 20 (add(2,3) = 5, multiply(5,4) = 20)
```

Try making `add` static in `math.c`. Watch the linker error. Try removing the `multiply` call from main—does the symbol still appear?

## Key Takeaways

1. **Symbols are names** for functions, variables, and other code elements
2. **Undefined symbols** are promises: "this exists somewhere, trust me"
3. **The linker** resolves undefined symbols to their definitions
4. **Binding** controls visibility: global, local, or weak
5. **This is not unlike ES6 modules**—the concepts transfer

## Looking Ahead

We've established that symbols are names, and that they live in object files. But we've been treating object files as black boxes—we know `nm` can list their symbols, but we haven't looked inside.

What *is* an object file, really? It's not just a bag of symbols. It's a structured container with headers, sections, and metadata. The symbol table is just one part of a larger architecture designed in the 1990s and still running on billions of devices today.

That architecture is called ELF—the Executable and Linkable Format. Understanding ELF means understanding how Linux, Android, and most of the world's servers actually load and run code. It also provides the conceptual foundation for understanding WebAssembly, which made different design choices for different reasons.

In the next chapter, we'll crack open an ELF file and examine its anatomy. You'll see where symbols live, how code is organized into sections, and why there are two different "views" of the same file. Bring your hex editor—we're going deep.
