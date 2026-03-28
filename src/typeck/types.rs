//! Internal type representation for the Forge type system.

use std::collections::HashMap;
use std::fmt;

/// A unique identifier for type variables (used during inference).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarId(pub u32);

/// The internal representation of a Forge type.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    // -- Primitive types --
    Int(IntWidth),
    UInt(IntWidth),
    Float(FloatWidth),
    Bool,
    Str,
    Unit,

    // -- Compound types --
    Array(Box<Ty>),
    /// A reference: `&T` or `&mut T`
    Ref {
        mutable: bool,
        inner: Box<Ty>,
    },
    /// A struct type, identified by name with its field types.
    Struct {
        name: String,
        fields: Vec<(String, Ty)>,
    },
    /// An enum type.
    Enum {
        name: String,
        variants: Vec<(String, Vec<Ty>)>,
    },
    /// A function type: `fn(A, B) -> C`
    Function {
        params: Vec<Ty>,
        ret: Box<Ty>,
    },
    /// A named type that hasn't been resolved yet.
    Named(String),

    // -- Inference --
    /// A type variable (to be unified during inference).
    Var(TypeVarId),
    /// A type that couldn't be determined (error recovery).
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntWidth {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatWidth {
    F32,
    F64,
}

impl Ty {
    /// The default integer type when no annotation is given.
    pub fn default_int() -> Ty {
        Ty::Int(IntWidth::I64)
    }

    /// The default float type when no annotation is given.
    pub fn default_float() -> Ty {
        Ty::Float(FloatWidth::F64)
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int(_) | Ty::UInt(_) | Ty::Float(_))
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Ty::Int(_) | Ty::UInt(_))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Ty::Float(_))
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }

    /// Resolve a type name string to a Ty.
    pub fn from_name(name: &str) -> Ty {
        match name {
            "i8" => Ty::Int(IntWidth::I8),
            "i16" => Ty::Int(IntWidth::I16),
            "i32" => Ty::Int(IntWidth::I32),
            "i64" => Ty::Int(IntWidth::I64),
            "i128" => Ty::Int(IntWidth::I128),
            "isize" => Ty::Int(IntWidth::ISize),
            "u8" => Ty::UInt(IntWidth::I8),
            "u16" => Ty::UInt(IntWidth::I16),
            "u32" => Ty::UInt(IntWidth::I32),
            "u64" => Ty::UInt(IntWidth::I64),
            "u128" => Ty::UInt(IntWidth::I128),
            "usize" => Ty::UInt(IntWidth::ISize),
            "f32" => Ty::Float(FloatWidth::F32),
            "f64" => Ty::Float(FloatWidth::F64),
            "bool" => Ty::Bool,
            "str" => Ty::Str,
            _ => Ty::Named(name.to_string()),
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int(w) => write!(
                f,
                "{}",
                match w {
                    IntWidth::I8 => "i8",
                    IntWidth::I16 => "i16",
                    IntWidth::I32 => "i32",
                    IntWidth::I64 => "i64",
                    IntWidth::I128 => "i128",
                    IntWidth::ISize => "isize",
                }
            ),
            Ty::UInt(w) => write!(
                f,
                "u{}",
                match w {
                    IntWidth::I8 => "8",
                    IntWidth::I16 => "16",
                    IntWidth::I32 => "32",
                    IntWidth::I64 => "64",
                    IntWidth::I128 => "128",
                    IntWidth::ISize => "size",
                }
            ),
            Ty::Float(w) => write!(
                f,
                "{}",
                match w {
                    FloatWidth::F32 => "f32",
                    FloatWidth::F64 => "f64",
                }
            ),
            Ty::Bool => write!(f, "bool"),
            Ty::Str => write!(f, "str"),
            Ty::Unit => write!(f, "()"),
            Ty::Array(inner) => write!(f, "[{inner}]"),
            Ty::Ref { mutable, inner } => {
                if *mutable {
                    write!(f, "&mut {inner}")
                } else {
                    write!(f, "&{inner}")
                }
            }
            Ty::Struct { name, .. } => write!(f, "{name}"),
            Ty::Enum { name, .. } => write!(f, "{name}"),
            Ty::Function { params, ret } => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")
            }
            Ty::Named(name) => write!(f, "{name}"),
            Ty::Var(id) => write!(f, "?T{}", id.0),
            Ty::Error => write!(f, "<error>"),
        }
    }
}

