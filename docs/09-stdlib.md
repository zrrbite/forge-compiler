# Standard Library

Forge's standard library provides built-in functions, methods, and types
that are always available without imports.

## Built-in Functions

### I/O

| Function | Description |
|----------|-------------|
| `print(value)` | Print a value + newline to stdout |
| `println(value)` | Alias for print |
| `eprint(value)` | Print to stderr |
| `input()` | Read a line from stdin |
| `input(prompt)` | Print prompt to stderr, then read a line |
| `stdin_lines()` | Read all lines from stdin as `[str]` |

### Type Conversion

| Function | Description |
|----------|-------------|
| `to_str(value)` | Convert any value to string |
| `to_int(s)` | Parse string as integer |
| `to_float(s)` | Parse string as float |

### Math

| Function | Description |
|----------|-------------|
| `abs(x)` | Absolute value |
| `min(a, b)` | Minimum of two values |
| `max(a, b)` | Maximum of two values |

### Assertions

| Function | Description |
|----------|-------------|
| `assert(cond)` | Panic if false |
| `assert_eq(a, b)` | Panic if a != b |

### Process / Environment

| Function | Description |
|----------|-------------|
| `args()` | Command-line arguments as `[str]` |
| `exit(code)` | Exit with status code |
| `exec(cmd, args)` | Run shell command, returns `{success, stdout, stderr, code}` |
| `env_get(key)` | Get environment variable (empty string if not set) |
| `env_set(key, value)` | Set environment variable |
| `env_vars()` | All env vars as `[[key, value], ...]` |

### Constructors

| Function | Description |
|----------|-------------|
| `Ok(value)` | Wrap value in Ok variant |
| `Err(value)` | Wrap value in Err variant |
| `Some(value)` | Wrap value in Some variant |
| `HashMap()` | Create empty hash map |

## Constants

| Name | Value | Description |
|------|-------|-------------|
| `PI` | 3.14159... | Circle constant |
| `E` | 2.71828... | Euler's number |
| `None` | — | Empty Option value |

## Built-in Methods

### Arrays

| Method | Description |
|--------|-------------|
| `len()` | Number of elements |
| `push(val)` | Append element (mutates) |
| `pop()` | Remove last element, returns `Some(val)` or `None` |
| `last()` | Last element as `Some(val)` or `None` |
| `get(i)` | Get element by index |
| `set(i, val)` | Set element at index |
| `insert(i, val)` | Insert at index |
| `remove(i)` | Remove at index |
| `contains(val)` | Check membership |
| `sort()` | Sort (numeric-aware) |
| `reverse()` | Reverse order |
| `dedup()` | Remove consecutive duplicates |
| `flatten()` | Flatten one level of nesting |
| `min()` | Minimum element |
| `max()` | Maximum element |
| `sum()` | Sum all elements |
| `enumerate()` | Returns `[[index, value], ...]` |
| `join(sep)` | Join elements with separator string |
| `map(f)` | Transform each element |
| `filter(f)` | Keep elements matching predicate |
| `fold(init, f)` | Reduce to single value |
| `each(f)` | Execute side effect per element |
| `is_empty()` | Check if empty |

### Strings

| Method | Description |
|--------|-------------|
| `len()` | Character count (UTF-8 aware) |
| `char_at(i)` | Character at index (returns single-char string) |
| `substring(start, end)` | Substring by char indices |
| `chars()` | Split into array of single characters |
| `lines()` | Split on newlines |
| `split(delim)` | Split on delimiter |
| `trim()` | Remove leading/trailing whitespace |
| `contains(sub)` | Check if contains substring |
| `starts_with(prefix)` | Check prefix |
| `ends_with(suffix)` | Check suffix |
| `find(sub)` | Find index of substring |
| `replace(from, to)` | Replace all occurrences |
| `to_upper()` | Uppercase |
| `to_lower()` | Lowercase |
| `repeat(n)` | Repeat n times |
| `parse_int()` | Parse as integer, returns `Ok(n)` or `Err(msg)` |
| `parse_float()` | Parse as float, returns `Ok(f)` or `Err(msg)` |
| `is_empty()` | Check if empty |
| `is_digit()` | Check if single digit character |
| `is_alpha()` | Check if single alphabetic character |
| `is_whitespace()` | Check if single whitespace character |

### Floats

| Method | Description |
|--------|-------------|
| `sqrt()` | Square root |
| `abs()` | Absolute value |
| `floor()` | Round down |
| `ceil()` | Round up |
| `round()` | Round to nearest |

### HashMap

| Method | Description |
|--------|-------------|
| `insert(key, value)` | Insert or update entry |
| `get(key)` | Get value as `Some(val)` or `None` |
| `get_or(key, default)` | Get value or return default |
| `contains_key(key)` | Check if key exists |
| `remove(key)` | Remove entry |
| `keys()` | All keys as array |
| `values()` | All values as array |
| `entries()` | All entries as `[[key, value], ...]` |
| `len()` | Number of entries |
| `is_empty()` | Check if empty |

HashMap supports iteration with `for pair in map { ... }` where each `pair` is `[key, value]`.

### Result / Option

| Method | Description |
|--------|-------------|
| `unwrap()` | Extract value or panic |
| `is_ok()` | Check if Ok |
| `is_err()` | Check if Err |

### File

| Method | Description |
|--------|-------------|
| `File.read(path)` | Read file contents, returns `Ok(str)` or `Err(msg)` |
| `File.read_lines(path)` | Read as array of lines, returns `Ok([str])` or `Err(msg)` |
| `File.write(path, content)` | Write string to file |
| `File.exists(path)` | Check if file exists |

## Slice Syntax

Arrays and strings support Go/Python-style slicing:

```
let nums = [1, 2, 3, 4, 5]
nums[1:3]    // [2, 3]
nums[:2]     // [1, 2]
nums[3:]     // [4, 5]

let s = "hello world"
s[0:5]       // "hello"
s[6:]        // "world"
```

## Example

```
fn main() {
    // I/O
    let name = input("Name: ")
    print("Hello, {name}!")

    // Arrays
    let nums = [5, 3, 1, 4, 2]
    print(nums.sort())           // [1, 2, 3, 4, 5]
    print(nums.map(|x| x * 2))  // [10, 6, 2, 8, 4]
    print(nums.sum())            // 15

    // HashMap
    let mut m = HashMap()
    m.insert("a", 1)
    m.insert("b", 2)
    print(m.get("a") ?? 0)      // 1

    // File I/O
    let lines = File.read_lines("input.txt").unwrap()
    for line in lines { print(line) }

    // Environment
    print(env_get("HOME"))

    // Defer
    defer print("cleanup!")
    print("working...")
}
```
