# What You Never Knew About Symbols

> *"The linker is the most underappreciated tool in your entire toolchain."*

## Who This Book Is For

You're a web developer. You've been at it for a decade. You've built SPAs, wrestled with webpack configurations, optimized bundle sizes, and maybe even touched some WebAssembly. You know JavaScript intimately—its quirks, its runtime, its module systems (all seven of them).

But here's a question: **What actually happens when code becomes... code?**

Not the high-level "it gets compiled" answer. The real answer. The one involving symbol tables, relocations, sections, and the three different kinds of linkers that make your programs actually work.

## Why Should You Care?

As a web developer, you might think this is esoteric systems programming trivia. It's not. Here's why:

1. **WebAssembly is eating the world.** When you compile Rust, C++, or Go to WASM, you're dealing with object files. Understanding them means understanding why your WASM module is 2MB instead of 200KB.

2. **Native modules exist.** Node.js addons, Electron apps, and native bindings all involve linking. When `npm install` fails with "undefined symbol," you'll know exactly what that means.

3. **Bundlers are linkers.** Webpack, Rollup, esbuild—they all do a form of linking. The concepts are the same: resolve symbols, eliminate dead code, produce a final artifact.

4. **Debugging gets deeper.** Stack traces, source maps, and debugging symbols all trace back to concepts we'll cover here.

5. **It's fascinating.** Seriously. The elegance of how a bunch of separate `.o` files become a running program is beautiful engineering.

## What We'll Cover

This book is structured as a journey from "what is an object file" to "I can read ELF headers for fun":

**Part I: Foundations**
- What symbols are and why they exist
- Deep dive into ELF (the format running on billions of devices)
- WebAssembly's object format (yes, WASM has one)

**Part II: Symbol Tables**
- How symbol tables work and what's in them
- Relocations: the placeholder system that makes linking possible

**Part III: Linking**
- Static linking: the simple case
- Dynamic linking: shared libraries and the runtime
- Runtime linking: `dlopen()` and its friends

**Part IV: Bringing It Together**
- ELF vs WASM: a detailed comparison
- Practical applications: debugging, optimization, and tooling

## Prerequisites

You should be comfortable with:
- Reading code in multiple languages
- Basic understanding of how compilation works (source → compiler → something → executable)
- Command-line tools
- A willingness to look at hexadecimal

You don't need:
- Prior systems programming experience
- Assembly language knowledge (though we'll see some)
- A Computer Science degree

## A Note on Tools

Throughout this book, we'll use various tools to inspect object files. Most examples use:

```bash
# For ELF files
readelf -a binary      # Inspect ELF structure
nm binary              # List symbols
objdump -d binary      # Disassemble

# For WASM files
wasm-objdump -x file.wasm  # Inspect WASM structure
wasm2wat file.wasm         # Convert to text format
```

On macOS, ELF tools may need to be installed via Homebrew (`binutils`). For WASM tools, the WebAssembly Binary Toolkit (WABT) is your friend.

## Let's Begin

Turn the page. We're going to start with the most fundamental question: what even *is* a symbol?

The answer might surprise you.
