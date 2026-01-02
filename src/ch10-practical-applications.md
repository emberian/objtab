# Practical Applications for Web Developers

You've learned about object files, symbols, relocations, and linkers. Now let's apply this knowledge to problems you actually face.

## Debugging Native Module Failures

You're installing an npm package and see:

```bash
$ npm install sharp

node-pre-gyp ERR! Tried to download(404): https://github.com/.../sharp-v0.32.0-linux-x64.tar.gz
gyp ERR! build error
gyp ERR! node-gyp -v v9.0.0
...
Error: Cannot find module '../build/Release/sharp-linux-x64.node'
```

What's happening? Native modules like `sharp` (image processing) or `bcrypt` (crypto) include compiled code. They're ELF shared objects (`.node` files are just `.so` files).

### Understanding the Error

1. **Pre-built binary not available** (the 404)
2. **Fell back to source compilation** (gyp)
3. **Compilation failed**

Check what's actually there:

```bash
$ ls node_modules/sharp/build/Release/
ls: cannot access 'node_modules/sharp/build/Release/': No such file or directory
$ file node_modules/sharp/build/Release/sharp-linux-x64.node
# If it exists: ELF 64-bit LSB shared object...
```

### Common Fixes

**Missing build tools:**
```bash
# Ubuntu/Debian
sudo apt install build-essential

# macOS
xcode-select --install
```

**Wrong Node.js version:**
```bash
$ node -p process.versions.modules
# Returns ABI version (e.g., 108 for Node 20)

# The .node file was compiled for a different ABI
# Solution: rebuild or use matching Node version
npm rebuild sharp
```

**Missing libraries:**
```bash
$ ldd node_modules/sharp/build/Release/sharp-linux-x64.node
    linux-vdso.so.1 (0x00007ffc...)
    libvips.so.42 => not found  # ← Problem!
    libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6

# Fix: install the missing library
sudo apt install libvips-dev
```

## Optimizing WASM Binary Size

Your Rust-to-WASM module is 2MB. Your users are sad. Let's fix it.

### Measure First

```bash
$ wasm-objdump -h my_module.wasm

Sections:
     Type start=0x0000000a end=0x00000134 (size=0x0000012a) count: 30
     Import start=0x00000137 end=0x00000198 (size=0x00000061) count: 3
     Function start=0x0000019b end=0x00000498 (size=0x000002fd) count: 380
     ...
     Code start=0x00001a23 end=0x001f8271 (size=0x001f684e) count: 380
                                          ↑
                                    This is your code: ~2MB
```

Most of the size is in the Code section. Let's analyze it:

```bash
# If you have twiggy (cargo install twiggy)
twiggy top my_module.wasm

# Shows biggest functions:
 Shallow Bytes │ Shallow % │ Item
───────────────┼───────────┼──────────────────────
       234,567 │    11.12% │ regex_automata::...
       198,234 │     9.40% │ serde_json::de::...
       ...
```

### Reduce Dependencies

That regex library is 11% of your binary. Do you need full regex? Maybe:
- Use a simpler pattern matcher
- Process regex on the JS side
- Use a lighter regex crate

### Enable LTO and Size Optimization

```toml
# Cargo.toml
[profile.release]
lto = true
opt-level = "z"  # Optimize for size
codegen-units = 1
```

### Strip Debug Info

```bash
# Using wasm-strip
wasm-strip my_module.wasm

# Or in Cargo.toml
[profile.release]
strip = true
```

### Use wasm-opt

```bash
# Part of Binaryen toolkit
wasm-opt -Oz -o optimized.wasm my_module.wasm

# Can reduce by 10-20%
```

### Result

Typical savings from all of the above:

| Stage | Size |
|-------|------|
| Initial build | 2.1 MB |
| + Release build | 1.4 MB |
| + LTO + opt-level z | 800 KB |
| + strip | 750 KB |
| + wasm-opt | 620 KB |
| + dependency audit | 400 KB |

That's an 80% reduction.

## Understanding Bundle Splitting

Modern bundlers (webpack, Rollup, esbuild) do linking. Understanding linker concepts helps you configure them.

### Dead Code Elimination = Linker GC

```javascript
// utils.js
export function usedFunction() { return 1; }
export function unusedFunction() { return 2; }  // Tree-shaken

// main.js
import { usedFunction } from './utils.js';
console.log(usedFunction());
```

This is the same as linker `--gc-sections`. The bundler traces from entry points, keeping only reachable code.

**Gotcha**: Side effects prevent tree-shaking:

```javascript
// analytics.js
console.log("Analytics loaded");  // Side effect!
export function track() { }
```

Even if `track` is unused, the file runs. Mark it pure:

```javascript
// package.json
{
  "sideEffects": false  // Or list specific files
}
```

### Code Splitting = Shared Libraries

```javascript
// Uses dynamic import
const heavyModule = await import('./heavy.js');
```

The bundler creates a separate chunk—like a shared library. It's loaded on demand.

**Tip**: Shared code goes into common chunks:

```javascript
// webpack.config.js
optimization: {
  splitChunks: {
    chunks: 'all',
    // Code used by multiple entry points becomes shared
  }
}
```

