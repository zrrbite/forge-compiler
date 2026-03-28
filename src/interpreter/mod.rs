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
        ];
        for name in builtins {
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
            ItemKind::Struct(_) | ItemKind::Enum(_) | ItemKind::Trait(_) => {}
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
        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i == block.stmts.len() - 1;
            match &stmt.kind {
                StmtKind::Let { name, value, .. } => {
                    let val = match value {
                        Some(expr) => try_val!(self.eval_expr(expr)),
                        None => Value::Unit,
                    };
                    self.env.define(name.clone(), val);
                    last = Value::Unit;
                }
                StmtKind::Return(expr) => {
                    let val = match expr {
                        Some(e) => try_val!(self.eval_expr(e)),
                        None => Value::Unit,
                    };
                    return Outcome::Return(val);
                }
                StmtKind::Break => return Outcome::Break,
                StmtKind::Continue => return Outcome::Continue,
                StmtKind::Expr(expr) => match self.eval_expr(expr) {
                    Outcome::Val(v) => {
                        if is_last {
                            last = v;
                        }
                    }
                    other => return other,
                },
            }
        }
        Outcome::Val(last)
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

            ExprKind::Identifier(name) => match self.env.get(name).cloned() {
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
                    Value::Variant { name, fields } if name == "Ok" => {
                        Outcome::Val(fields.first().cloned().unwrap_or(Value::Unit))
                    }
                    Value::Variant { name, .. } if name == "Err" => Outcome::Return(val),
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
                if let Value::Array(items) = iterable {
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

    fn eval_call(&mut self, callee: &Expr, args: &[Expr]) -> Outcome {
        // Method call: `obj.method(args)`
        if let ExprKind::FieldAccess { object, field } = &callee.kind {
            let obj = try_val!(self.eval_expr(object));
            return self.eval_method_call(obj, field, args);
        }

        let func = try_val!(self.eval_expr(callee));
        let mut arg_vals = Vec::new();
        for arg in args {
            arg_vals.push(try_val!(self.eval_expr(arg)));
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
            (Value::String(s), "len") => {
                return Outcome::Val(Value::Int(s.len() as i128));
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
            (Value::Float(f), "sqrt") => {
                return Outcome::Val(Value::Float(f.sqrt()));
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
        self.env.pop_scope();
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
            _ => Outcome::Val(Value::Unit),
        }
    }

    // ── Assignment ──────────────────────────────────────────────────────

    fn eval_assign(&mut self, target: &Expr, op: Option<BinOp>, value: Value) {
        match &target.kind {
            ExprKind::Identifier(name) => {
                let final_val = if let Some(bin_op) = op {
                    if let Some(current) = self.env.get(name).cloned() {
                        eval_binop(&current, bin_op, &value).unwrap_or(value)
                    } else {
                        value
                    }
                } else {
                    value
                };
                let _ = self.env.set(name, final_val);
            }
            ExprKind::FieldAccess { object, field } => {
                if let ExprKind::Identifier(obj_name) = &object.kind
                    && let Some(Value::Struct { name, mut fields }) =
                        self.env.get(obj_name).cloned()
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
                    let _ = self.env.set(obj_name, Value::Struct { name, fields });
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
