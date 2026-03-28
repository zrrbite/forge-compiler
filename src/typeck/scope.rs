//! Scope and symbol table for name resolution.

use std::collections::HashMap;

use super::types::Ty;

/// Information about a variable binding.
#[derive(Debug, Clone)]
pub struct VarInfo {
    pub ty: Ty,
    pub mutable: bool,
}

/// Information about a function.
#[derive(Debug, Clone)]
pub struct FnInfo {
    pub params: Vec<(String, Ty)>,
    pub ret: Ty,
}

/// Information about a struct type.
#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<(String, Ty)>,
}

/// Information about a method on a type.
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    /// Whether the first parameter is `self` (instance method) or not (static).
    pub is_instance: bool,
    pub params: Vec<(String, Ty)>,
    pub ret: Ty,
}

/// Information about an enum type.
#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<(String, Vec<Ty>)>,
}

/// A scope in the symbol table.
#[derive(Debug, Clone)]
struct Scope {
    variables: HashMap<String, VarInfo>,
}

impl Scope {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }
}

/// The symbol table: a stack of scopes plus type-level information.
#[derive(Debug, Clone)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
    /// Global functions.
    pub functions: HashMap<String, FnInfo>,
    /// Struct definitions.
    pub structs: HashMap<String, StructInfo>,
    /// Enum definitions.
    pub enums: HashMap<String, EnumInfo>,
    /// Methods: type_name → method_name → MethodInfo.
    pub methods: HashMap<String, HashMap<String, MethodInfo>>,
    /// Trait impls: (trait_name, type_name) → method_name → MethodInfo.
    pub trait_impls: HashMap<(String, String), HashMap<String, MethodInfo>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            methods: HashMap::new(),
            trait_impls: HashMap::new(),
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Define a variable in the current scope.
    pub fn define_var(&mut self, name: String, info: VarInfo) {
        self.scopes.last_mut().unwrap().variables.insert(name, info);
    }

    /// Look up a variable by name, walking the scope chain.
    pub fn lookup_var(&self, name: &str) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.variables.get(name) {
                return Some(info);
            }
        }
        None
    }

    /// Look up a function by name.
    pub fn lookup_fn(&self, name: &str) -> Option<&FnInfo> {
        self.functions.get(name)
    }

    /// Look up a struct by name.
    pub fn lookup_struct(&self, name: &str) -> Option<&StructInfo> {
        self.structs.get(name)
    }

    /// Look up an enum by name.
    pub fn lookup_enum(&self, name: &str) -> Option<&EnumInfo> {
        self.enums.get(name)
    }

    /// Look up a method on a type.
    pub fn lookup_method(&self, type_name: &str, method_name: &str) -> Option<&MethodInfo> {
        self.methods
            .get(type_name)
            .and_then(|methods| methods.get(method_name))
    }

    /// Register a function.
    pub fn register_fn(&mut self, name: String, info: FnInfo) {
        self.functions.insert(name, info);
    }

    /// Register a struct type.
    pub fn register_struct(&mut self, info: StructInfo) {
        self.structs.insert(info.name.clone(), info);
    }

    /// Register an enum type.
    pub fn register_enum(&mut self, info: EnumInfo) {
        self.enums.insert(info.name.clone(), info);
    }

    /// Register a method on a type.
    pub fn register_method(&mut self, type_name: String, info: MethodInfo) {
        self.methods
            .entry(type_name)
            .or_default()
            .insert(info.name.clone(), info);
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}