This is linker thinking: identify common symbols, extract to shared unit.

## Building Node.js Native Addons

Want to write a native addon? You're authoring an ELF shared object.

### Using node-addon-api

```cpp
// addon.cc
#include <napi.h>

Napi::Number Add(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    double a = info[0].As<Napi::Number>().DoubleValue();
    double b = info[1].As<Napi::Number>().DoubleValue();
    return Napi::Number::New(env, a + b);
}

Napi::Object Init(Napi::Env env, Napi::Object exports) {
    exports.Set("add", Napi::Function::New(env, Add));
    return exports;
}

NODE_API_MODULE(addon, Init)
```

```python
# binding.gyp
{
  "targets": [{
    "target_name": "addon",
    "sources": ["addon.cc"],
    "include_dirs": [
      "<!@(node -p \"require('node-addon-api').include\")"
    ],
    "defines": ["NAPI_DISABLE_CPP_EXCEPTIONS"]
  }]
}
```

Build and examine:

```bash
$ npm install
$ node-gyp build

$ file build/Release/addon.node
addon.node: ELF 64-bit LSB shared object, x86-64

$ nm -D build/Release/addon.node | grep ' T '
0000000000001234 T napi_register_module_v1  # The entry point
```

### Debugging Symbol Issues

```bash
$ node -e "require('./build/Release/addon')"
Error: /path/addon.node: undefined symbol: _ZN4Napi5Value9As...

# Decode the symbol
$ c++filt _ZN4Napi5Value9As...
Napi::Value::As<Napi::Number>()

# Missing: linked against wrong version of node-addon-api
# Fix: check include paths, rebuild with correct headers
```

## Inspecting What Your Server Loads

Your Node.js app has latency spikes. Is it library loading?

```bash
$ LD_DEBUG=files node app.js 2>&1 | head -50

# Count loaded libraries
$ cat /proc/$(pgrep -f "node app.js")/maps | grep '\.so' | wc -l

# Check for relocations at startup
$ LD_DEBUG=statistics node app.js 2>&1 | grep runtime
```

If many libraries load lazily (PLT overhead on first call), consider:

```bash
# Force eager binding for consistent latency
LD_BIND_NOW=1 node app.js
```

## WASM in Production

### Loading Strategies

**Streaming compilation** (fastest):
```javascript
const module = await WebAssembly.compileStreaming(fetch('/module.wasm'));
const instance = await WebAssembly.instantiate(module, imports);
```

**Caching compiled modules** (for repeat visits):
```javascript
// Using IndexedDB to cache compiled module
const cache = await caches.open('wasm-cache');
const response = await cache.match('/module.wasm');

if (response) {
    const module = await WebAssembly.compileStreaming(response);
    // ...
} else {
    const response = await fetch('/module.wasm');
    cache.put('/module.wasm', response.clone());
    const module = await WebAssembly.compileStreaming(response);
    // ...
}
```

### Memory Management

WASM uses linear memory. Watch for growth:

```javascript
const memory = instance.exports.memory;

// Check current size (in pages, 64KB each)
console.log('Memory pages:', memory.buffer.byteLength / 65536);

// Grow if needed
memory.grow(10);  // Add 10 pages (640KB)
```

**Warning**: Growing memory can move the buffer, invalidating views:

```javascript
const memory = instance.exports.memory;
let view = new Uint8Array(memory.buffer);

memory.grow(1);

// view is now detached!
// Must recreate:
view = new Uint8Array(memory.buffer);
```

## Debugging Tools Summary

| Task | ELF Tool | WASM Tool |
|------|----------|-----------|
| List symbols | `nm`, `readelf -s` | `wasm-objdump -t` |
| Show sections | `readelf -S` | `wasm-objdump -h` |
| Disassemble | `objdump -d` | `wasm2wat` |
| Check dependencies | `ldd` | (inspect imports) |
| Debug loading | `LD_DEBUG=all` | Browser DevTools |
| Analyze size | `bloaty` | `twiggy` |
| Strip debug info | `strip` | `wasm-strip` |
| Optimize | (compiler flags) | `wasm-opt` |

## Quick Reference: Error Messages

| Error | Meaning | Fix |
|-------|---------|-----|
| `undefined symbol: X` | Symbol X not found in any library | Check ldd output, install missing lib |
| `version 'GLIBC_2.XX' not found` | Built against newer glibc | Rebuild on older system or use static linking |
| `ELF interpreter not found` | Wrong architecture or broken binary | Check `file` output, ensure 64-bit matches |
| `LinkError: import X not found` | WASM import not provided | Check your imports object in JS |
| `RuntimeError: unreachable` | WASM trap (assertion/bounds) | Debug with source maps |

## Key Takeaways

1. **Native module errors** are often ELF linking issues—use `ldd`, `nm`, `readelf`
2. **WASM binary size** can be reduced 50-80% with proper optimization
3. **Bundlers are linkers**—same concepts of tree-shaking and code splitting
4. **Native addons** are shared objects—debug them like any ELF file
5. **Streaming compilation** and caching make WASM load fast
6. **Know your tools**—the debugging commands will save hours

You now understand what happens between source code and running program. Use this knowledge to debug faster, optimize better, and build more robust systems.