/// Type unification table. Maps type variables to their resolved types.
#[derive(Debug, Clone)]
pub struct UnificationTable {
    /// TypeVarId → resolved type (or None if still free).
    bindings: HashMap<TypeVarId, Ty>,
    next_var: u32,
}

impl UnificationTable {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            next_var: 0,
        }
    }

    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> Ty {
        let id = TypeVarId(self.next_var);
        self.next_var += 1;
        Ty::Var(id)
    }

    /// Resolve a type, following type variable bindings.
    pub fn resolve(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Var(id) => match self.bindings.get(id) {
                Some(bound) => self.resolve(bound),
                None => ty.clone(),
            },
            Ty::Array(inner) => Ty::Array(Box::new(self.resolve(inner))),
            Ty::Ref { mutable, inner } => Ty::Ref {
                mutable: *mutable,
                inner: Box::new(self.resolve(inner)),
            },
            Ty::Function { params, ret } => Ty::Function {
                params: params.iter().map(|p| self.resolve(p)).collect(),
                ret: Box::new(self.resolve(ret)),
            },
            _ => ty.clone(),
        }
    }

    /// Unify two types. Returns Ok(()) if they're compatible, Err with a
    /// message if they conflict.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), String> {
        let a = self.resolve(a);
        let b = self.resolve(b);

        match (&a, &b) {
            // Same type → OK.
            _ if a == b => Ok(()),

            // Error types unify with anything (error recovery).
            (Ty::Error, _) | (_, Ty::Error) => Ok(()),

            // Type variable → bind it.
            (Ty::Var(id), _) => {
                self.bindings.insert(*id, b);
                Ok(())
            }
            (_, Ty::Var(id)) => {
                self.bindings.insert(*id, a);
                Ok(())
            }

            // Named types: resolve to the same name.
            (Ty::Named(a_name), Ty::Named(b_name)) if a_name == b_name => Ok(()),

            // Named type vs concrete: the named type might be an alias.
            // For now, named types match if the name is the same.
            (Ty::Struct { name: a_name, .. }, Ty::Named(b_name))
            | (Ty::Named(b_name), Ty::Struct { name: a_name, .. })
                if a_name == b_name =>
            {
                Ok(())
            }

            // Numeric promotion: int literal can be any integer type.
            (Ty::Int(_), Ty::Int(_)) => Ok(()),
            (Ty::UInt(_), Ty::UInt(_)) => Ok(()),
            (Ty::Float(_), Ty::Float(_)) => Ok(()),

            // Array types.
            (Ty::Array(a_inner), Ty::Array(b_inner)) => self.unify(a_inner, b_inner),

            // Reference types.
            (
                Ty::Ref {
                    mutable: a_mut,
                    inner: a_inner,
                },
                Ty::Ref {
                    mutable: b_mut,
                    inner: b_inner,
                },
            ) => {
                if a_mut != b_mut {
                    return Err(format!("Cannot unify &{a} with &{b}: mutability mismatch"));
                }
                self.unify(a_inner, b_inner)
            }

            // Function types.
            (
                Ty::Function {
                    params: a_params,
                    ret: a_ret,
                },
                Ty::Function {
                    params: b_params,
                    ret: b_ret,
                },
            ) => {
                if a_params.len() != b_params.len() {
                    return Err(format!(
                        "Function parameter count mismatch: {} vs {}",
                        a_params.len(),
                        b_params.len()
                    ));
                }
                for (ap, bp) in a_params.iter().zip(b_params) {
                    self.unify(ap, bp)?;
                }
                self.unify(a_ret, b_ret)
            }

            _ => Err(format!("Type mismatch: expected {a}, found {b}")),
        }
    }
}

impl Default for UnificationTable {
    fn default() -> Self {
        Self::new()
    }
}
