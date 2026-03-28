//! Forge standard library — built-in functions and constants.
//!
//! These are registered as builtins in the interpreter and as known
//! function signatures in the type checker. They don't require `use`
//! imports — they're always available.

#[cfg(test)]
mod tests;

/// Description of a built-in function.
pub struct BuiltinFn {
    pub name: &'static str,
    pub params: &'static [(&'static str, &'static str)],
    pub ret: &'static str,
    pub description: &'static str,
}

/// All built-in functions in the Forge standard library.
pub const BUILTINS: &[BuiltinFn] = &[
    // ── I/O ─────────────────────────────────────────────────────────────
    BuiltinFn {
        name: "print",
        params: &[("value", "any")],
        ret: "()",
        description: "Print a value followed by a newline",
    },
    BuiltinFn {
        name: "println",
        params: &[("value", "any")],
        ret: "()",
        description: "Print a value followed by a newline (alias for print)",
    },
    BuiltinFn {
        name: "eprint",
        params: &[("value", "any")],
        ret: "()",
        description: "Print to stderr",
    },
    // ── Type conversion ─────────────────────────────────────────────────
    BuiltinFn {
        name: "to_str",
        params: &[("value", "any")],
        ret: "str",
        description: "Convert any value to its string representation",
    },
    BuiltinFn {
        name: "to_int",
        params: &[("value", "str")],
        ret: "i64",
        description: "Parse a string as an integer",
    },
    BuiltinFn {
        name: "to_float",
        params: &[("value", "str")],
        ret: "f64",
        description: "Parse a string as a float",
    },
    // ── Math ────────────────────────────────────────────────────────────
    BuiltinFn {
        name: "abs",
        params: &[("x", "f64")],
        ret: "f64",
        description: "Absolute value",
    },
    BuiltinFn {
        name: "min",
        params: &[("a", "i64"), ("b", "i64")],
        ret: "i64",
        description: "Minimum of two values",
    },
    BuiltinFn {
        name: "max",
        params: &[("a", "i64"), ("b", "i64")],
        ret: "i64",
        description: "Maximum of two values",
    },
    // ── Assertions ──────────────────────────────────────────────────────
    BuiltinFn {
        name: "assert",
        params: &[("condition", "bool")],
        ret: "()",
        description: "Panic if condition is false",
    },
    BuiltinFn {
        name: "assert_eq",
        params: &[("a", "any"), ("b", "any")],
        ret: "()",
        description: "Panic if a != b",
    },
];

/// Built-in math constants.
pub const MATH_PI: f64 = std::f64::consts::PI;
pub const MATH_E: f64 = std::f64::consts::E;
