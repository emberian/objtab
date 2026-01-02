# Runtime Linking & Loading

Dynamic linking happens at program startup. But what if you want to load code *during* execution? Maybe based on configuration, user input, or plugin architecture?

That's runtime linking. Load a library, look up symbols, call functions—all while the program runs.

## The dlopen API

On Unix-like systems, four functions handle runtime linking:

```c
#include <dlfcn.h>

// Open a shared library
void *dlopen(const char *filename, int flags);

// Find a symbol
void *dlsym(void *handle, const char *symbol);

// Close a library
int dlclose(void *handle);

// Get error message
char *dlerror(void);
```

### Opening a Library

```c
void *handle = dlopen("./libplugin.so", RTLD_NOW);
if (!handle) {
    fprintf(stderr, "dlopen failed: %s\n", dlerror());
    exit(1);
}
```

Flags control behavior:

| Flag | Meaning |
|------|---------|
| `RTLD_NOW` | Resolve all symbols immediately |
| `RTLD_LAZY` | Resolve symbols on first use |
| `RTLD_GLOBAL` | Symbols available for subsequent loads |
| `RTLD_LOCAL` | Symbols not available to other libraries |
| `RTLD_NODELETE` | Don't unload on dlclose |
| `RTLD_NOLOAD` | Don't load, just check if loaded |

### Finding Symbols

```c
typedef int (*add_func)(int, int);

add_func add = (add_func)dlsym(handle, "add");
if (!add) {
    fprintf(stderr, "dlsym failed: %s\n", dlerror());
    exit(1);
}

int result = add(2, 3);  // Call the function!
```

`dlsym` returns a void pointer. You cast it to the appropriate function pointer type.

### Closing

```c
if (dlclose(handle) != 0) {
    fprintf(stderr, "dlclose failed: %s\n", dlerror());
}
```

After `dlclose`, the handle is invalid. The library might be unloaded (unless other references exist).

## A Complete Example

```c
// plugin_user.c
#include <stdio.h>
#include <dlfcn.h>

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <plugin.so>\n", argv[0]);
        return 1;
    }
    
    // Load the plugin
    void *handle = dlopen(argv[1], RTLD_NOW);
    if (!handle) {
        fprintf(stderr, "Error: %s\n", dlerror());
        return 1;
    }
    
    // Find the entry point
    typedef void (*plugin_init_func)(void);
    plugin_init_func init = dlsym(handle, "plugin_init");
    if (!init) {
        fprintf(stderr, "Error: %s\n", dlerror());
        dlclose(handle);
        return 1;
    }
    
    // Call it
    init();
    
    // Clean up
    dlclose(handle);
    return 0;
}
```

```c
// plugin.c
#include <stdio.h>

void plugin_init(void) {
    printf("Plugin initialized!\n");
}
```

```bash
# Build
gcc -shared -fPIC plugin.c -o plugin.so
gcc plugin_user.c -ldl -o plugin_user

# Run
./plugin_user ./plugin.so
# Output: Plugin initialized!
```

## Plugin Architectures

Many applications use dlopen for plugins:

### Defined Interface

```c
// plugin_api.h - shared between host and plugins
struct plugin_api {
    const char *name;
    const char *version;
    int (*init)(void *host_context);
    void (*shutdown)(void);
    int (*process)(void *data, size_t len);
};

// Each plugin exports this symbol
extern struct plugin_api plugin;
```

```c
// my_plugin.c
#include "plugin_api.h"

static int init(void *ctx) { /* ... */ return 0; }
static void shutdown(void) { /* ... */ }
static int process(void *data, size_t len) { /* ... */ return 0; }

struct plugin_api plugin = {
    .name = "My Plugin",
    .version = "1.0",
    .init = init,
    .shutdown = shutdown,
    .process = process,
};
```

```c
// host.c
void load_plugin(const char *path) {
    void *handle = dlopen(path, RTLD_NOW);
    struct plugin_api *api = dlsym(handle, "plugin");
    
    printf("Loaded plugin: %s v%s\n", api->name, api->version);
    api->init(host_context);
    
    // Store handle and api for later use
}
```

Real examples:
- **Apache modules** (mod_ssl, mod_php)
- **Nginx modules**
- **PostgreSQL extensions**
- **Node.js native addons**

## Symbol Lookup Variants

`dlsym` has variants for different lookup behaviors:

### `RTLD_DEFAULT` and `RTLD_NEXT`

```c
// Find symbol in default search order (all loaded libraries)
void *sym = dlsym(RTLD_DEFAULT, "malloc");

// Find the NEXT definition (for interposition)
void *real_malloc = dlsym(RTLD_NEXT, "malloc");
```

`RTLD_NEXT` is how you wrap library functions:

```c
// malloc_wrapper.c
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>

void *malloc(size_t size) {
    static void *(*real_malloc)(size_t) = NULL;
    if (!real_malloc) {
        real_malloc = dlsym(RTLD_NEXT, "malloc");
    }
    
    void *ptr = real_malloc(size);
    fprintf(stderr, "malloc(%zu) = %p\n", size, ptr);
    return ptr;
}
```

```bash
gcc -shared -fPIC malloc_wrapper.c -o malloc_wrapper.so -ldl
LD_PRELOAD=./malloc_wrapper.so ls
# Every malloc is logged!
```

### dladdr: Reverse Lookup

Given an address, find which library and symbol it belongs to:

```c
#include <dlfcn.h>

Dl_info info;
if (dladdr(some_function_ptr, &info)) {
    printf("Function is in: %s\n", info.dli_fname);
    printf("Nearest symbol: %s\n", info.dli_sname);
}
```

Useful for crash reporters and profilers.

## Windows Equivalent

