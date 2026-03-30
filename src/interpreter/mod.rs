use std::collections::HashMap;
use std::fmt;

use crate::ast::*;

#[cfg(test)]
mod tests;

// ── Values ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Value {
    Int(i128),
    Float(f64),
    Bool(bool),
    String(String),
    Array(Vec<Value>),
    Struct {
        name: String,
        fields: HashMap<String, Value>,
    },
    Variant {
        name: String,
        fields: Vec<Value>,
    },
    /// A hash map (dictionary).
    Map(Vec<(Value, Value)>),
    /// A mutable reference to a variable in an outer scope.
    MutRef(String),
    Function(FnValue),
    Unit,
}

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::String(_) => "str",
            Value::Array(_) => "array",
            Value::Map(_) => "map",
            Value::MutRef(_) => "ref",
            Value::Struct { name, .. } => name,
            Value::Variant { name, .. } => name,
            Value::Function(_) => "fn",
            Value::Unit => "()",
        }
    }

    fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Unit => false,
            _ => true,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::String(s) => write!(f, "{s}"),
            Value::Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::Struct { name, fields } => {
                write!(f, "{name} {{ ")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, " }}")
            }
            Value::Variant { name, fields } => {
                if fields.is_empty() {
                    write!(f, "{name}")
                } else {
                    write!(f, "{name}(")?;
                    for (i, v) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{v}")?;
                    }
                    write!(f, ")")
                }
            }
            Value::Map(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            Value::MutRef(name) => write!(f, "&mut {name}"),
            Value::Function(_) => write!(f, "<fn>"),
            Value::Unit => write!(f, "()"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum FnValue {
    UserDefined {
        name: String,
        params: Vec<Param>,
        body: Block,
    },
    Closure {
        params: Vec<ClosureParam>,
        body: Expr,
        env: Env,
    },
    Builtin(String),
}

// ── Environment ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Env {
    scopes: Vec<HashMap<String, Value>>,
}

impl Env {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: String, value: Value) {
        self.scopes.last_mut().unwrap().insert(name, value);
    }

    fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return Ok(());
            }
        }
        Err(format!("Undefined variable: {name}"))
    }

    /// Get a value, transparently following MutRef chains.
    fn get_deref(&self, name: &str) -> Option<Value> {
        match self.get(name) {
            Some(Value::MutRef(ref_name)) => {
                // Follow the ref, skipping other MutRefs with the same name.
                for scope in self.scopes.iter().rev() {
                    if let Some(val) = scope.get(ref_name)
                        && !matches!(val, Value::MutRef(_))
                    {
                        return Some(val.clone());
                    }
                }
                None
            }
            Some(v) => Some(v.clone()),
            None => None,
        }
    }

    /// Set a value, transparently following MutRef chains.
    fn set_deref(&mut self, name: &str, value: Value) -> Result<(), String> {
        let target = match self.get(name) {
            Some(Value::MutRef(ref_name)) => ref_name.clone(),
            _ => name.to_string(),
        };
        // Find the scope with the actual (non-MutRef) value.
        for scope in self.scopes.iter_mut().rev() {
            if let Some(val) = scope.get(&target)
                && !matches!(val, Value::MutRef(_))
            {
                scope.insert(target, value);
                return Ok(());
            }
        }
        // Fallback.
        self.set(&target, value)
    }
}

// ── Control flow ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct RuntimeError(pub String);

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Runtime error: {}", self.0)
    }
}

/// Outcome of evaluating an expression or block.
/// Control flow signals are values, not errors — this lets us propagate
/// them cleanly through the call stack.
enum Outcome {
    Val(Value),
    Return(Value),
    Break,
    Continue,
    Error(String),
}

// ── Interpreter ─────────────────────────────────────────────────────────