Windows has similar functions with different names:

| Unix | Windows |
|------|---------|
| `dlopen` | `LoadLibrary` |
| `dlsym` | `GetProcAddress` |
| `dlclose` | `FreeLibrary` |
| `dlerror` | `GetLastError` |

```c
// Windows version
HMODULE handle = LoadLibrary("plugin.dll");
FARPROC func = GetProcAddress(handle, "plugin_init");
FreeLibrary(handle);
```

## WASM Dynamic Loading

WebAssembly modules can be loaded dynamically in JavaScript:

```javascript
async function loadPlugin(url) {
    // Fetch the module
    const response = await fetch(url);
    const bytes = await response.arrayBuffer();
    
    // Compile and instantiate
    const module = await WebAssembly.compile(bytes);
    const instance = await WebAssembly.instantiate(module, {
        env: {
            memory: sharedMemory,
            log: console.log,
        }
    });
    
    // Call exported function
    instance.exports.plugin_init();
    
    return instance;
}

// Load plugins based on configuration
for (const pluginUrl of config.plugins) {
    await loadPlugin(pluginUrl);
}
```

The JavaScript host acts as the dynamic linker, providing imports and managing module lifecycle.

### WASM Module Caching

`WebAssembly.compile()` is expensive. Cache compiled modules:

```javascript
const moduleCache = new Map();

async function getModule(url) {
    if (moduleCache.has(url)) {
        return moduleCache.get(url);
    }
    
    const response = await fetch(url);
    const bytes = await response.arrayBuffer();
    const module = await WebAssembly.compile(bytes);
    
    moduleCache.set(url, module);
    return module;
}
```

Even better: use `WebAssembly.compileStreaming()` to compile while downloading:

```javascript
const module = await WebAssembly.compileStreaming(fetch(url));
```

## Thread Safety

`dlopen` and friends have thread-safety considerations:

- **dlopen/dlclose**: Generally thread-safe on modern systems
- **dlerror**: Returns thread-local error (thread-safe)
- **Library constructors**: Run with a lock held
- **Symbol resolution during lazy binding**: Thread-safe

But your plugins must be thread-safe too. A plugin's `init` function might be called from multiple threads.

## Hot Reloading

Runtime linking enables hot reloading—updating code without restarting:

```c
void *current_handle = NULL;
struct plugin_api *current_api = NULL;

void reload_plugin(const char *path) {
    // Load new version first (keeps old version if this fails)
    void *new_handle = dlopen(path, RTLD_NOW);
    if (!new_handle) {
        fprintf(stderr, "Failed to load: %s\n", dlerror());
        return;
    }
    
    struct plugin_api *new_api = dlsym(new_handle, "plugin");
    if (!new_api) {
        fprintf(stderr, "No plugin symbol: %s\n", dlerror());
        dlclose(new_handle);
        return;
    }
    
    // Swap (atomically if needed)
    struct plugin_api *old_api = current_api;
    void *old_handle = current_handle;
    
    current_api = new_api;
    current_handle = new_handle;
    
    // Clean up old
    if (old_api) old_api->shutdown();
    if (old_handle) dlclose(old_handle);
    
    // Initialize new
    current_api->init(host_context);
}
```

Game engines often use this for rapid iteration during development.

## Security Considerations

Runtime linking is powerful but risky:

### Path Injection

Never construct library paths from user input without validation:

```c
// DANGEROUS
char path[256];
snprintf(path, sizeof(path), "./plugins/%s.so", user_input);
dlopen(path, RTLD_NOW);  // User could specify "../../../etc/passwd"

// SAFER
if (!is_valid_plugin_name(user_input)) {
    return error;
}
// ... then load
```

### Symbol Injection

Malicious libraries can define functions that override standard ones:

```c
// Evil plugin
void free(void *ptr) {
    // Don't actually free, cause memory exhaustion
    // Or: log the pointer for later exploitation
}
```

Mitigations:
- Load plugins with `RTLD_LOCAL`
- Use namespaced symbol names
- Validate plugin signatures

### Privilege Escalation

If your program runs with elevated privileges, don't load user-specified libraries:

```c
if (geteuid() != getuid()) {
    // Running setuid - don't trust LD_LIBRARY_PATH or user paths
    secure_plugin_path = "/usr/lib/myapp/plugins";
}
```

## Performance Characteristics

| Operation | Cost |
|-----------|------|
| `dlopen` (first time) | ~milliseconds (file I/O, relocation) |
| `dlopen` (already loaded) | ~microseconds (increment refcount) |
| `dlsym` | ~microseconds (hash lookup) |
| `dlclose` | ~microseconds (decrement refcount) |
| Actual unload | ~milliseconds (if refcount hits zero) |

Tips:
- Cache dlsym results (function pointers)
- Use `RTLD_NOW` in production (fail fast)
- Use `RTLD_LAZY` in development (faster startup)

## Debugging Runtime Loading

```bash
# See what's being loaded
LD_DEBUG=files ./program

# See symbol lookups
LD_DEBUG=symbols ./program

# Trace library calls
ltrace ./program

# In GDB
(gdb) set stop-on-solib-events 1
(gdb) run
# Breaks when shared libraries load
```

## Key Takeaways

1. **dlopen/dlsym/dlclose** provide runtime library loading
2. **Plugin architectures** use defined interfaces and dlsym
3. **RTLD_NEXT** enables function wrapping
4. **Hot reloading** is possible with careful lifecycle management
5. **Security matters**—validate paths and understand symbol scope
6. **WASM uses JavaScript** for runtime module loading
7. **Cache compiled modules** and symbol lookups for performance

Next, we'll compare ELF and WASM side by side, highlighting their different design choices and trade-offs.