pub struct Interpreter {
    env: Env,
    methods: HashMap<String, HashMap<String, Function>>,
    trait_impls: HashMap<(String, String), HashMap<String, Function>>,
    output: Option<Vec<String>>,
    /// Temporary storage for modified `self` from mut self methods.
    last_modified_self: Option<Value>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let mut interp = Self {
            env: Env::new(),
            methods: HashMap::new(),
            trait_impls: HashMap::new(),
            output: None,
            last_modified_self: None,
        };
        interp.register_builtins();
        interp
    }

    pub fn new_capturing() -> Self {
        let mut interp = Self {
            env: Env::new(),
            methods: HashMap::new(),
            trait_impls: HashMap::new(),
            output: Some(Vec::new()),
            last_modified_self: None,
        };
        interp.register_builtins();
        interp
    }

    fn register_builtins(&mut self) {
        let builtins = [
            "print",
            "println",
            "eprint",
            "to_str",
            "to_int",
            "to_float",
            "abs",
            "min",
            "max",
            "assert",
            "assert_eq",
            // Result/Option constructors
            "Ok",
            "Err",
            "Some",
        ];
        for name in builtins {
            self.env
                .define(name.into(), Value::Function(FnValue::Builtin(name.into())));
        }
        // None is a value, not a function.
        self.env.define(
            "None".into(),
            Value::Variant {
                name: "None".into(),
                fields: vec![],
            },
        );
        // HashMap constructor.
        self.env.define(
            "HashMap".into(),
            Value::Function(FnValue::Builtin("HashMap".into())),
        );
        // File namespace (for File.read, File.write, File.exists).
        self.env.define(
            "File".into(),
            Value::Variant {
                name: "File".into(),
                fields: vec![],
            },
        );
        // Process/IO builtins.
        for name in [
            "args",
            "exit",
            "exec",
            "input",
            "stdin_lines",
            "env_get",
            "env_set",
            "env_vars",
        ] {
            self.env
                .define(name.into(), Value::Function(FnValue::Builtin(name.into())));
        }
        // Math constants.
        self.env
            .define("PI".into(), Value::Float(std::f64::consts::PI));
        self.env
            .define("E".into(), Value::Float(std::f64::consts::E));
    }

    pub fn get_output(&self) -> &[String] {
        self.output.as_deref().unwrap_or(&[])
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        for item in &program.items {
            self.register_item(item);
        }

        let main_fn = self.env.get("main").cloned();
        match main_fn {
            Some(Value::Function(FnValue::UserDefined { params, body, .. })) => {
                if !params.is_empty() {
                    return Err(RuntimeError("main() must take no parameters".into()));
                }
                match self.eval_block(&body) {
                    Outcome::Val(_) | Outcome::Return(_) => Ok(()),
                    Outcome::Break => Err(RuntimeError("break outside loop".into())),
                    Outcome::Continue => Err(RuntimeError("continue outside loop".into())),
                    Outcome::Error(msg) => Err(RuntimeError(msg)),
                }
            }
            Some(_) => Err(RuntimeError("'main' is not a function".into())),
            None => Err(RuntimeError("No main() function found".into())),
        }
    }

    /// Evaluate a program fragment in the REPL. Registers items and executes
    /// statements in the current scope. Returns the last expression value
    /// (if any) for auto-printing.
    pub fn eval_repl(&mut self, program: &Program) -> Result<Option<Value>, RuntimeError> {
        let mut last_value = None;

        for item in &program.items {
            match &item.kind {
                ItemKind::Function(func) => {
                    if func.name == "main" {
                        // Wrapped input — run body in current scope (no push/pop)
                        match self.eval_block_inner(&func.body) {
                            Outcome::Val(v) => last_value = Some(v),
                            Outcome::Return(v) => last_value = Some(v),
                            Outcome::Error(msg) => return Err(RuntimeError(msg)),
                            _ => {}
                        }
                    } else {
                        self.register_item(item);
                    }
                }
                _ => self.register_item(item),
            }
        }

        Ok(last_value)
    }

    fn register_item(&mut self, item: &Item) {
        match &item.kind {
            ItemKind::Function(func) => {
                let value = Value::Function(FnValue::UserDefined {
                    name: func.name.clone(),
                    params: func.params.clone(),
                    body: func.body.clone(),
                });
                self.env.define(func.name.clone(), value);
            }
            ItemKind::Impl(imp) => {
                let type_methods = self.methods.entry(imp.target.clone()).or_default();
                for method in &imp.methods {
                    type_methods.insert(method.name.clone(), method.clone());
                }
                if let Some(trait_name) = &imp.trait_name {
                    let key = (trait_name.clone(), imp.target.clone());
                    let impl_methods = self.trait_impls.entry(key).or_default();
                    for method in &imp.methods {
                        impl_methods.insert(method.name.clone(), method.clone());
                    }
                }
            }
            ItemKind::Struct(_) | ItemKind::Enum(_) | ItemKind::Trait(_) | ItemKind::Use(_) => {}
        }
    }

    // ── Block / statement evaluation ────────────────────────────────────

    fn eval_block(&mut self, block: &Block) -> Outcome {
        self.env.push_scope();
        let result = self.eval_block_inner(block);
        self.env.pop_scope();
        result
    }

    fn eval_block_inner(&mut self, block: &Block) -> Outcome {
        let mut last = Value::Unit;
        let mut deferred: Vec<&Expr> = Vec::new();

        let result = 'block: {
            for (i, stmt) in block.stmts.iter().enumerate() {
                let is_last = i == block.stmts.len() - 1;
                match &stmt.kind {
                    StmtKind::Let { name, value, .. } => {
                        let val = match value {
                            Some(expr) => match self.eval_expr(expr) {
                                Outcome::Val(v) => v,
                                other => break 'block other,
                            },
                            None => Value::Unit,
                        };
                        self.env.define(name.clone(), val);
                        last = Value::Unit;
                    }
                    StmtKind::Return(expr) => {
                        let val = match expr {
                            Some(e) => match self.eval_expr(e) {
                                Outcome::Val(v) => v,
                                other => break 'block other,
                            },
                            None => Value::Unit,
                        };
                        break 'block Outcome::Return(val);
                    }
                    StmtKind::Break => break 'block Outcome::Break,
                    StmtKind::Continue => break 'block Outcome::Continue,
                    StmtKind::Defer(expr) => {
                        deferred.push(expr);
                    }
                    StmtKind::Expr(expr) => match self.eval_expr(expr) {
                        Outcome::Val(v) => {
                            if is_last {
                                last = v;
                            }
                        }
                        other => break 'block other,
                    },
                }
            }
            Outcome::Val(last)
        };

        // Execute deferred expressions in LIFO order.
        for expr in deferred.iter().rev() {
            let _ = self.eval_expr(expr);
        }

        result
    }

    // ── Expression evaluation ───────────────────────────────────────────

    fn eval_expr(&mut self, expr: &Expr) -> Outcome {
        match &expr.kind {
            ExprKind::IntLiteral(n) => Outcome::Val(Value::Int(*n)),
            ExprKind::FloatLiteral(f) => Outcome::Val(Value::Float(*f)),
            ExprKind::BoolLiteral(b) => Outcome::Val(Value::Bool(*b)),
            ExprKind::StringLiteral(s) => Outcome::Val(Value::String(s.clone())),

            ExprKind::InterpolatedString(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        StringPart::Literal(s) => result.push_str(s),
                        StringPart::Expr(e) => {
                            let val = try_val!(self.eval_expr(e));
                            result.push_str(&val.to_string());
                        }
                    }
                }
                Outcome::Val(Value::String(result))
            }

            ExprKind::Identifier(name) => match self.env.get_deref(name) {
                Some(v) => Outcome::Val(v),
                None => {
                    // Could be a type name used in static method calls (Vec2.new()).
                    // Check if we have methods registered for this name.
                    if self.methods.contains_key(name.as_str()) {
                        Outcome::Val(Value::Variant {
                            name: name.clone(),
                            fields: vec![],
                        })
                    } else {
                        Outcome::Error(format!("Undefined variable: {name}"))
                    }
                }
            },

            ExprKind::SelfValue => match self.env.get("self").cloned() {
                Some(v) => Outcome::Val(v),
                None => Outcome::Val(Value::Unit),
            },

            ExprKind::BinaryOp { left, op, right } => {
                let lhs = try_val!(self.eval_expr(left));
                let rhs = try_val!(self.eval_expr(right));

                // For struct types, try operator overloading first.
                if let Value::Struct { name, .. } = &lhs {
                    let method = op_method_name(*op);
                    if let Some(type_methods) = self.methods.get(name.as_str()).cloned()
                        && let Some(method_fn) = type_methods.get(method).cloned()
                    {
                        return self.call_function(&method_fn, vec![lhs, rhs]);
                    }
                }

                // Primitive types: use built-in operators.
                match eval_binop(&lhs, *op, &rhs) {
                    Ok(v) => Outcome::Val(v),
                    Err(msg) => Outcome::Error(msg),
                }
            }

            ExprKind::UnaryOp { op, expr } => {
                let val = try_val!(self.eval_expr(expr));
                match op {
                    UnaryOp::Neg => match val {
                        Value::Int(n) => Outcome::Val(Value::Int(-n)),
                        Value::Float(f) => Outcome::Val(Value::Float(-f)),
                        _ => Outcome::Val(Value::Unit),
                    },
                    UnaryOp::Not => match val {
                        Value::Bool(b) => Outcome::Val(Value::Bool(!b)),
                        _ => Outcome::Val(Value::Unit),
                    },
                    UnaryOp::Ref | UnaryOp::Deref => Outcome::Val(val),
                }
            }

            ExprKind::Call { callee, args } => self.eval_call(callee, args),

            ExprKind::FieldAccess { object, field } => {
                let obj = try_val!(self.eval_expr(object));
                match &obj {
                    Value::Struct { fields, .. } => {
                        Outcome::Val(fields.get(field).cloned().unwrap_or(Value::Unit))
                    }
                    _ => Outcome::Val(Value::Unit),
                }
            }

            ExprKind::Index { object, index } => {
                let obj = try_val!(self.eval_expr(object));
                let idx = try_val!(self.eval_expr(index));
                match (&obj, &idx) {
                    (Value::Array(items), Value::Int(i)) => {
                        Outcome::Val(items.get(*i as usize).cloned().unwrap_or(Value::Unit))
                    }
                    _ => Outcome::Val(Value::Unit),
                }
            }

            ExprKind::Slice { object, start, end } => {
                let obj = try_val!(self.eval_expr(object));
                let s = if let Some(start_expr) = start {
                    let v = try_val!(self.eval_expr(start_expr));
                    if let Value::Int(i) = v { i as usize } else { 0 }
                } else {
                    0
                };
                match &obj {
                    Value::Array(items) => {
                        let e = if let Some(end_expr) = end {
                            let v = try_val!(self.eval_expr(end_expr));
                            if let Value::Int(i) = v {
                                i as usize
                            } else {
                                items.len()
                            }
                        } else {
                            items.len()
                        };
                        let e = e.min(items.len());
                        let s = s.min(e);
                        Outcome::Val(Value::Array(items[s..e].to_vec()))
                    }
                    Value::String(string) => {
                        let chars: Vec<char> = string.chars().collect();
                        let e = if let Some(end_expr) = end {
                            let v = try_val!(self.eval_expr(end_expr));
                            if let Value::Int(i) = v {
                                i as usize
                            } else {
                                chars.len()
                            }
                        } else {
                            chars.len()
                        };
                        let e = e.min(chars.len());
                        let s = s.min(e);
                        Outcome::Val(Value::String(chars[s..e].iter().collect()))
                    }
                    _ => Outcome::Val(Value::Unit),
                }
            }

            ExprKind::Block(block) => self.eval_block(block),

            ExprKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let cond = try_val!(self.eval_expr(condition));
                if cond.is_truthy() {
                    self.eval_block(then_block)
                } else if let Some(else_expr) = else_block {
                    self.eval_expr(else_expr)
                } else {
                    Outcome::Val(Value::Unit)
                }
            }

            ExprKind::Match { expr, arms } => {
                let val = try_val!(self.eval_expr(expr));
                for arm in arms {
                    if let Some(bindings) = match_pattern(&arm.pattern, &val) {
                        self.env.push_scope();
                        for (name, v) in bindings {
                            self.env.define(name, v);
                        }
                        let result = self.eval_expr(&arm.body);
                        self.env.pop_scope();
                        return result;
                    }
                }
                Outcome::Val(Value::Unit)
            }

            ExprKind::Closure { params, body } => Outcome::Val(Value::Function(FnValue::Closure {
                params: params.clone(),
                body: *body.clone(),
                env: self.env.clone(),
            })),

            ExprKind::Assign { target, op, value } => {
                let val = try_val!(self.eval_expr(value));
                self.eval_assign(target, *op, val);
                Outcome::Val(Value::Unit)
            }

            ExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                let s = match start {
                    Some(e) => try_val!(self.eval_expr(e)),
                    None => Value::Int(0),
                };
                let e = match end {
                    Some(e) => try_val!(self.eval_expr(e)),
                    None => return Outcome::Val(Value::Unit),
                };
                match (&s, &e) {
                    (Value::Int(start), Value::Int(end)) => {
                        let end = if *inclusive { *end + 1 } else { *end };
                        let items: Vec<Value> = (*start..end).map(Value::Int).collect();
                        Outcome::Val(Value::Array(items))
                    }
                    _ => Outcome::Val(Value::Unit),
                }
            }

            ExprKind::Reference { expr, .. } | ExprKind::Dereference(expr) => self.eval_expr(expr),

            ExprKind::StructLiteral { name, fields } => {
                let type_name = match &name.kind {
                    ExprKind::Identifier(n) => n.clone(),
                    _ => return Outcome::Val(Value::Unit),
                };
                let mut field_values = HashMap::new();
                for fi in fields {
                    let val = match &fi.value {
                        Some(expr) => try_val!(self.eval_expr(expr)),
                        None => self.env.get(&fi.name).cloned().unwrap_or(Value::Unit),
                    };
                    field_values.insert(fi.name.clone(), val);
                }
                Outcome::Val(Value::Struct {
                    name: type_name,
                    fields: field_values,
                })
            }

            ExprKind::Try(inner) => {
                let val = try_val!(self.eval_expr(inner));
                match &val {
                    // Result: Ok(v) unwraps, Err(e) early-returns.
                    Value::Variant { name, fields } if name == "Ok" => {
                        Outcome::Val(fields.first().cloned().unwrap_or(Value::Unit))
                    }
                    Value::Variant { name, .. } if name == "Err" => Outcome::Return(val),
                    // Option: Some(v) unwraps, None early-returns.
                    Value::Variant { name, fields } if name == "Some" => {
                        Outcome::Val(fields.first().cloned().unwrap_or(Value::Unit))
                    }
                    Value::Variant { name, .. } if name == "None" => Outcome::Return(val),
                    _ => Outcome::Val(val),
                }
            }

            ExprKind::SafeNav {
                object,
                field,
                call_args,
            } => {
                let val = try_val!(self.eval_expr(object));
                match &val {
                    // None/Err -> propagate None
                    Value::Variant { name, .. } if name == "None" || name == "Err" => {
                        Outcome::Val(val)
                    }
                    // Some(v) -> unwrap and access field/method
                    Value::Variant { name, fields } if name == "Some" || name == "Ok" => {
                        let inner = fields.first().cloned().unwrap_or(Value::Unit);
                        if let Some(args) = call_args {
                            self.eval_method_call(inner, field, args)
                        } else {
                            // Field access on the inner value
                            match &inner {
                                Value::Struct { fields: sf, .. } => {
                                    Outcome::Val(sf.get(field).cloned().unwrap_or(Value::Unit))
                                }
                                _ => self.eval_method_call(inner, field, &[]),
                            }
                        }
                    }
                    // Not an Option/Result — just do normal field access
                    _ => {
                        if let Some(args) = call_args {
                            self.eval_method_call(val, field, args)
                        } else {
                            match &val {
                                Value::Struct { fields: sf, .. } => {
                                    Outcome::Val(sf.get(field).cloned().unwrap_or(Value::Unit))
                                }
                                _ => self.eval_method_call(val, field, &[]),
                            }
                        }
                    }
                }
            }

            ExprKind::NullCoalesce { expr, default } => {
                let val = try_val!(self.eval_expr(expr));
                match &val {
                    Value::Variant { name, .. } if name == "None" => self.eval_expr(default),
                    Value::Variant { name, .. } if name == "Err" => self.eval_expr(default),
                    Value::Variant { name, fields } if name == "Some" || name == "Ok" => {
                        Outcome::Val(fields.first().cloned().unwrap_or(Value::Unit))
                    }
                    _ => Outcome::Val(val),
                }
            }

            ExprKind::Turbofish { expr, .. } => self.eval_expr(expr),

            ExprKind::Array(elements) => {
                let mut items = Vec::new();
                for e in elements {
                    items.push(try_val!(self.eval_expr(e)));
                }
                Outcome::Val(Value::Array(items))
            }

            ExprKind::For {
                binding,
                iter,
                body,
            } => {
                let iterable = try_val!(self.eval_expr(iter));
                // Convert iterable to a Vec<Value> for uniform handling.
                let items = match iterable {
                    Value::Array(items) => items,
                    Value::Map(entries) => entries
                        .into_iter()
                        .map(|(k, v)| Value::Array(vec![k, v]))
                        .collect(),
                    _ => vec![],
                };
                for item in items {
                    self.env.push_scope();
                    self.env.define(binding.clone(), item);
                    match self.eval_block_inner(body) {
                        Outcome::Val(_) => {}
                        Outcome::Break => {
                            self.env.pop_scope();
                            break;
                        }
                        Outcome::Continue => {}
                        Outcome::Return(v) => {
                            self.env.pop_scope();
                            return Outcome::Return(v);
                        }
                        Outcome::Error(e) => {
                            self.env.pop_scope();
                            return Outcome::Error(e);
                        }
                    }
                    self.env.pop_scope();
                }
                Outcome::Val(Value::Unit)
            }

            ExprKind::While { condition, body } => {
                loop {
                    let cond = try_val!(self.eval_expr(condition));
                    if !cond.is_truthy() {
                        break;
                    }
                    match self.eval_block(body) {
                        Outcome::Val(_) => {}
                        Outcome::Break => break,
                        Outcome::Continue => continue,
                        Outcome::Return(v) => return Outcome::Return(v),
                        Outcome::Error(e) => return Outcome::Error(e),
                    }
                }
                Outcome::Val(Value::Unit)
            }

            ExprKind::Comptime(block) => {
                // In the interpreter, comptime blocks are just evaluated normally.
                self.eval_block(block)
            }
        }
    }

    // ── Calls ───────────────────────────────────────────────────────────

    /// Mutating array methods — the result should be written back to the variable.
    const MUTATING_METHODS: &'static [&'static str] =
        &["push", "pop", "insert", "remove", "clear", "set", "reverse"];

    fn eval_call(&mut self, callee: &Expr, args: &[Expr]) -> Outcome {
        // Method call: `obj.method(args)`
        if let ExprKind::FieldAccess { object, field } = &callee.kind {
            let obj = try_val!(self.eval_expr(object));

            // For mutating array methods, perform mutation in-place.
            if let Value::Array(items) = &obj
                && Self::MUTATING_METHODS.contains(&field.as_str())
                && let ExprKind::Identifier(var_name) = &object.kind
            {
                let mut items = items.clone();
                let return_val = match field.as_str() {
                    "push" => {
                        for arg in args {
                            items.push(try_val!(self.eval_expr(arg)));
                        }
                        Value::Unit
                    }
                    "pop" => match items.pop() {
                        Some(v) => v,
                        None => Value::Unit,
                    },
                    "insert" => {
                        if args.len() == 2 {
                            let idx = try_val!(self.eval_expr(&args[0]));
                            let val = try_val!(self.eval_expr(&args[1]));
                            if let Value::Int(i) = idx {
                                let i = i as usize;
                                if i <= items.len() {
                                    items.insert(i, val);
                                }
                            }
                        }
                        Value::Unit
                    }
                    "remove" => {
                        if args.len() == 1 {
                            let idx = try_val!(self.eval_expr(&args[0]));
                            if let Value::Int(i) = idx {
                                let i = i as usize;
                                if i < items.len() {
                                    return {
                                        let removed = items.remove(i);
                                        let _ = self.env.set_deref(var_name, Value::Array(items));
                                        Outcome::Val(removed)
                                    };
                                }
                            }
                        }
                        Value::Unit
                    }
                    "clear" => {
                        items.clear();
                        Value::Unit
                    }
                    "set" => {
                        if args.len() == 2 {
                            let idx = try_val!(self.eval_expr(&args[0]));
                            let val = try_val!(self.eval_expr(&args[1]));
                            if let Value::Int(i) = idx {
                                let i = i as usize;
                                if i < items.len() {
                                    items[i] = val;
                                }
                            }
                        }
                        Value::Unit
                    }
                    "reverse" => {
                        items.reverse();
                        Value::Unit
                    }
                    _ => Value::Unit,
                };
                let _ = self.env.set_deref(var_name, Value::Array(items));
                return Outcome::Val(return_val);
            }

            // Mutating map methods.
            if let Value::Map(entries) = &obj {
                let mutating_map = ["insert", "remove"];
                if mutating_map.contains(&field.as_str())
                    && let ExprKind::Identifier(var_name) = &object.kind
                {
                    let mut entries = entries.clone();
                    let return_val = match field.as_str() {
                        "insert" => {
                            if args.len() == 2 {
                                let key = try_val!(self.eval_expr(&args[0]));
                                let val = try_val!(self.eval_expr(&args[1]));
                                let key_str = key.to_string();
                                if let Some(entry) =
                                    entries.iter_mut().find(|(k, _)| k.to_string() == key_str)
                                {
                                    entry.1 = val;
                                } else {
                                    entries.push((key, val));
                                }
                            }
                            Value::Unit
                        }
                        "remove" => {
                            if args.len() == 1 {
                                let key = try_val!(self.eval_expr(&args[0]));
                                let key_str = key.to_string();
                                entries.retain(|(k, _)| k.to_string() != key_str);
                            }
                            Value::Unit
                        }
                        _ => Value::Unit,
                    };
                    let _ = self.env.set_deref(var_name, Value::Map(entries));
                    return Outcome::Val(return_val);
                }
            }

            self.last_modified_self = None;
            let result = self.eval_method_call(obj, field, args);

            // Write back modified self for mut self methods.
            if let Some(new_self) = self.last_modified_self.take() {
                let var_name = match &object.kind {
                    ExprKind::Identifier(name) => Some(name.as_str()),
                    ExprKind::SelfValue => Some("self"),
                    _ => None,
                };
                if let Some(name) = var_name {
                    let _ = self.env.set_deref(name, new_self);
                }
            }

            return result;
        }

        let func = try_val!(self.eval_expr(callee));

        // For user-defined functions, check if parameters are &mut and pass MutRefs.
        let mut arg_vals = Vec::new();
        if let Value::Function(FnValue::UserDefined { ref params, .. }) = func {
            for (i, arg) in args.iter().enumerate() {
                let is_mut_ref = params.get(i).is_some_and(|p| {
                    matches!(
                        &p.ty.kind,
                        crate::ast::TypeExprKind::Reference { mutable: true, .. }
                    )
                });
                if is_mut_ref {
                    // Pass a MutRef instead of cloning the value.
                    if let ExprKind::Identifier(var_name) = &arg.kind {
                        arg_vals.push(Value::MutRef(var_name.clone()));
                    } else {
                        arg_vals.push(try_val!(self.eval_expr(arg)));
                    }
                } else {
                    arg_vals.push(try_val!(self.eval_expr(arg)));
                }
            }
        } else {
            for arg in args {
                arg_vals.push(try_val!(self.eval_expr(arg)));
            }
        }
        self.call_value(&func, arg_vals)
    }

    fn eval_method_call(&mut self, obj: Value, method: &str, args: &[Expr]) -> Outcome {
        // Built-in methods on primitives.
        match (&obj, method) {
            (Value::Array(items), "len") => {
                return Outcome::Val(Value::Int(items.len() as i128));
            }
            (Value::Array(items), "push") => {
                let mut items = items.clone();
                for arg in args {
                    items.push(try_val!(self.eval_expr(arg)));
                }
                return Outcome::Val(Value::Array(items));
            }
            (Value::Array(items), "pop") => {
                let mut items = items.clone();
                return match items.pop() {
                    Some(v) => Outcome::Val(Value::Variant {
                        name: "Some".into(),
                        fields: vec![v],
                    }),
                    None => Outcome::Val(Value::Variant {
                        name: "None".into(),
                        fields: vec![],
                    }),
                };
            }
            (Value::Array(items), "insert") => {
                if args.len() != 2 {
                    return Outcome::Error("insert requires 2 arguments".into());
                }
                let idx = try_val!(self.eval_expr(&args[0]));
                let val = try_val!(self.eval_expr(&args[1]));
                let mut items = items.clone();
                if let Value::Int(i) = idx {
                    let i = i as usize;
                    if i <= items.len() {
                        items.insert(i, val);
                    }
                }
                return Outcome::Val(Value::Array(items));
            }
            (Value::Array(items), "remove") => {
                if args.len() != 1 {
                    return Outcome::Error("remove requires 1 argument".into());
                }
                let idx = try_val!(self.eval_expr(&args[0]));
                let mut items = items.clone();
                if let Value::Int(i) = idx {
                    let i = i as usize;
                    if i < items.len() {
                        let removed = items.remove(i);
                        return Outcome::Val(removed);
                    }
                }
                return Outcome::Error("remove: index out of bounds".into());
            }
            (Value::Array(items), "clear") => {
                let _ = items;
                return Outcome::Val(Value::Array(vec![]));
            }
            (Value::Array(items), "is_empty") => {
                return Outcome::Val(Value::Bool(items.is_empty()));
            }
            (Value::Array(items), "contains") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let val = try_val!(self.eval_expr(&args[0]));
                let val_str = val.to_string();
                let found = items.iter().any(|item| item.to_string() == val_str);
                return Outcome::Val(Value::Bool(found));
            }
            (Value::Array(items), "reverse") => {
                let mut items = items.clone();
                items.reverse();
                return Outcome::Val(Value::Array(items));
            }
            (Value::Array(items), "sorted") | (Value::Array(items), "sort") => {
                let mut items = items.clone();
                items.sort_by(|a, b| match (a, b) {
                    (Value::Int(a), Value::Int(b)) => a.cmp(b),
                    (Value::Float(a), Value::Float(b)) => {
                        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    _ => a.to_string().cmp(&b.to_string()),
                });
                return Outcome::Val(Value::Array(items));
            }
            (Value::Array(items), "min") => {
                if items.is_empty() {
                    return Outcome::Val(Value::Variant {
                        name: "None".into(),
                        fields: vec![],
                    });
                }
                let mut best = items[0].clone();
                for item in &items[1..] {
                    match (&best, item) {
                        (Value::Int(a), Value::Int(b)) if b < a => best = item.clone(),
                        (Value::Float(a), Value::Float(b)) if b < a => best = item.clone(),
                        _ => {}
                    }
                }
                return Outcome::Val(best);
            }
            (Value::Array(items), "max") => {
                if items.is_empty() {
                    return Outcome::Val(Value::Variant {
                        name: "None".into(),
                        fields: vec![],
                    });
                }
                let mut best = items[0].clone();
                for item in &items[1..] {
                    match (&best, item) {
                        (Value::Int(a), Value::Int(b)) if b > a => best = item.clone(),
                        (Value::Float(a), Value::Float(b)) if b > a => best = item.clone(),
                        _ => {}
                    }
                }
                return Outcome::Val(best);
            }
            (Value::Array(items), "sum") => {
                let mut total: i128 = 0;
                let mut is_float = false;
                let mut ftotal: f64 = 0.0;
                for item in items {
                    match item {
                        Value::Int(n) => {
                            total += n;
                            ftotal += *n as f64;
                        }
                        Value::Float(f) => {
                            is_float = true;
                            ftotal += f;
                        }
                        _ => {}
                    }
                }
                return if is_float {
                    Outcome::Val(Value::Float(ftotal))
                } else {
                    Outcome::Val(Value::Int(total))
                };
            }
            (Value::Array(items), "enumerate") => {
                let pairs: Vec<Value> = items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Value::Array(vec![Value::Int(i as i128), v.clone()]))
                    .collect();
                return Outcome::Val(Value::Array(pairs));
            }
            (Value::Array(items), "flatten") => {
                let mut flat = Vec::new();
                for item in items {
                    if let Value::Array(inner) = item {
                        flat.extend(inner.iter().cloned());
                    } else {
                        flat.push(item.clone());
                    }
                }
                return Outcome::Val(Value::Array(flat));
            }
            (Value::Array(items), "dedup") => {
                let mut seen = Vec::new();
                let mut result = Vec::new();
                for item in items {
                    let key = item.to_string();
                    if !seen.contains(&key) {
                        seen.push(key);
                        result.push(item.clone());
                    }
                }
                return Outcome::Val(Value::Array(result));
            }
            (Value::Array(items), "join") => {
                if args.len() != 1 {
                    return Outcome::Error("join requires 1 argument".into());
                }
                let sep = try_val!(self.eval_expr(&args[0]));
                if let Value::String(sep) = sep {
                    let result: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                    return Outcome::Val(Value::String(result.join(&sep)));
                }
                return Outcome::Val(Value::Unit);
            }
            (Value::Array(items), "get") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let idx = try_val!(self.eval_expr(&args[0]));
                if let Value::Int(i) = idx {
                    return match items.get(i as usize) {
                        Some(v) => Outcome::Val(v.clone()),
                        None => Outcome::Error("get: index out of bounds".into()),
                    };
                }
                return Outcome::Val(Value::Unit);
            }
            (Value::Array(items), "set") => {
                if args.len() != 2 {
                    return Outcome::Error("set requires 2 arguments".into());
                }
                let idx = try_val!(self.eval_expr(&args[0]));
                let val = try_val!(self.eval_expr(&args[1]));
                let mut items = items.clone();
                if let Value::Int(i) = idx {
                    let i = i as usize;
                    if i < items.len() {
                        items[i] = val;
                    }
                }
                return Outcome::Val(Value::Array(items));
            }
            (Value::Array(items), "last") => {
                return match items.last() {
                    Some(v) => Outcome::Val(Value::Variant {
                        name: "Some".into(),
                        fields: vec![v.clone()],
                    }),
                    None => Outcome::Val(Value::Variant {
                        name: "None".into(),
                        fields: vec![],
                    }),
                };
            }
            (Value::Array(items), "map") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let func = try_val!(self.eval_expr(&args[0]));
                let mut result = Vec::new();
                for item in items {
                    result.push(try_val!(self.call_value(&func, vec![item.clone()])));
                }
                return Outcome::Val(Value::Array(result));
            }
            (Value::Array(items), "filter") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let func = try_val!(self.eval_expr(&args[0]));
                let mut result = Vec::new();
                for item in items {
                    let keep = try_val!(self.call_value(&func, vec![item.clone()]));
                    if keep.is_truthy() {
                        result.push(item.clone());
                    }
                }
                return Outcome::Val(Value::Array(result));
            }
            (Value::Array(items), "fold") => {
                if args.len() != 2 {
                    return Outcome::Val(Value::Unit);
                }
                let mut acc = try_val!(self.eval_expr(&args[0]));
                let func = try_val!(self.eval_expr(&args[1]));
                for item in items {
                    acc = try_val!(self.call_value(&func, vec![acc, item.clone()]));
                }
                return Outcome::Val(acc);
            }
            (Value::Array(items), "each") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let func = try_val!(self.eval_expr(&args[0]));
                for item in items {
                    try_val!(self.call_value(&func, vec![item.clone()]));
                }
                return Outcome::Val(Value::Unit);
            }
            // ── String methods ───────────────────────────────────────
            (Value::String(s), "len") => {
                return Outcome::Val(Value::Int(s.chars().count() as i128));
            }
            (Value::String(s), "trim") => {
                return Outcome::Val(Value::String(s.trim().to_string()));
            }
            (Value::String(s), "contains") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let arg = try_val!(self.eval_expr(&args[0]));
                if let Value::String(sub) = arg {
                    return Outcome::Val(Value::Bool(s.contains(&sub)));
                }
                return Outcome::Val(Value::Unit);
            }
            (Value::String(s), "byte_at") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let idx = try_val!(self.eval_expr(&args[0]));
                if let Value::Int(i) = idx {
                    let i = i as usize;
                    if i < s.len() {
                        return Outcome::Val(Value::Int(s.as_bytes()[i] as i128));
                    }
                }
                return Outcome::Error("byte_at: index out of bounds".into());
            }
            (Value::String(s), "char_at") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let idx = try_val!(self.eval_expr(&args[0]));
                if let Value::Int(i) = idx
                    && let Some(ch) = s.chars().nth(i as usize)
                {
                    return Outcome::Val(Value::String(ch.to_string()));
                }
                return Outcome::Error("char_at: index out of bounds".into());
            }
            (Value::String(s), "substring") => {
                if args.len() != 2 {
                    return Outcome::Error("substring requires 2 arguments".into());
                }
                let start = try_val!(self.eval_expr(&args[0]));
                let end = try_val!(self.eval_expr(&args[1]));
                if let (Value::Int(s_idx), Value::Int(e_idx)) = (start, end) {
                    let s_idx = s_idx as usize;
                    let e_idx = e_idx as usize;
                    let char_count = s.chars().count();
                    if s_idx <= e_idx && e_idx <= char_count {
                        let result: String = s.chars().skip(s_idx).take(e_idx - s_idx).collect();
                        return Outcome::Val(Value::String(result));
                    }
                }
                return Outcome::Error("substring: invalid range".into());
            }
            (Value::String(s), "starts_with") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let arg = try_val!(self.eval_expr(&args[0]));
                if let Value::String(prefix) = arg {
                    return Outcome::Val(Value::Bool(s.starts_with(&prefix)));
                }
                return Outcome::Val(Value::Unit);
            }
            (Value::String(s), "ends_with") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let arg = try_val!(self.eval_expr(&args[0]));
                if let Value::String(suffix) = arg {
                    return Outcome::Val(Value::Bool(s.ends_with(&suffix)));
                }
                return Outcome::Val(Value::Unit);
            }
            (Value::String(s), "find") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let arg = try_val!(self.eval_expr(&args[0]));
                if let Value::String(needle) = arg {
                    return match s.find(&needle) {
                        Some(i) => Outcome::Val(Value::Int(i as i128)),
                        None => Outcome::Val(Value::Int(-1)),
                    };
                }
                return Outcome::Val(Value::Int(-1));
            }
            (Value::String(s), "split") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let arg = try_val!(self.eval_expr(&args[0]));
                if let Value::String(delim) = arg {
                    let parts: Vec<Value> = s
                        .split(&delim)
                        .map(|p| Value::String(p.to_string()))
                        .collect();
                    return Outcome::Val(Value::Array(parts));
                }
                return Outcome::Val(Value::Unit);
            }
            (Value::String(s), "replace") => {
                if args.len() != 2 {
                    return Outcome::Error("replace requires 2 arguments".into());
                }
                let from = try_val!(self.eval_expr(&args[0]));
                let to = try_val!(self.eval_expr(&args[1]));
                if let (Value::String(f), Value::String(t)) = (from, to) {
                    return Outcome::Val(Value::String(s.replace(&f, &t)));
                }
                return Outcome::Val(Value::Unit);
            }
            (Value::String(s), "to_upper") => {
                return Outcome::Val(Value::String(s.to_uppercase()));
            }
            (Value::String(s), "to_lower") => {
                return Outcome::Val(Value::String(s.to_lowercase()));
            }
            (Value::String(s), "is_empty") => {
                return Outcome::Val(Value::Bool(s.is_empty()));
            }
            (Value::String(s), "is_digit") => {
                return Outcome::Val(Value::Bool(
                    s.len() == 1 && s.chars().next().unwrap().is_ascii_digit(),
                ));
            }
            (Value::String(s), "lines") => {
                let parts: Vec<Value> = s.lines().map(|l| Value::String(l.to_string())).collect();
                return Outcome::Val(Value::Array(parts));
            }
            (Value::String(s), "chars") => {
                let chars: Vec<Value> = s.chars().map(|c| Value::String(c.to_string())).collect();
                return Outcome::Val(Value::Array(chars));
            }
            (Value::String(s), "repeat") => {
                if args.len() != 1 {
                    return Outcome::Error("repeat requires 1 argument".into());
                }
                let n = try_val!(self.eval_expr(&args[0]));
                if let Value::Int(n) = n {
                    return Outcome::Val(Value::String(s.repeat(n as usize)));
                }
                return Outcome::Error("repeat: argument must be an integer".into());
            }
            (Value::String(s), "parse_int") => {
                return match s.trim().parse::<i128>() {
                    Ok(n) => Outcome::Val(Value::Variant {
                        name: "Ok".into(),
                        fields: vec![Value::Int(n)],
                    }),
                    Err(_) => Outcome::Val(Value::Variant {
                        name: "Err".into(),
                        fields: vec![Value::String(format!("cannot parse '{s}' as int"))],
                    }),
                };
            }
            (Value::String(s), "parse_float") => {
                return match s.trim().parse::<f64>() {
                    Ok(f) => Outcome::Val(Value::Variant {
                        name: "Ok".into(),
                        fields: vec![Value::Float(f)],
                    }),
                    Err(_) => Outcome::Val(Value::Variant {
                        name: "Err".into(),
                        fields: vec![Value::String(format!("cannot parse '{s}' as float"))],
                    }),
                };
            }
            (Value::String(s), "is_alpha") => {
                return Outcome::Val(Value::Bool(
                    s.len() == 1 && s.chars().next().unwrap().is_alphabetic(),
                ));
            }
            (Value::String(s), "is_whitespace") => {
                return Outcome::Val(Value::Bool(
                    s.len() == 1 && s.chars().next().unwrap().is_whitespace(),
                ));
            }
            // ── Float methods ───────────────────────────────────────
            (Value::Float(f), "sqrt") => {
                return Outcome::Val(Value::Float(f.sqrt()));
            }
            (Value::Float(f), "abs") => {
                return Outcome::Val(Value::Float(f.abs()));
            }
            (Value::Float(f), "floor") => {
                return Outcome::Val(Value::Float(f.floor()));
            }
            (Value::Float(f), "ceil") => {
                return Outcome::Val(Value::Float(f.ceil()));
            }
            (Value::Float(f), "round") => {
                return Outcome::Val(Value::Float(f.round()));
            }
            // ── Result/Option methods ────────────────────────────────
            (Value::Variant { name, .. }, "is_ok") => {
                return Outcome::Val(Value::Bool(name == "Ok"));
            }
            (Value::Variant { name, .. }, "is_err") => {
                return Outcome::Val(Value::Bool(name == "Err"));
            }
            (Value::Variant { name, .. }, "is_some") => {
                return Outcome::Val(Value::Bool(name == "Some"));
            }
            (Value::Variant { name, .. }, "is_none") => {
                return Outcome::Val(Value::Bool(name == "None"));
            }
            (Value::Variant { name, fields }, "unwrap") => {
                if (name == "Ok" || name == "Some") && !fields.is_empty() {
                    return Outcome::Val(fields[0].clone());
                }
                return Outcome::Error(format!("Called unwrap() on {name}"));
            }
            (Value::Variant { name, fields }, "unwrap_or") => {
                if (name == "Ok" || name == "Some") && !fields.is_empty() {
                    return Outcome::Val(fields[0].clone());
                }
                if args.len() == 1 {
                    return Outcome::Val(try_val!(self.eval_expr(&args[0])));
                }
                return Outcome::Val(Value::Unit);
            }
            (Value::Variant { name, fields }, "map") => {
                if (name == "Ok" || name == "Some") && !fields.is_empty() && args.len() == 1 {
                    let func = try_val!(self.eval_expr(&args[0]));
                    let mapped = try_val!(self.call_value(&func, vec![fields[0].clone()]));
                    return Outcome::Val(Value::Variant {
                        name: name.clone(),
                        fields: vec![mapped],
                    });
                }
                // None/Err pass through unchanged.
                return Outcome::Val(obj.clone());
            }
            // ── File methods (static) ────────────────────────────────
            (Value::Variant { name: vname, .. }, "read") if vname == "File" => {
                if args.len() != 1 {
                    return Outcome::Error("File.read requires 1 argument".into());
                }
                let path = try_val!(self.eval_expr(&args[0]));
                if let Value::String(path) = path {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            return Outcome::Val(Value::Variant {
                                name: "Ok".into(),
                                fields: vec![Value::String(content)],
                            });
                        }
                        Err(e) => {
                            return Outcome::Val(Value::Variant {
                                name: "Err".into(),
                                fields: vec![Value::String(e.to_string())],
                            });
                        }
                    }
                }
                return Outcome::Error("File.read: path must be a string".into());
            }
            (Value::Variant { name: vname, .. }, "read_lines") if vname == "File" => {
                if args.len() != 1 {
                    return Outcome::Error("File.read_lines requires 1 argument".into());
                }
                let path = try_val!(self.eval_expr(&args[0]));
                if let Value::String(path) = path {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            let lines: Vec<Value> = content
                                .lines()
                                .map(|l| Value::String(l.to_string()))
                                .collect();
                            return Outcome::Val(Value::Variant {
                                name: "Ok".into(),
                                fields: vec![Value::Array(lines)],
                            });
                        }
                        Err(e) => {
                            return Outcome::Val(Value::Variant {
                                name: "Err".into(),
                                fields: vec![Value::String(e.to_string())],
                            });
                        }
                    }
                }
                return Outcome::Error("File.read_lines: path must be a string".into());
            }
            (Value::Variant { name: vname, .. }, "write") if vname == "File" => {
                if args.len() != 2 {
                    return Outcome::Error("File.write requires 2 arguments".into());
                }
                let path = try_val!(self.eval_expr(&args[0]));
                let content = try_val!(self.eval_expr(&args[1]));
                if let (Value::String(path), Value::String(content)) = (path, content) {
                    match std::fs::write(&path, &content) {
                        Ok(()) => {
                            return Outcome::Val(Value::Variant {
                                name: "Ok".into(),
                                fields: vec![Value::Unit],
                            });
                        }
                        Err(e) => {
                            return Outcome::Val(Value::Variant {
                                name: "Err".into(),
                                fields: vec![Value::String(e.to_string())],
                            });
                        }
                    }
                }
                return Outcome::Error("File.write: path and content must be strings".into());
            }
            (Value::Variant { name: vname, .. }, "exists") if vname == "File" => {
                if args.len() != 1 {
                    return Outcome::Error("File.exists requires 1 argument".into());
                }
                let path = try_val!(self.eval_expr(&args[0]));
                if let Value::String(path) = path {
                    return Outcome::Val(Value::Bool(std::path::Path::new(&path).exists()));
                }
                return Outcome::Val(Value::Bool(false));
            }
            // ── HashMap methods ──────────────────────────────────────
            (Value::Map(entries), "insert") => {
                if args.len() != 2 {
                    return Outcome::Error("insert requires 2 arguments".into());
                }
                let key = try_val!(self.eval_expr(&args[0]));
                let val = try_val!(self.eval_expr(&args[1]));
                let mut entries = entries.clone();
                // Update existing or append.
                let key_str = key.to_string();
                if let Some(entry) = entries.iter_mut().find(|(k, _)| k.to_string() == key_str) {
                    entry.1 = val;
                } else {
                    entries.push((key, val));
                }
                return Outcome::Val(Value::Map(entries));
            }
            (Value::Map(entries), "get") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let key = try_val!(self.eval_expr(&args[0]));
                let key_str = key.to_string();
                return match entries.iter().find(|(k, _)| k.to_string() == key_str) {
                    Some((_, v)) => Outcome::Val(Value::Variant {
                        name: "Some".into(),
                        fields: vec![v.clone()],
                    }),
                    None => Outcome::Val(Value::Variant {
                        name: "None".into(),
                        fields: vec![],
                    }),
                };
            }
            (Value::Map(entries), "contains_key") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let key = try_val!(self.eval_expr(&args[0]));
                let key_str = key.to_string();
                let found = entries.iter().any(|(k, _)| k.to_string() == key_str);
                return Outcome::Val(Value::Bool(found));
            }
            (Value::Map(entries), "remove") => {
                if args.len() != 1 {
                    return Outcome::Val(Value::Unit);
                }
                let key = try_val!(self.eval_expr(&args[0]));
                let key_str = key.to_string();
                let mut entries = entries.clone();
                entries.retain(|(k, _)| k.to_string() != key_str);
                return Outcome::Val(Value::Map(entries));
            }
            (Value::Map(entries), "len") => {
                return Outcome::Val(Value::Int(entries.len() as i128));
            }
            (Value::Map(entries), "is_empty") => {
                return Outcome::Val(Value::Bool(entries.is_empty()));
            }
            (Value::Map(entries), "keys") => {
                let keys: Vec<Value> = entries.iter().map(|(k, _)| k.clone()).collect();
                return Outcome::Val(Value::Array(keys));
            }
            (Value::Map(entries), "values") => {
                let vals: Vec<Value> = entries.iter().map(|(_, v)| v.clone()).collect();
                return Outcome::Val(Value::Array(vals));
            }
            (Value::Map(entries), "entries") => {
                let pairs: Vec<Value> = entries
                    .iter()
                    .map(|(k, v)| Value::Array(vec![k.clone(), v.clone()]))
                    .collect();
                return Outcome::Val(Value::Array(pairs));
            }
            (Value::Map(entries), "get_or") => {
                if args.len() != 2 {
                    return Outcome::Error("get_or requires 2 arguments (key, default)".into());
                }
                let key = try_val!(self.eval_expr(&args[0]));
                let default = try_val!(self.eval_expr(&args[1]));
                let key_str = key.to_string();
                return match entries.iter().find(|(k, _)| k.to_string() == key_str) {
                    Some((_, v)) => Outcome::Val(v.clone()),
                    None => Outcome::Val(default),
                };
            }
            _ => {}
        }

        // User-defined methods.
        // Determine the type name and whether this is a static call (Type.method())
        // or an instance call (value.method()).
        let (type_name, is_static) = match &obj {
            Value::Struct { name, .. } => (name.clone(), false),
            // A Variant with empty fields is our sentinel for a type reference
            // (from `Vec2.new(...)` where Vec2 is not a value).
            Value::Variant { name, fields }
                if fields.is_empty() && self.methods.contains_key(name.as_str()) =>
            {
                (name.clone(), true)
            }
            _ => (obj.type_name().to_string(), false),
        };

        if let Some(type_methods) = self.methods.get(&type_name).cloned()
            && let Some(method_fn) = type_methods.get(method).cloned()
        {
            let mut arg_vals = if is_static {
                // Static call: don't pass the type reference as self.
                Vec::new()
            } else {
                vec![obj]
            };
            for arg in args {
                arg_vals.push(try_val!(self.eval_expr(arg)));
            }
            return self.call_function(&method_fn, arg_vals);
        }

        Outcome::Error(format!("No method '{method}' on type '{type_name}'"))
    }

    fn call_value(&mut self, func: &Value, args: Vec<Value>) -> Outcome {
        match func {
            Value::Function(FnValue::UserDefined { params, body, .. }) => {
                self.env.push_scope();
                for (param, val) in params.iter().zip(args) {
                    self.env.define(param.name.clone(), val);
                }
                let result = match self.eval_block_inner(body) {
                    Outcome::Return(v) => Outcome::Val(v),
                    other => other,
                };
                self.env.pop_scope();
                result
            }
            Value::Function(FnValue::Closure { params, body, env }) => {
                let saved_env = self.env.clone();
                self.env = env.clone();
                self.env.push_scope();
                for (param, val) in params.iter().zip(args) {
                    self.env.define(param.name.clone(), val);
                }
                let result = self.eval_expr(body);
                self.env.pop_scope();
                self.env = saved_env;
                result
            }
            Value::Function(FnValue::Builtin(name)) => self.call_builtin(name, args),
            _ => Outcome::Val(Value::Unit),
        }
    }

    fn call_function(&mut self, func: &Function, args: Vec<Value>) -> Outcome {
        self.env.push_scope();
        for (param, val) in func.params.iter().zip(args) {
            self.env.define(param.name.clone(), val);
        }
        let result = match self.eval_block_inner(&func.body) {
            Outcome::Return(v) => Outcome::Val(v),
            other => other,
        };
        // Capture modified self before popping scope (for mut self methods).
        let modified_self = if func
            .params
            .first()
            .is_some_and(|p| p.name == "self" && p.mutable)
        {
            self.env.get("self").cloned()
        } else {
            None
        };
        self.env.pop_scope();
        // Store modified self so the caller can write it back.
        if let Some(new_self) = modified_self {
            self.last_modified_self = Some(new_self);
        }
        result
    }

    fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> Outcome {
        match name {
            "print" | "println" => {
                let text: Vec<String> = args.iter().map(|v| v.to_string()).collect();
                let line = text.join(" ");
                if let Some(output) = &mut self.output {
                    output.push(line);
                } else {
                    println!("{line}");
                }
                Outcome::Val(Value::Unit)
            }
            "eprint" => {
                let text: Vec<String> = args.iter().map(|v| v.to_string()).collect();
                eprintln!("{}", text.join(" "));
                Outcome::Val(Value::Unit)
            }
            "to_str" => {
                let val = args.first().cloned().unwrap_or(Value::Unit);
                Outcome::Val(Value::String(val.to_string()))
            }
            "to_int" => {
                if let Some(Value::String(s)) = args.first() {
                    match s.parse::<i128>() {
                        Ok(n) => Outcome::Val(Value::Int(n)),
                        Err(_) => Outcome::Error(format!("Cannot parse '{s}' as integer")),
                    }
                } else {
                    Outcome::Error("to_int expects a string argument".into())
                }
            }
            "to_float" => {
                if let Some(Value::String(s)) = args.first() {
                    match s.parse::<f64>() {
                        Ok(f) => Outcome::Val(Value::Float(f)),
                        Err(_) => Outcome::Error(format!("Cannot parse '{s}' as float")),
                    }
                } else {
                    Outcome::Error("to_float expects a string argument".into())
                }
            }
            "abs" => match args.first() {
                Some(Value::Int(n)) => Outcome::Val(Value::Int(n.abs())),
                Some(Value::Float(f)) => Outcome::Val(Value::Float(f.abs())),
                _ => Outcome::Val(Value::Unit),
            },
            "min" => match (args.first(), args.get(1)) {
                (Some(Value::Int(a)), Some(Value::Int(b))) => Outcome::Val(Value::Int(*a.min(b))),
                (Some(Value::Float(a)), Some(Value::Float(b))) => {
                    Outcome::Val(Value::Float(a.min(*b)))
                }
                _ => Outcome::Val(Value::Unit),
            },
            "max" => match (args.first(), args.get(1)) {
                (Some(Value::Int(a)), Some(Value::Int(b))) => Outcome::Val(Value::Int(*a.max(b))),
                (Some(Value::Float(a)), Some(Value::Float(b))) => {
                    Outcome::Val(Value::Float(a.max(*b)))
                }
                _ => Outcome::Val(Value::Unit),
            },
            "assert" => match args.first() {
                Some(val) if val.is_truthy() => Outcome::Val(Value::Unit),
                _ => Outcome::Error("Assertion failed".into()),
            },
            "assert_eq" => match (args.first(), args.get(1)) {
                (Some(a), Some(b)) if a.to_string() == b.to_string() => Outcome::Val(Value::Unit),
                (Some(a), Some(b)) => Outcome::Error(format!("Assertion failed: {} != {}", a, b)),
                _ => Outcome::Error("assert_eq requires two arguments".into()),
            },
            // Result/Option constructors.
            "Ok" => {
                let val = args.into_iter().next().unwrap_or(Value::Unit);
                Outcome::Val(Value::Variant {
                    name: "Ok".into(),
                    fields: vec![val],
                })
            }
            "Err" => {
                let val = args.into_iter().next().unwrap_or(Value::Unit);
                Outcome::Val(Value::Variant {
                    name: "Err".into(),
                    fields: vec![val],
                })
            }
            "Some" => {
                let val = args.into_iter().next().unwrap_or(Value::Unit);
                Outcome::Val(Value::Variant {
                    name: "Some".into(),
                    fields: vec![val],
                })
            }
            "HashMap" => Outcome::Val(Value::Map(vec![])),
            "args" => {
                let args_vec: Vec<Value> = std::env::args().map(Value::String).collect();
                Outcome::Val(Value::Array(args_vec))
            }
            "exit" => {
                let code = match args.first() {
                    Some(Value::Int(n)) => *n as i32,
                    _ => 0,
                };
                std::process::exit(code);
            }
            "exec" => {
                // exec(cmd, [args]) -> { success: bool, stdout: str, stderr: str, code: i64 }
                if args.is_empty() {
                    return Outcome::Error("exec requires at least 1 argument (command)".into());
                }
                let cmd = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Outcome::Error("exec: command must be a string".into()),
                };

                let mut cmd_args: Vec<String> = vec![];
                if let Some(Value::Array(items)) = args.get(1) {
                    for item in items {
                        if let Value::String(s) = item {
                            cmd_args.push(s.clone());
                        }
                    }
                }

                let output = std::process::Command::new(&cmd).args(&cmd_args).output();

                let make_result = |success: bool, stdout: String, stderr: String, code: i128| {
                    let mut fields = HashMap::new();
                    fields.insert("success".into(), Value::Bool(success));
                    fields.insert("stdout".into(), Value::String(stdout));
                    fields.insert("stderr".into(), Value::String(stderr));
                    fields.insert("code".into(), Value::Int(code));
                    Value::Struct {
                        name: "ExecResult".into(),
                        fields,
                    }
                };

                match output {
                    Ok(out) => {
                        let success = out.status.success();
                        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        let code = out.status.code().unwrap_or(-1) as i128;
                        Outcome::Val(make_result(success, stdout, stderr, code))
                    }
                    Err(e) => Outcome::Val(make_result(false, String::new(), e.to_string(), -1)),
                }
            }
            "input" => {
                // input() or input("prompt") — read a line from stdin
                if let Some(Value::String(prompt)) = args.first() {
                    eprint!("{prompt}");
                    use std::io::Write;
                    std::io::stderr().flush().ok();
                }
                let mut line = String::new();
                match std::io::stdin().read_line(&mut line) {
                    Ok(0) => Outcome::Val(Value::String(String::new())), // EOF
                    Ok(_) => {
                        // Strip trailing newline
                        if line.ends_with('\n') {
                            line.pop();
                            if line.ends_with('\r') {
                                line.pop();
                            }
                        }
                        Outcome::Val(Value::String(line))
                    }
                    Err(e) => Outcome::Error(format!("input error: {e}")),
                }
            }
            "stdin_lines" => {
                // stdin_lines() -> [str] — read all lines from stdin
                use std::io::BufRead;
                let mut lines = Vec::new();
                let stdin = std::io::stdin();
                for line in stdin.lock().lines() {
                    match line {
                        Ok(l) => lines.push(Value::String(l)),
                        Err(_) => break,
                    }
                }
                Outcome::Val(Value::Array(lines))
            }
            "env_get" => {
                // env_get(key) -> str (empty string if not set)
                if let Some(Value::String(key)) = args.first() {
                    let val = std::env::var(key).unwrap_or_default();
                    Outcome::Val(Value::String(val))
                } else {
                    Outcome::Error("env_get requires a string argument".into())
                }
            }
            "env_set" => {
                // env_set(key, value)
                if let [Value::String(key), Value::String(val), ..] = args.as_slice() {
                    // SAFETY: Forge is single-threaded; no concurrent env access.
                    unsafe { std::env::set_var(key, val) };
                    Outcome::Val(Value::Unit)
                } else {
                    Outcome::Error("env_set requires two string arguments".into())
                }
            }
            "env_vars" => {
                // env_vars() -> [[key, value], ...]
                let pairs: Vec<Value> = std::env::vars()
                    .map(|(k, v)| Value::Array(vec![Value::String(k), Value::String(v)]))
                    .collect();
                Outcome::Val(Value::Array(pairs))
            }
            _ => Outcome::Val(Value::Unit),
        }
    }

    // ── Assignment ──────────────────────────────────────────────────────

    fn eval_assign(&mut self, target: &Expr, op: Option<BinOp>, value: Value) {
        match &target.kind {
            ExprKind::Identifier(name) => {
                let final_val = if let Some(bin_op) = op {
                    if let Some(current) = self.env.get_deref(name) {
                        eval_binop(&current, bin_op, &value).unwrap_or(value)
                    } else {
                        value
                    }
                } else {
                    value
                };
                let _ = self.env.set_deref(name, final_val);
            }
            ExprKind::FieldAccess { object, field } => {
                let obj_name = match &object.kind {
                    ExprKind::Identifier(n) => Some(n.as_str()),
                    ExprKind::SelfValue => Some("self"),
                    _ => None,
                };
                if let Some(obj_name) = obj_name
                    && let Some(Value::Struct { name, mut fields }) = self.env.get_deref(obj_name)
                {
                    let final_val = if let Some(bin_op) = op {
                        if let Some(current) = fields.get(field) {
                            eval_binop(current, bin_op, &value).unwrap_or(value)
                        } else {
                            value
                        }
                    } else {
                        value
                    };
                    fields.insert(field.clone(), final_val);
                    let _ = self.env.set_deref(obj_name, Value::Struct { name, fields });
                }
            }
            ExprKind::Index { object, index } => {
                if let ExprKind::Identifier(arr_name) = &object.kind
                    && let Outcome::Val(Value::Int(i)) = self.eval_expr(index)
                    && let Some(Value::Array(mut items)) = self.env.get(arr_name).cloned()
                {
                    let i = i as usize;
                    if i < items.len() {
                        let final_val = if let Some(bin_op) = op {
                            eval_binop(&items[i], bin_op, &value).unwrap_or(value)
                        } else {
                            value
                        };
                        items[i] = final_val;
                        let _ = self.env.set(arr_name, Value::Array(items));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Try to extract a value from an Outcome. If it's a control flow signal,
/// return it from the enclosing function.
macro_rules! try_val {
    ($e:expr) => {
        match $e {
            Outcome::Val(v) => v,
            other => return other,
        }
    };
}
use try_val;

// ── Pattern matching ────────────────────────────────────────────────────

fn match_pattern(pattern: &Pattern, value: &Value) -> Option<Vec<(String, Value)>> {
    match &pattern.kind {
        PatternKind::Wildcard => Some(vec![]),
        PatternKind::Identifier(name) => Some(vec![(name.clone(), value.clone())]),
        PatternKind::Literal(expr) => match (&expr.kind, value) {
            (ExprKind::IntLiteral(a), Value::Int(b)) if a == b => Some(vec![]),
            (ExprKind::FloatLiteral(a), Value::Float(b)) if (a - b).abs() < f64::EPSILON => {
                Some(vec![])
            }
            (ExprKind::BoolLiteral(a), Value::Bool(b)) if a == b => Some(vec![]),
            (ExprKind::StringLiteral(a), Value::String(b)) if a == b => Some(vec![]),
            _ => None,
        },
        PatternKind::Variant { path, fields } => {
            if let Value::Variant {
                name: v_name,
                fields: v_fields,
            } = value
            {
                let pattern_name = path.last()?;
                if pattern_name != v_name || fields.len() != v_fields.len() {
                    return None;
                }
                let mut bindings = Vec::new();
                for (pat, val) in fields.iter().zip(v_fields) {
                    bindings.extend(match_pattern(pat, val)?);
                }
                Some(bindings)
            } else {
                None
            }
        }
    }
}

// ── Binary operators ────────────────────────────────────────────────────

fn eval_binop(lhs: &Value, op: BinOp, rhs: &Value) -> Result<Value, String> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => match op {
            BinOp::Add => Ok(Value::Int(a + b)),
            BinOp::Sub => Ok(Value::Int(a - b)),
            BinOp::Mul => Ok(Value::Int(a * b)),
            BinOp::Div => {
                if *b == 0 {
                    Err("Division by zero".into())
                } else {
                    Ok(Value::Int(a / b))
                }
            }
            BinOp::Mod => {
                if *b == 0 {
                    Err("Modulo by zero".into())
                } else {
                    Ok(Value::Int(a % b))
                }
            }
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::NotEq => Ok(Value::Bool(a != b)),
            BinOp::Lt => Ok(Value::Bool(a < b)),
            BinOp::Gt => Ok(Value::Bool(a > b)),
            BinOp::LtEq => Ok(Value::Bool(a <= b)),
            BinOp::GtEq => Ok(Value::Bool(a >= b)),
            BinOp::And | BinOp::Or => Err("Logical operators not supported on integers".into()),
        },
        (Value::Float(a), Value::Float(b)) => match op {
            BinOp::Add => Ok(Value::Float(a + b)),
            BinOp::Sub => Ok(Value::Float(a - b)),
            BinOp::Mul => Ok(Value::Float(a * b)),
            BinOp::Div => Ok(Value::Float(a / b)),
            BinOp::Mod => Ok(Value::Float(a % b)),
            BinOp::Eq => Ok(Value::Bool((a - b).abs() < f64::EPSILON)),
            BinOp::NotEq => Ok(Value::Bool((a - b).abs() >= f64::EPSILON)),
            BinOp::Lt => Ok(Value::Bool(a < b)),
            BinOp::Gt => Ok(Value::Bool(a > b)),
            BinOp::LtEq => Ok(Value::Bool(a <= b)),
            BinOp::GtEq => Ok(Value::Bool(a >= b)),
            _ => Err(format!("Invalid float operation: {:?}", op)),
        },
        (Value::Int(a), Value::Float(_)) => eval_binop(&Value::Float(*a as f64), op, rhs),
        (Value::Float(_), Value::Int(b)) => eval_binop(lhs, op, &Value::Float(*b as f64)),
        (Value::Bool(a), Value::Bool(b)) => match op {
            BinOp::And => Ok(Value::Bool(*a && *b)),
            BinOp::Or => Ok(Value::Bool(*a || *b)),
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::NotEq => Ok(Value::Bool(a != b)),
            _ => Err(format!("Invalid bool operation: {:?}", op)),
        },
        (Value::String(a), Value::String(b)) => match op {
            BinOp::Add => Ok(Value::String(format!("{a}{b}"))),
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::NotEq => Ok(Value::Bool(a != b)),
            _ => Err(format!("Invalid string operation: {:?}", op)),
        },
        _ => Err(format!(
            "Cannot apply {:?} to {} and {}",
            op,
            lhs.type_name(),
            rhs.type_name()
        )),
    }
}

/// Map a binary operator to its trait method name.
fn op_method_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Mod => "mod_",
        BinOp::Eq | BinOp::NotEq => "eq",
        BinOp::Lt => "lt",
        BinOp::Gt => "gt",
        BinOp::LtEq => "le",
        BinOp::GtEq => "ge",
        BinOp::And => "and",
        BinOp::Or => "or",
    }
}

/// Map a binary operator to its trait name.
#[allow(dead_code)]
fn op_trait_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "Add",
        BinOp::Sub => "Sub",
        BinOp::Mul => "Mul",
        BinOp::Div => "Div",
        BinOp::Mod => "Mod",
        BinOp::Eq | BinOp::NotEq => "Eq",
        BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => "Ord",
        BinOp::And | BinOp::Or => "Logic",
    }
}
