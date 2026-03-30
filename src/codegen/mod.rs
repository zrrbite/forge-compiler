//! LLVM code generation for Forge.
//!
//! Lowers the HIR into LLVM IR using inkwell, then compiles to a native binary.
//! Supports: functions, structs, methods, field access, integers, floats,
//! booleans, strings, arithmetic, comparisons, let bindings, if/else,
//! while loops, for loops, print, and operator overloading.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::Path;

use inkwell::AddressSpace;
use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::OptimizationLevel;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, StructType};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, PointerValue};

use crate::hir::*;

#[derive(Debug)]
pub struct CodegenError(pub String);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Codegen error: {}", self.0)
    }
}

/// The LLVM code generator.
pub struct Codegen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    /// Variable name → (alloca pointer, pointee type).
    variables: Vec<HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>>,
    /// Function name → LLVM function value.
    functions: HashMap<String, FunctionValue<'ctx>>,
    /// Struct name → (LLVM struct type, field names in order).
    struct_types: HashMap<String, (StructType<'ctx>, Vec<String>)>,
    /// Method name mangled as "Type.method" → LLVM function value.
    method_functions: HashMap<String, FunctionValue<'ctx>>,
    /// Current impl target (set during method compilation).
    current_impl_target: Option<String>,
    /// Variable name → Forge struct type name (for method dispatch).
    var_struct_names: Vec<HashMap<String, String>>,
    printf_fn: FunctionValue<'ctx>,
}

impl<'ctx> Codegen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();

        let printf_type = context
            .i32_type()
            .fn_type(&[context.ptr_type(AddressSpace::default()).into()], true);
        let printf_fn = module.add_function("printf", printf_type, None);

        let mut codegen = Self {
            context,
            module,
            builder,
            variables: vec![HashMap::new()],
            functions: HashMap::new(),
            struct_types: HashMap::new(),
            method_functions: HashMap::new(),
            current_impl_target: None,
            var_struct_names: vec![HashMap::new()],
            printf_fn,
        };
        codegen.emit_string_runtime();
        codegen.emit_array_runtime();
        codegen
    }

    /// Emit the forge_str_concat runtime function as LLVM IR.
    fn emit_string_runtime(&mut self) {
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let i64_type = self.context.i64_type();

        // Declare strlen, malloc, memcpy
        let strlen_ty = i64_type.fn_type(&[ptr_type.into()], false);
        let strlen_fn = self
            .module
            .get_function("strlen")
            .unwrap_or_else(|| self.module.add_function("strlen", strlen_ty, None));

        let malloc_ty = ptr_type.fn_type(&[i64_type.into()], false);
        let malloc_fn = self
            .module
            .get_function("malloc")
            .unwrap_or_else(|| self.module.add_function("malloc", malloc_ty, None));

        let memcpy_ty =
            ptr_type.fn_type(&[ptr_type.into(), ptr_type.into(), i64_type.into()], false);
        let memcpy_fn = self
            .module
            .get_function("memcpy")
            .unwrap_or_else(|| self.module.add_function("memcpy", memcpy_ty, None));

        // Build forge_str_concat(a: ptr, b: ptr) -> ptr
        let concat_ty = ptr_type.fn_type(&[ptr_type.into(), ptr_type.into()], false);
        let concat_fn = self
            .module
            .add_function("forge_str_concat", concat_ty, None);
        let entry = self.context.append_basic_block(concat_fn, "entry");
        self.builder.position_at_end(entry);

        let a = concat_fn.get_nth_param(0).unwrap().into_pointer_value();
        let b = concat_fn.get_nth_param(1).unwrap().into_pointer_value();

        // la = strlen(a), lb = strlen(b)
        let la = match self
            .builder
            .build_call(strlen_fn, &[a.into()], "la")
            .unwrap()
            .try_as_basic_value()
        {
            inkwell::values::ValueKind::Basic(v) => v.into_int_value(),
            _ => i64_type.const_int(0, false),
        };
        let lb = match self
            .builder
            .build_call(strlen_fn, &[b.into()], "lb")
            .unwrap()
            .try_as_basic_value()
        {
            inkwell::values::ValueKind::Basic(v) => v.into_int_value(),
            _ => i64_type.const_int(0, false),
        };

        // total = la + lb + 1
        let sum = self.builder.build_int_add(la, lb, "sum").unwrap();
        let one = i64_type.const_int(1, false);
        let total = self.builder.build_int_add(sum, one, "total").unwrap();

        // r = malloc(total)
        let r = match self
            .builder
            .build_call(malloc_fn, &[total.into()], "r")
            .unwrap()
            .try_as_basic_value()
        {
            inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
            _ => ptr_type.const_null(),
        };

        // memcpy(r, a, la)
        self.builder
            .build_call(memcpy_fn, &[r.into(), a.into(), la.into()], "")
            .unwrap();

        // memcpy(r + la, b, lb + 1)  (include null terminator)
        let r_plus_la = unsafe {
            self.builder
                .build_gep(self.context.i8_type(), r, &[la], "r_off")
                .unwrap()
        };
        let lb_plus_1 = self.builder.build_int_add(lb, one, "lb1").unwrap();
        self.builder
            .build_call(
                memcpy_fn,
                &[r_plus_la.into(), b.into(), lb_plus_1.into()],
                "",
            )
            .unwrap();

        self.builder.build_return(Some(&r)).unwrap();
    }

    pub fn compile_program(&mut self, program: &HirProgram) -> Result<(), CodegenError> {
        // Pass 1: register struct types.
        for item in &program.items {
            if let HirItemKind::Struct(s) = &item.kind {
                self.register_struct(s)?;
            }
        }

        // Pass 2: declare all functions and methods.
        for item in &program.items {
            match &item.kind {
                HirItemKind::Function(func) => {
                    self.declare_function(func)?;
                }
                HirItemKind::Impl(imp) => {
                    for method in &imp.methods {
                        self.declare_method(&imp.target, method)?;
                    }
                }
                _ => {}
            }
        }

        // Pass 3: compile function and method bodies.
        for item in &program.items {
            match &item.kind {
                HirItemKind::Function(func) => {
                    self.compile_function(func, None)?;
                }
                HirItemKind::Impl(imp) => {
                    for method in &imp.methods {
                        self.compile_function(method, Some(&imp.target))?;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub fn write_ir(&self, path: &Path) -> Result<(), CodegenError> {
        self.module
            .print_to_file(path)
            .map_err(|e| CodegenError(format!("Failed to write IR: {}", e.to_string())))
    }

    pub fn write_object(&self, path: &Path) -> Result<(), CodegenError> {
        Target::initialize_native(&InitializationConfig::default())
            .map_err(|e| CodegenError(format!("Failed to initialize target: {e}")))?;

        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple)
            .map_err(|e| CodegenError(format!("Failed to get target: {}", e.to_string())))?;

        let cpu = TargetMachine::get_host_cpu_name();
        let features = TargetMachine::get_host_cpu_features();
        let machine = target
            .create_target_machine(
                &triple,
                cpu.to_str().unwrap_or("generic"),
                features.to_str().unwrap_or(""),
                OptimizationLevel::Aggressive, // -O3
                RelocMode::PIC,
                CodeModel::Default,
            )
            .ok_or_else(|| CodegenError("Failed to create target machine".into()))?;

        // Run LLVM optimization passes (-O3 equivalent).
        let pass_options = inkwell::passes::PassBuilderOptions::create();
        pass_options.set_loop_unrolling(true);
        pass_options.set_merge_functions(true);
        self.module
            .run_passes("default<O3>", &machine, pass_options)
            .map_err(|e| CodegenError(format!("Optimization failed: {}", e.to_string())))?;

        machine
            .write_to_file(&self.module, FileType::Object, path)
            .map_err(|e| CodegenError(format!("Failed to write object: {}", e.to_string())))
    }

    pub fn get_ir(&self) -> String {
        self.module.print_to_string().to_string()
    }

    /// Emit forge_array_push as LLVM IR: takes {ptr, i64} + i64, returns {ptr, i64}
    fn emit_array_runtime(&mut self) {
        let arr_type = self.array_type();
        let i64_type = self.context.i64_type();
        let ptr_type = self.context.ptr_type(AddressSpace::default());

        let realloc_ty = ptr_type.fn_type(&[ptr_type.into(), i64_type.into()], false);
        let realloc_fn = self
            .module
            .get_function("realloc")
            .unwrap_or_else(|| self.module.add_function("realloc", realloc_ty, None));

        // forge_array_push(arr: {ptr, i64}, val: i64) -> {ptr, i64}
        let push_ty = arr_type.fn_type(&[arr_type.into(), i64_type.into()], false);
        let push_fn = self
            .module
            .get_function("forge_array_push")
            .unwrap_or_else(|| self.module.add_function("forge_array_push", push_ty, None));

        let entry = self.context.append_basic_block(push_fn, "entry");
        self.builder.position_at_end(entry);

        let arr_param = push_fn.get_nth_param(0).unwrap().into_struct_value();
        let val_param = push_fn.get_nth_param(1).unwrap().into_int_value();

        // Extract data ptr and len
        let data = self
            .builder
            .build_extract_value(arr_param, 0, "data")
            .unwrap()
            .into_pointer_value();
        let len = self
            .builder
            .build_extract_value(arr_param, 1, "len")
            .unwrap()
            .into_int_value();

        // new_len = len + 1
        let one = i64_type.const_int(1, false);
        let new_len = self.builder.build_int_add(len, one, "new_len").unwrap();

        // new_size = new_len * 8
        let eight = i64_type.const_int(8, false);
        let new_size = self
            .builder
            .build_int_mul(new_len, eight, "new_size")
            .unwrap();

        // new_data = realloc(data, new_size)
        let new_data = match self
            .builder
            .build_call(realloc_fn, &[data.into(), new_size.into()], "new_data")
            .unwrap()
            .try_as_basic_value()
        {
            inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
            _ => ptr_type.const_null(),
        };

        // new_data[len] = val
        let elem_ptr = unsafe {
            self.builder
                .build_gep(i64_type, new_data, &[len], "elem_ptr")
                .unwrap()
        };
        self.builder.build_store(elem_ptr, val_param).unwrap();

        // Build result struct { new_data, new_len }
        let result_alloca = self.builder.build_alloca(arr_type, "result").unwrap();
        let data_field = self
            .builder
            .build_struct_gep(arr_type, result_alloca, 0, "res_data")
            .unwrap();
        self.builder.build_store(data_field, new_data).unwrap();
        let len_field = self
            .builder
            .build_struct_gep(arr_type, result_alloca, 1, "res_len")
            .unwrap();
        self.builder.build_store(len_field, new_len).unwrap();
        let result = self
            .builder
            .build_load(arr_type, result_alloca, "result_val")
            .unwrap();
        self.builder.build_return(Some(&result)).unwrap();
    }

    /// Returns the LLVM struct type for Forge arrays: { ptr, i64 } (data, len).
    fn array_type(&self) -> StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.ptr_type(AddressSpace::default()).into(),
                self.context.i64_type().into(),
            ],
            false,
        )
    }

    // ── Scope ───────────────────────────────────────────────────────────

    fn push_scope(&mut self) {
        self.variables.push(HashMap::new());
        self.var_struct_names.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.variables.pop();
        self.var_struct_names.pop();
    }

    fn define_var(&mut self, name: String, ptr: PointerValue<'ctx>, ty: BasicTypeEnum<'ctx>) {
        self.variables.last_mut().unwrap().insert(name, (ptr, ty));
    }

    fn lookup_var(&self, name: &str) -> Option<(PointerValue<'ctx>, BasicTypeEnum<'ctx>)> {
        for scope in self.variables.iter().rev() {
            if let Some(entry) = scope.get(name) {
                return Some(*entry);
            }
        }
        None
    }

    // ── Structs ─────────────────────────────────────────────────────────

    fn register_struct(&mut self, s: &HirStructDef) -> Result<(), CodegenError> {
        let field_types: Vec<BasicTypeEnum<'ctx>> = s
            .fields
            .iter()
            .map(|f| self.convert_type(&f.ty))
            .collect::<Result<Vec<_>, _>>()?;
        let field_names: Vec<String> = s.fields.iter().map(|f| f.name.clone()).collect();

        let struct_type = self.context.struct_type(&field_types, false);
        self.struct_types
            .insert(s.name.clone(), (struct_type, field_names));
        Ok(())
    }

    fn get_struct_field_index(&self, struct_name: &str, field_name: &str) -> Option<u32> {
        self.struct_types.get(struct_name).and_then(|(_, fields)| {
            fields
                .iter()
                .position(|f| f == field_name)
                .map(|i| i as u32)
        })
    }

    // ── Functions ───────────────────────────────────────────────────────

    fn declare_function(&mut self, func: &HirFunction) -> Result<(), CodegenError> {
        let (fn_type, _) = self.build_fn_type(func)?;
        let llvm_fn = self.module.add_function(&func.name, fn_type, None);
        self.functions.insert(func.name.clone(), llvm_fn);
        Ok(())
    }

    fn declare_method(&mut self, target: &str, method: &HirFunction) -> Result<(), CodegenError> {
        let mangled = format!("{}.{}", target, method.name);
        let (fn_type, _) = self.build_fn_type_for_method(target, method)?;
        let llvm_fn = self.module.add_function(&mangled, fn_type, None);
        self.method_functions.insert(mangled, llvm_fn);
        Ok(())
    }

    fn build_fn_type(
        &self,
        func: &HirFunction,
    ) -> Result<(inkwell::types::FunctionType<'ctx>, Vec<BasicTypeEnum<'ctx>>), CodegenError> {
        let mut ret_type = func
            .return_type
            .as_ref()
            .map(|t| self.convert_type(t))
            .transpose()?;

        // C's main() must return i32.
        if func.name == "main" && ret_type.is_none() {
            ret_type = Some(self.context.i32_type().into());
        }

        let param_types: Vec<BasicTypeEnum<'ctx>> = func
            .params
            .iter()
            .map(|p| self.convert_type(&p.ty))
            .collect::<Result<Vec<_>, _>>()?;

        let meta_params: Vec<BasicMetadataTypeEnum<'ctx>> =
            param_types.iter().map(|t| (*t).into()).collect();

        let fn_type = match ret_type {
            Some(ty) => ty.fn_type(&meta_params, false),
            None => self.context.void_type().fn_type(&meta_params, false),
        };

        Ok((fn_type, param_types))
    }

    fn build_fn_type_for_method(
        &self,
        target: &str,
        method: &HirFunction,
    ) -> Result<(inkwell::types::FunctionType<'ctx>, Vec<BasicTypeEnum<'ctx>>), CodegenError> {
        let ret_type = method
            .return_type
            .as_ref()
            .map(|t| self.convert_type_with_self(t, target))
            .transpose()?;

        let param_types: Vec<BasicTypeEnum<'ctx>> = method
            .params
            .iter()
            .map(|p| self.convert_type_with_self(&p.ty, target))
            .collect::<Result<Vec<_>, _>>()?;

        let meta_params: Vec<BasicMetadataTypeEnum<'ctx>> =
            param_types.iter().map(|t| (*t).into()).collect();

        let fn_type = match ret_type {
            Some(ty) => ty.fn_type(&meta_params, false),
            None => self.context.void_type().fn_type(&meta_params, false),
        };

        Ok((fn_type, param_types))
    }

    fn compile_function(
        &mut self,
        func: &HirFunction,
        impl_target: Option<&str>,
    ) -> Result<(), CodegenError> {
        self.current_impl_target = impl_target.map(|s| s.to_string());
        let fn_name = match impl_target {
            Some(target) => format!("{}.{}", target, func.name),
            None => func.name.clone(),
        };

        let llvm_fn = self
            .functions
            .get(&fn_name)
            .or_else(|| self.method_functions.get(&fn_name))
            .copied()
            .ok_or_else(|| CodegenError(format!("Function '{}' not declared", fn_name)))?;

        let entry = self.context.append_basic_block(llvm_fn, "entry");
        self.builder.position_at_end(entry);
        self.push_scope();

        for (i, param) in func.params.iter().enumerate() {
            let param_val = llvm_fn.get_nth_param(i as u32).unwrap();
            let alloca = self.create_alloca(&param.name, param_val.get_type(), llvm_fn);
            self.builder.build_store(alloca, param_val).unwrap();
            self.define_var(param.name.clone(), alloca, param_val.get_type());
        }

        let result = self.compile_block(&func.body, llvm_fn)?;

        // Don't add a return if the block already terminated (e.g., explicit return).
        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            if let Some(ret_type) = &func.return_type {
                if let Some(val) = result {
                    self.builder.build_return(Some(&val)).unwrap();
                } else {
                    let ret_ty = match impl_target {
                        Some(t) => self.convert_type_with_self(ret_type, t)?,
                        None => self.convert_type(ret_type)?,
                    };
                    self.builder
                        .build_return(Some(&ret_ty.const_zero()))
                        .unwrap();
                }
            } else if func.name == "main" {
                // main() returns 0 to the OS.
                self.builder
                    .build_return(Some(&self.context.i32_type().const_int(0, false)))
                    .unwrap();
            } else {
                self.builder.build_return(None).unwrap();
            }
        }

        self.pop_scope();
        Ok(())
    }

    // ── Blocks ──────────────────────────────────────────────────────────

    fn compile_block(
        &mut self,
        block: &HirBlock,
        function: FunctionValue<'ctx>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let mut last_val = None;

        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i == block.stmts.len() - 1;
            // Stop if previous statement added a terminator.
            if self
                .builder
                .get_insert_block()
                .unwrap()
                .get_terminator()
                .is_some()
            {
                break;
            }
            match &stmt.kind {
                HirStmtKind::Let { name, value, .. } => {
                    if let Some(init_expr) = value {
                        let val = self.compile_expr(init_expr, function)?;
                        let alloca = self.create_alloca(name, val.get_type(), function);
                        self.builder.build_store(alloca, val).unwrap();
                        self.define_var(name.clone(), alloca, val.get_type());
                        // Track struct type name for method dispatch.
                        if let Some(sname) = self.infer_struct_name_from_expr(init_expr) {
                            self.var_struct_names
                                .last_mut()
                                .unwrap()
                                .insert(name.clone(), sname);
                        }
                    }
                    last_val = None;
                }
                HirStmtKind::Expr(expr) => {
                    let val = self.compile_expr(expr, function)?;
                    if is_last {
                        last_val = Some(val);
                    }
                }
                HirStmtKind::Return(expr) => {
                    if let Some(e) = expr {
                        let val = self.compile_expr(e, function)?;
                        self.builder.build_return(Some(&val)).unwrap();
                    } else {
                        self.builder.build_return(None).unwrap();
                    }
                    return Ok(None);
                }
                HirStmtKind::Break | HirStmtKind::Continue => {
                    last_val = None;
                }
                HirStmtKind::Defer(_) => { /* TODO: defer codegen */ }
            }
        }

        Ok(last_val)
    }

    // ── Expressions ─────────────────────────────────────────────────────

    fn compile_expr(
        &mut self,
        expr: &HirExpr,
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        match &expr.kind {
            HirExprKind::IntLiteral(n) => {
                Ok(self.context.i64_type().const_int(*n as u64, true).into())
            }
            HirExprKind::FloatLiteral(f) => Ok(self.context.f64_type().const_float(*f).into()),
            HirExprKind::BoolLiteral(b) => {
                Ok(self.context.bool_type().const_int(*b as u64, false).into())
            }
            HirExprKind::StringLiteral(s) => {
                let global = self.builder.build_global_string_ptr(s, "str").unwrap();
                Ok(global.as_pointer_value().into())
            }

            HirExprKind::StringConcat(parts) => {
                for part in parts {
                    let val = self.compile_expr(part, function)?;
                    self.emit_print_value(val)?;
                }
                let global = self
                    .builder
                    .build_global_string_ptr("", "empty_str")
                    .unwrap();
                Ok(global.as_pointer_value().into())
            }

            HirExprKind::Identifier(name) => {
                if let Some((ptr, ty)) = self.lookup_var(name) {
                    Ok(self.builder.build_load(ty, ptr, name).unwrap())
                } else {
                    Err(CodegenError(format!("Undefined variable: {name}")))
                }
            }

            HirExprKind::SelfValue => {
                if let Some((ptr, ty)) = self.lookup_var("self") {
                    Ok(self.builder.build_load(ty, ptr, "self").unwrap())
                } else {
                    Err(CodegenError("'self' outside method".into()))
                }
            }

            HirExprKind::BinaryOp { left, op, right } => {
                // Check for struct operator overloading first.
                if let Some(struct_name) = self.infer_struct_name(left) {
                    let method = match op {
                        BinOp::Add => "add",
                        BinOp::Sub => "sub",
                        BinOp::Mul => "mul",
                        BinOp::Div => "div",
                        _ => "",
                    };
                    if !method.is_empty() {
                        let mangled = format!("{struct_name}.{method}");
                        if let Some(&llvm_fn) = self.method_functions.get(&mangled) {
                            let lhs_val = self.compile_expr(left, function)?;
                            let rhs_val = self.compile_expr(right, function)?;
                            let result = self
                                .builder
                                .build_call(llvm_fn, &[lhs_val.into(), rhs_val.into()], "op_call")
                                .unwrap();
                            return match result.try_as_basic_value() {
                                inkwell::values::ValueKind::Basic(val) => Ok(val),
                                _ => Ok(self.context.i64_type().const_int(0, false).into()),
                            };
                        }
                    }
                }
                let lhs = self.compile_expr(left, function)?;
                let rhs = self.compile_expr(right, function)?;
                self.compile_binop(lhs, *op, rhs)
            }

            HirExprKind::UnaryOp { op, expr: inner } => {
                let val = self.compile_expr(inner, function)?;
                match op {
                    UnaryOp::Neg => {
                        if val.is_int_value() {
                            Ok(self
                                .builder
                                .build_int_neg(val.into_int_value(), "neg")
                                .unwrap()
                                .into())
                        } else {
                            Ok(self
                                .builder
                                .build_float_neg(val.into_float_value(), "fneg")
                                .unwrap()
                                .into())
                        }
                    }
                    UnaryOp::Not => Ok(self
                        .builder
                        .build_not(val.into_int_value(), "not")
                        .unwrap()
                        .into()),
                    _ => Ok(val),
                }
            }

            HirExprKind::Assign { target, value } => {
                let val = self.compile_expr(value, function)?;
                if let HirExprKind::Identifier(name) = &target.kind
                    && let Some((ptr, _)) = self.lookup_var(name)
                {
                    self.builder.build_store(ptr, val).unwrap();
                }
                Ok(val)
            }

            HirExprKind::Call { callee, args } => self.compile_call(callee, args, function),

            HirExprKind::FieldAccess { object, field } => {
                self.compile_field_access(object, field, function)
            }

            HirExprKind::If {
                condition,
                then_block,
                else_block,
            } => self.compile_if(condition, then_block, else_block.as_deref(), function),

            HirExprKind::While { condition, body } => self.compile_while(condition, body, function),

            HirExprKind::For {
                binding,
                iter,
                body,
            } => self.compile_for(binding, iter, body, function),

            HirExprKind::Block(block) => {
                self.push_scope();
                let val = self.compile_block(block, function)?;
                self.pop_scope();
                Ok(val.unwrap_or_else(|| self.context.i64_type().const_int(0, false).into()))
            }

            HirExprKind::StructLiteral { name, fields } => {
                self.compile_struct_literal(name, fields, function)
            }

            HirExprKind::Index { object, index } => {
                let arr = self.compile_expr(object, function)?;
                let idx = self.compile_expr(index, function)?;
                // If it's an array struct { ptr, i64 }, extract data pointer and index
                if arr.get_type() == self.array_type().into() {
                    let arr_alloca = self
                        .builder
                        .build_alloca(self.array_type(), "idx_arr")
                        .unwrap();
                    self.builder.build_store(arr_alloca, arr).unwrap();
                    let data_ptr = self
                        .builder
                        .build_struct_gep(self.array_type(), arr_alloca, 0, "data_ptr")
                        .unwrap();
                    let data = self
                        .builder
                        .build_load(
                            self.context.ptr_type(AddressSpace::default()),
                            data_ptr,
                            "data",
                        )
                        .unwrap();
                    let elem_ptr = unsafe {
                        self.builder
                            .build_gep(
                                self.context.i64_type(),
                                data.into_pointer_value(),
                                &[idx.into_int_value()],
                                "elem",
                            )
                            .unwrap()
                    };
                    let val = self
                        .builder
                        .build_load(self.context.i64_type(), elem_ptr, "elem_val")
                        .unwrap();
                    Ok(val)
                } else {
                    Ok(self.context.i64_type().const_int(0, false).into())
                }
            }

            HirExprKind::Slice { object, start, end } => {
                // Slice — stub: just return the compiled object value for now.
                let _ = (start, end);
                self.compile_expr(object, function)
            }

            HirExprKind::Array(elements) => {
                let arr_type = self.array_type();
                if elements.is_empty() {
                    // Empty array: { null, 0 }
                    let null_ptr = self.context.ptr_type(AddressSpace::default()).const_null();
                    let zero = self.context.i64_type().const_int(0, false);
                    let arr = arr_type.const_named_struct(&[null_ptr.into(), zero.into()]);
                    return Ok(arr.into());
                }
                // Non-empty array: malloc + store elements
                let len = elements.len();
                let total_size = self.context.i64_type().const_int((len * 8) as u64, false);
                // Call malloc
                let malloc_type = self
                    .context
                    .ptr_type(AddressSpace::default())
                    .fn_type(&[self.context.i64_type().into()], false);
                let malloc_fn = self
                    .module
                    .get_function("malloc")
                    .unwrap_or_else(|| self.module.add_function("malloc", malloc_type, None));
                let data_ptr = match self
                    .builder
                    .build_call(malloc_fn, &[total_size.into()], "arr_data")
                    .unwrap()
                    .try_as_basic_value()
                {
                    inkwell::values::ValueKind::Basic(val) => val,
                    _ => self
                        .context
                        .ptr_type(AddressSpace::default())
                        .const_null()
                        .into(),
                };
                // Store each element
                for (i, elem) in elements.iter().enumerate() {
                    let val = self.compile_expr(elem, function)?;
                    let offset = self.context.i64_type().const_int(i as u64, false);
                    let elem_ptr = unsafe {
                        self.builder
                            .build_gep(
                                self.context.i64_type(),
                                data_ptr.into_pointer_value(),
                                &[offset],
                                "elem_ptr",
                            )
                            .unwrap()
                    };
                    self.builder.build_store(elem_ptr, val).unwrap();
                }
                // Build { ptr, len } struct
                let len_val = self.context.i64_type().const_int(len as u64, false);
                let arr_alloca = self.builder.build_alloca(arr_type, "arr").unwrap();
                let data_field = self
                    .builder
                    .build_struct_gep(arr_type, arr_alloca, 0, "arr.data")
                    .unwrap();
                self.builder.build_store(data_field, data_ptr).unwrap();
                let len_field = self
                    .builder
                    .build_struct_gep(arr_type, arr_alloca, 1, "arr.len")
                    .unwrap();
                self.builder.build_store(len_field, len_val).unwrap();
                let arr_val = self
                    .builder
                    .build_load(arr_type, arr_alloca, "arr_val")
                    .unwrap();
                Ok(arr_val)
            }

            HirExprKind::Range { start, end, .. } => {
                // Ranges in compiled code: just return the start value for now.
                if let Some(s) = start {
                    self.compile_expr(s, function)
                } else if let Some(e) = end {
                    self.compile_expr(e, function)
                } else {
                    Ok(self.context.i64_type().const_int(0, false).into())
                }
            }

            HirExprKind::Closure { params, body } => self.compile_closure(params, body, function),

            HirExprKind::Comptime(block) => {
                // Comptime blocks should be evaluated before codegen.
                // If we get here, just compile the block normally.
                self.push_scope();
                let val = self.compile_block(block, function)?;
                self.pop_scope();
                Ok(val.unwrap_or_else(|| self.context.i64_type().const_int(0, false).into()))
            }

            HirExprKind::Match {
                expr: scrutinee,
                arms,
            } => self.compile_match(scrutinee, arms, function),

            HirExprKind::Reference { expr: inner, .. }
            | HirExprKind::Dereference(inner)
            | HirExprKind::Try(inner)
            | HirExprKind::Turbofish { expr: inner, .. } => self.compile_expr(inner, function),
            HirExprKind::SafeNav { object, .. } => self.compile_expr(object, function),
            HirExprKind::NullCoalesce { expr, .. } => self.compile_expr(expr, function),
        }
    }

    // ── Structs ─────────────────────────────────────────────────────────

    fn compile_struct_literal(
        &mut self,
        name: &str,
        fields: &[HirFieldInit],
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let (struct_type, field_names) = self
            .struct_types
            .get(name)
            .ok_or_else(|| CodegenError(format!("Unknown struct: {name}")))?
            .clone();

        let alloca = self.create_alloca(name, struct_type.into(), function);

        for field_init in fields {
            let idx = field_names
                .iter()
                .position(|f| f == &field_init.name)
                .ok_or_else(|| {
                    CodegenError(format!("No field '{}' on struct {name}", field_init.name))
                })?;

            let val = self.compile_expr(&field_init.value, function)?;
            let field_ptr = self
                .builder
                .build_struct_gep(struct_type, alloca, idx as u32, &field_init.name)
                .unwrap();
            self.builder.build_store(field_ptr, val).unwrap();
        }

        let loaded = self
            .builder
            .build_load(struct_type, alloca, "struct_val")
            .unwrap();
        Ok(loaded)
    }

    fn compile_field_access(
        &mut self,
        object: &HirExpr,
        field: &str,
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let obj_val = self.compile_expr(object, function)?;

        // Determine struct name from the expression.
        let struct_name = self.infer_struct_name(object);

        if let Some(name) = struct_name
            && let Some((struct_type, _)) = self.struct_types.get(&name).cloned()
            && let Some(idx) = self.get_struct_field_index(&name, field)
        {
            // Store the struct value, then GEP into it.
            let alloca = self.create_alloca("tmp_struct", struct_type.into(), function);
            self.builder.build_store(alloca, obj_val).unwrap();
            let field_ptr = self
                .builder
                .build_struct_gep(struct_type, alloca, idx, field)
                .unwrap();
            let field_type = struct_type.get_field_type_at_index(idx).unwrap();
            let val = self
                .builder
                .build_load(field_type, field_ptr, field)
                .unwrap();
            return Ok(val);
        }

        Err(CodegenError(format!("Cannot access field '{field}'")))
    }

    /// Try to determine the struct type name from an expression.
    /// Infer the Forge struct name from an expression (for let bindings).
    fn infer_struct_name_from_expr(&self, expr: &HirExpr) -> Option<String> {
        match &expr.kind {
            HirExprKind::StructLiteral { name, .. } => Some(name.clone()),
            HirExprKind::Call { callee, .. } => {
                // Type.new() → Type
                if let HirExprKind::FieldAccess { object, .. } = &callee.kind
                    && let HirExprKind::Identifier(name) = &object.kind
                    && self.struct_types.contains_key(name.as_str())
                {
                    Some(name.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn infer_struct_name(&self, expr: &HirExpr) -> Option<String> {
        match &expr.kind {
            HirExprKind::SelfValue => {
                // Inside a method, self is the current impl target.
                self.current_impl_target.clone()
            }
            HirExprKind::Identifier(name) => {
                // First check the explicit struct name tracking.
                for scope in self.var_struct_names.iter().rev() {
                    if let Some(sname) = scope.get(name.as_str()) {
                        return Some(sname.clone());
                    }
                }
                // Fall back to LLVM type comparison.
                if let Some((_, ty)) = self.lookup_var(name)
                    && ty.is_struct_type()
                {
                    let st = ty.into_struct_type();
                    for (sname, (stype, _)) in &self.struct_types {
                        if *stype == st {
                            return Some(sname.clone());
                        }
                    }
                }
                None
            }
            HirExprKind::Call { callee, .. } => {
                // For Type.method() calls, the type is the first part of the callee.
                if let HirExprKind::FieldAccess { object, .. } = &callee.kind {
                    self.infer_struct_name(object)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    // ── Calls ───────────────────────────────────────────────────────────

    fn compile_call(
        &mut self,
        callee: &HirExpr,
        args: &[HirExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        // print() — special case.
        if let HirExprKind::Identifier(name) = &callee.kind {
            if name == "print" {
                return self.compile_print(args, function);
            }
            if let Some(&llvm_fn) = self.functions.get(name.as_str()) {
                return self.compile_direct_call(llvm_fn, args, function);
            }

            // Try indirect call — callee is a variable holding a function pointer.
            if let Some((ptr, _ty)) = self.lookup_var(name) {
                let fn_ptr = self
                    .builder
                    .build_load(
                        self.context.ptr_type(AddressSpace::default()),
                        ptr,
                        "fn_ptr",
                    )
                    .unwrap();

                let mut compiled_args: Vec<BasicMetadataValueEnum> = Vec::new();
                for arg in args {
                    compiled_args.push(self.compile_expr(arg, function)?.into());
                }

                // Build function type for the indirect call (all i64 for now).
                let param_types: Vec<BasicMetadataTypeEnum> = compiled_args
                    .iter()
                    .map(|_| self.context.i64_type().into())
                    .collect();
                let fn_type = self.context.i64_type().fn_type(&param_types, false);

                let result = self
                    .builder
                    .build_indirect_call(
                        fn_type,
                        fn_ptr.into_pointer_value(),
                        &compiled_args,
                        "icall",
                    )
                    .unwrap();
                return match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(val) => Ok(val),
                    _ => Ok(self.context.i64_type().const_int(0, false).into()),
                };
            }
        }

        // Method call: expr.method(args) — callee is FieldAccess.
        if let HirExprKind::FieldAccess { object, field } = &callee.kind {
            return self.compile_method_call(object, field, args, function);
        }

        Err(CodegenError(format!(
            "Unsupported call target: {:?}",
            callee.kind
        )))
    }

    fn compile_direct_call(
        &mut self,
        llvm_fn: FunctionValue<'ctx>,
        args: &[HirExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let mut compiled_args: Vec<BasicMetadataValueEnum> = Vec::new();
        for arg in args {
            compiled_args.push(self.compile_expr(arg, function)?.into());
        }

        let result = self
            .builder
            .build_call(llvm_fn, &compiled_args, "call")
            .unwrap();

        match result.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(val) => Ok(val),
            _ => Ok(self.context.i64_type().const_int(0, false).into()),
        }
    }

    fn compile_method_call(
        &mut self,
        object: &HirExpr,
        method: &str,
        args: &[HirExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        // Built-in method: array.len()
        if method == "len" {
            let obj = self.compile_expr(object, function)?;
            if obj.get_type() == self.array_type().into() {
                let arr_alloca = self
                    .builder
                    .build_alloca(self.array_type(), "len_arr")
                    .unwrap();
                self.builder.build_store(arr_alloca, obj).unwrap();
                let len_ptr = self
                    .builder
                    .build_struct_gep(self.array_type(), arr_alloca, 1, "len_ptr")
                    .unwrap();
                let len = self
                    .builder
                    .build_load(self.context.i64_type(), len_ptr, "len")
                    .unwrap();
                return Ok(len);
            }
        }

        // Built-in method: array.push(val) — returns new array with element appended
        if method == "push" && !args.is_empty() {
            let obj = self.compile_expr(object, function)?;
            if obj.get_type() == self.array_type().into() {
                let val = self.compile_expr(&args[0], function)?;
                let push_fn = self.get_or_declare_array_push();
                let result = self
                    .builder
                    .build_call(push_fn, &[obj.into(), val.into()], "pushed")
                    .unwrap();
                return match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => Ok(v),
                    _ => Ok(self.array_type().const_zero().into()),
                };
            }
        }

        // Built-in array methods: map, filter, fold, each
        if (method == "map" || method == "filter" || method == "fold" || method == "each")
            && !args.is_empty()
        {
            let obj = self.compile_expr(object, function)?;
            if obj.get_type() == self.array_type().into() {
                return self.compile_array_closure_method(obj, method, args, function);
            }
        }

        // Built-in method: float.sqrt()
        if method == "sqrt" {
            let obj = self.compile_expr(object, function)?;
            if obj.is_float_value() {
                let sqrt_type = self
                    .context
                    .f64_type()
                    .fn_type(&[self.context.f64_type().into()], false);
                let sqrt_fn = self
                    .module
                    .get_function("llvm.sqrt.f64")
                    .unwrap_or_else(|| self.module.add_function("llvm.sqrt.f64", sqrt_type, None));
                let result = self
                    .builder
                    .build_call(sqrt_fn, &[obj.into_float_value().into()], "sqrt")
                    .unwrap();
                return match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(val) => Ok(val),
                    _ => Ok(self.context.f64_type().const_float(0.0).into()),
                };
            }
        }

        // Determine struct type name.
        let struct_name = self.infer_struct_name(object);

        // Static call: Type.method(args) — object is the type name identifier.
        if let HirExprKind::Identifier(type_name) = &object.kind
            && self.struct_types.contains_key(type_name.as_str())
        {
            let mangled = format!("{type_name}.{method}");
            if let Some(&llvm_fn) = self.method_functions.get(&mangled) {
                return self.compile_direct_call(llvm_fn, args, function);
            }
        }

        // Instance call: value.method(args) — pass object as first arg.
        if let Some(name) = &struct_name {
            let mangled = format!("{name}.{method}");
            if let Some(&llvm_fn) = self.method_functions.get(&mangled) {
                let obj_val = self.compile_expr(object, function)?;
                let mut compiled_args: Vec<BasicMetadataValueEnum> = vec![obj_val.into()];
                for arg in args {
                    compiled_args.push(self.compile_expr(arg, function)?.into());
                }
                let result = self
                    .builder
                    .build_call(llvm_fn, &compiled_args, "mcall")
                    .unwrap();
                return match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(val) => Ok(val),
                    _ => Ok(self.context.i64_type().const_int(0, false).into()),
                };
            }
        }

        Err(CodegenError(format!(
            "No method '{method}' found{}",
            struct_name
                .map(|n| format!(" on type '{n}'"))
                .unwrap_or_default()
        )))
    }

    fn compile_print(
        &mut self,
        args: &[HirExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        if args.is_empty() {
            let fmt = self
                .builder
                .build_global_string_ptr("\n", "fmt_nl")
                .unwrap();
            self.builder
                .build_call(self.printf_fn, &[fmt.as_pointer_value().into()], "printf")
                .unwrap();
            return Ok(self.context.i64_type().const_int(0, false).into());
        }

        let val = self.compile_expr(&args[0], function)?;
        self.emit_print_value(val)?;

        let nl = self.builder.build_global_string_ptr("\n", "nl").unwrap();
        self.builder
            .build_call(self.printf_fn, &[nl.as_pointer_value().into()], "nl")
            .unwrap();

        Ok(self.context.i64_type().const_int(0, false).into())
    }

    fn emit_print_value(&self, val: BasicValueEnum<'ctx>) -> Result<(), CodegenError> {
        if val.is_int_value() {
            let int_val = val.into_int_value();
            if int_val.get_type().get_bit_width() == 1 {
                let fmt = self
                    .builder
                    .build_global_string_ptr("%d", "fmt_bool")
                    .unwrap();
                self.builder
                    .build_call(
                        self.printf_fn,
                        &[fmt.as_pointer_value().into(), int_val.into()],
                        "printf",
                    )
                    .unwrap();
            } else {
                let fmt = self
                    .builder
                    .build_global_string_ptr("%lld", "fmt_int")
                    .unwrap();
                self.builder
                    .build_call(
                        self.printf_fn,
                        &[fmt.as_pointer_value().into(), int_val.into()],
                        "printf",
                    )
                    .unwrap();
            }
        } else if val.is_float_value() {
            let fmt = self
                .builder
                .build_global_string_ptr("%g", "fmt_float")
                .unwrap();
            self.builder
                .build_call(
                    self.printf_fn,
                    &[fmt.as_pointer_value().into(), val.into_float_value().into()],
                    "printf",
                )
                .unwrap();
        } else if val.is_pointer_value() {
            let fmt = self
                .builder
                .build_global_string_ptr("%s", "fmt_str")
                .unwrap();
            self.builder
                .build_call(
                    self.printf_fn,
                    &[
                        fmt.as_pointer_value().into(),
                        val.into_pointer_value().into(),
                    ],
                    "printf",
                )
                .unwrap();
        }
        Ok(())
    }

    // ── Binary operators ────────────────────────────────────────────────

    fn compile_binop(
        &self,
        lhs: BasicValueEnum<'ctx>,
        op: BinOp,
        rhs: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        if lhs.is_float_value() && rhs.is_float_value() {
            let l = lhs.into_float_value();
            let r = rhs.into_float_value();
            return match op {
                BinOp::Add => Ok(self.builder.build_float_add(l, r, "fadd").unwrap().into()),
                BinOp::Sub => Ok(self.builder.build_float_sub(l, r, "fsub").unwrap().into()),
                BinOp::Mul => Ok(self.builder.build_float_mul(l, r, "fmul").unwrap().into()),
                BinOp::Div => Ok(self.builder.build_float_div(l, r, "fdiv").unwrap().into()),
                BinOp::Mod => Ok(self.builder.build_float_rem(l, r, "fmod").unwrap().into()),
                BinOp::Lt => Ok(self
                    .builder
                    .build_float_compare(FloatPredicate::OLT, l, r, "flt")
                    .unwrap()
                    .into()),
                BinOp::Gt => Ok(self
                    .builder
                    .build_float_compare(FloatPredicate::OGT, l, r, "fgt")
                    .unwrap()
                    .into()),
                BinOp::LtEq => Ok(self
                    .builder
                    .build_float_compare(FloatPredicate::OLE, l, r, "fle")
                    .unwrap()
                    .into()),
                BinOp::GtEq => Ok(self
                    .builder
                    .build_float_compare(FloatPredicate::OGE, l, r, "fge")
                    .unwrap()
                    .into()),
                BinOp::Eq => Ok(self
                    .builder
                    .build_float_compare(FloatPredicate::OEQ, l, r, "feq")
                    .unwrap()
                    .into()),
                BinOp::NotEq => Ok(self
                    .builder
                    .build_float_compare(FloatPredicate::ONE, l, r, "fne")
                    .unwrap()
                    .into()),
                _ => Err(CodegenError(format!("Unsupported float op: {op:?}"))),
            };
        }

        if lhs.is_int_value() && rhs.is_int_value() {
            let l = lhs.into_int_value();
            let r = rhs.into_int_value();
            return match op {
                BinOp::Add => Ok(self.builder.build_int_add(l, r, "add").unwrap().into()),
                BinOp::Sub => Ok(self.builder.build_int_sub(l, r, "sub").unwrap().into()),
                BinOp::Mul => Ok(self.builder.build_int_mul(l, r, "mul").unwrap().into()),
                BinOp::Div => Ok(self
                    .builder
                    .build_int_signed_div(l, r, "div")
                    .unwrap()
                    .into()),
                BinOp::Mod => Ok(self
                    .builder
                    .build_int_signed_rem(l, r, "mod")
                    .unwrap()
                    .into()),
                BinOp::Lt => Ok(self
                    .builder
                    .build_int_compare(IntPredicate::SLT, l, r, "lt")
                    .unwrap()
                    .into()),
                BinOp::Gt => Ok(self
                    .builder
                    .build_int_compare(IntPredicate::SGT, l, r, "gt")
                    .unwrap()
                    .into()),
                BinOp::LtEq => Ok(self
                    .builder
                    .build_int_compare(IntPredicate::SLE, l, r, "le")
                    .unwrap()
                    .into()),
                BinOp::GtEq => Ok(self
                    .builder
                    .build_int_compare(IntPredicate::SGE, l, r, "ge")
                    .unwrap()
                    .into()),
                BinOp::Eq => Ok(self
                    .builder
                    .build_int_compare(IntPredicate::EQ, l, r, "eq")
                    .unwrap()
                    .into()),
                BinOp::NotEq => Ok(self
                    .builder
                    .build_int_compare(IntPredicate::NE, l, r, "ne")
                    .unwrap()
                    .into()),
                BinOp::And => Ok(self.builder.build_and(l, r, "and").unwrap().into()),
                BinOp::Or => Ok(self.builder.build_or(l, r, "or").unwrap().into()),
            };
        }

        // String operations: both operands are pointers (char*)
        if lhs.is_pointer_value() && rhs.is_pointer_value() {
            let l = lhs.into_pointer_value();
            let r = rhs.into_pointer_value();
            return match op {
                BinOp::Add => {
                    // String concatenation: call forge_str_concat(a, b)
                    let concat_fn = self.get_or_declare_str_concat();
                    let result = self
                        .builder
                        .build_call(concat_fn, &[l.into(), r.into()], "concat")
                        .unwrap();
                    match result.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(val) => Ok(val),
                        _ => Ok(self
                            .context
                            .ptr_type(AddressSpace::default())
                            .const_null()
                            .into()),
                    }
                }
                BinOp::Eq => {
                    // String equality: strcmp(a, b) == 0
                    let strcmp_fn = self.get_or_declare_strcmp();
                    let cmp = self
                        .builder
                        .build_call(strcmp_fn, &[l.into(), r.into()], "strcmp")
                        .unwrap()
                        .try_as_basic_value();
                    let cmp_val = match cmp {
                        inkwell::values::ValueKind::Basic(val) => val.into_int_value(),
                        _ => self.context.i32_type().const_int(1, false),
                    };
                    let zero = self.context.i32_type().const_int(0, false);
                    Ok(self
                        .builder
                        .build_int_compare(IntPredicate::EQ, cmp_val, zero, "streq")
                        .unwrap()
                        .into())
                }
                BinOp::NotEq => {
                    let strcmp_fn = self.get_or_declare_strcmp();
                    let cmp = self
                        .builder
                        .build_call(strcmp_fn, &[l.into(), r.into()], "strcmp")
                        .unwrap()
                        .try_as_basic_value();
                    let cmp_val = match cmp {
                        inkwell::values::ValueKind::Basic(val) => val.into_int_value(),
                        _ => self.context.i32_type().const_int(1, false),
                    };
                    let zero = self.context.i32_type().const_int(0, false);
                    Ok(self
                        .builder
                        .build_int_compare(IntPredicate::NE, cmp_val, zero, "strne")
                        .unwrap()
                        .into())
                }
                _ => Err(CodegenError(format!(
                    "Unsupported string operation: {:?}",
                    op
                ))),
            };
        }

        Err(CodegenError("Mismatched types in binary operation".into()))
    }

    fn get_or_declare_strcmp(&self) -> FunctionValue<'ctx> {
        let name = "strcmp";
        self.module.get_function(name).unwrap_or_else(|| {
            let fn_type = self.context.i32_type().fn_type(
                &[
                    self.context.ptr_type(AddressSpace::default()).into(),
                    self.context.ptr_type(AddressSpace::default()).into(),
                ],
                false,
            );
            self.module.add_function(name, fn_type, None)
        })
    }

    fn get_or_declare_str_concat(&self) -> FunctionValue<'ctx> {
        let name = "forge_str_concat";
        self.module.get_function(name).unwrap_or_else(|| {
            let fn_type = self.context.ptr_type(AddressSpace::default()).fn_type(
                &[
                    self.context.ptr_type(AddressSpace::default()).into(),
                    self.context.ptr_type(AddressSpace::default()).into(),
                ],
                false,
            );
            self.module.add_function(name, fn_type, None)
        })
    }

    fn get_or_declare_array_push(&self) -> FunctionValue<'ctx> {
        let name = "forge_array_push";
        self.module.get_function(name).unwrap_or_else(|| {
            let arr_type = self.array_type();
            let fn_type =
                arr_type.fn_type(&[arr_type.into(), self.context.i64_type().into()], false);
            self.module.add_function(name, fn_type, None)
        })
    }

    // ── Control flow ────────────────────────────────────────────────────

    fn compile_if(
        &mut self,
        condition: &HirExpr,
        then_block: &HirBlock,
        else_block: Option<&HirExpr>,
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let cond_val = self.compile_expr(condition, function)?;
        let cond_bool = cond_val.into_int_value();

        let then_bb = self.context.append_basic_block(function, "then");
        let else_bb = self.context.append_basic_block(function, "else");
        let merge_bb = self.context.append_basic_block(function, "merge");

        self.builder
            .build_conditional_branch(cond_bool, then_bb, else_bb)
            .unwrap();

        self.builder.position_at_end(then_bb);
        self.push_scope();
        let then_val = self.compile_block(then_block, function)?;
        self.pop_scope();
        let then_val =
            then_val.unwrap_or_else(|| self.context.i64_type().const_int(0, false).into());
        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            self.builder.build_unconditional_branch(merge_bb).unwrap();
        }
        let then_bb_end = self.builder.get_insert_block().unwrap();

        self.builder.position_at_end(else_bb);
        let else_val = if let Some(else_expr) = else_block {
            self.compile_expr(else_expr, function)?
        } else {
            self.context.i64_type().const_int(0, false).into()
        };
        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            self.builder.build_unconditional_branch(merge_bb).unwrap();
        }
        let else_bb_end = self.builder.get_insert_block().unwrap();

        self.builder.position_at_end(merge_bb);
        if then_val.get_type() == else_val.get_type() {
            let phi = self
                .builder
                .build_phi(then_val.get_type(), "if_result")
                .unwrap();
            phi.add_incoming(&[(&then_val, then_bb_end), (&else_val, else_bb_end)]);
            Ok(phi.as_basic_value())
        } else {
            Ok(then_val)
        }
    }

    fn compile_while(
        &mut self,
        condition: &HirExpr,
        body: &HirBlock,
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let cond_bb = self.context.append_basic_block(function, "while_cond");
        let body_bb = self.context.append_basic_block(function, "while_body");
        let exit_bb = self.context.append_basic_block(function, "while_exit");

        self.builder.build_unconditional_branch(cond_bb).unwrap();

        self.builder.position_at_end(cond_bb);
        let cond_val = self.compile_expr(condition, function)?;
        self.builder
            .build_conditional_branch(cond_val.into_int_value(), body_bb, exit_bb)
            .unwrap();

        self.builder.position_at_end(body_bb);
        self.push_scope();
        self.compile_block(body, function)?;
        self.pop_scope();
        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            self.builder.build_unconditional_branch(cond_bb).unwrap();
        }

        self.builder.position_at_end(exit_bb);
        Ok(self.context.i64_type().const_int(0, false).into())
    }

    fn compile_closure(
        &mut self,
        params: &[HirClosureParam],
        body: &HirExpr,
        parent_fn: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        // Generate a unique name for the closure function.
        use std::sync::atomic::{AtomicU64, Ordering};
        static CLOSURE_COUNT: AtomicU64 = AtomicU64::new(0);
        let id = CLOSURE_COUNT.fetch_add(1, Ordering::Relaxed);
        let closure_name = format!("__closure_{id}");

        // Save the current builder position.
        let saved_block = self.builder.get_insert_block();

        // Build the function type: all params are i64 for now.
        let param_types: Vec<BasicMetadataTypeEnum<'ctx>> = params
            .iter()
            .map(|_| self.context.i64_type().into())
            .collect();
        let fn_type = self.context.i64_type().fn_type(&param_types, false);
        let closure_fn = self.module.add_function(&closure_name, fn_type, None);

        // Build the closure body in a new function.
        let entry = self.context.append_basic_block(closure_fn, "entry");
        self.builder.position_at_end(entry);

        self.push_scope();
        for (i, param) in params.iter().enumerate() {
            let alloca = self
                .builder
                .build_alloca(self.context.i64_type(), &param.name)
                .unwrap();
            self.builder
                .build_store(alloca, closure_fn.get_nth_param(i as u32).unwrap())
                .unwrap();
            self.define_var(param.name.clone(), alloca, self.context.i64_type().into());
        }

        let result = self.compile_expr(body, closure_fn)?;
        self.builder.build_return(Some(&result)).unwrap();
        self.pop_scope();

        // Restore builder to the parent function.
        if let Some(block) = saved_block {
            self.builder.position_at_end(block);
        }

        // Return the function pointer.
        // Store it in a variable so it can be loaded as a ptr later.
        let _ = parent_fn; // suppress warning
        Ok(closure_fn.as_global_value().as_pointer_value().into())
    }

    fn compile_array_closure_method(
        &mut self,
        arr: BasicValueEnum<'ctx>,
        method: &str,
        args: &[HirExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i64_type = self.context.i64_type();
        let arr_type = self.array_type();
        let ptr_type = self.context.ptr_type(AddressSpace::default());

        // Extract data ptr and len from source array
        let src_alloca = self.builder.build_alloca(arr_type, "src_arr").unwrap();
        self.builder.build_store(src_alloca, arr).unwrap();
        let data_gep = self
            .builder
            .build_struct_gep(arr_type, src_alloca, 0, "data_gep")
            .unwrap();
        let data_ptr = self
            .builder
            .build_load(ptr_type, data_gep, "data")
            .unwrap()
            .into_pointer_value();
        let len_gep = self
            .builder
            .build_struct_gep(arr_type, src_alloca, 1, "len_gep")
            .unwrap();
        let len = self
            .builder
            .build_load(i64_type, len_gep, "len")
            .unwrap()
            .into_int_value();

        // Compile the closure argument
        let closure_arg_idx = if method == "fold" { 1 } else { 0 };
        let closure_val = self.compile_expr(&args[closure_arg_idx], function)?;
        let closure_ptr = closure_val.into_pointer_value();

        match method {
            "map" => {
                // Build new array, call closure for each element
                let push_fn = self.get_or_declare_array_push();
                let result = self.builder.build_alloca(arr_type, "map_result").unwrap();
                // Start with empty array
                let empty_data = ptr_type.const_null();
                let zero = i64_type.const_int(0, false);
                let data_field = self
                    .builder
                    .build_struct_gep(arr_type, result, 0, "rd")
                    .unwrap();
                self.builder.build_store(data_field, empty_data).unwrap();
                let len_field = self
                    .builder
                    .build_struct_gep(arr_type, result, 1, "rl")
                    .unwrap();
                self.builder.build_store(len_field, zero).unwrap();

                // Loop
                let idx = self.builder.build_alloca(i64_type, "idx").unwrap();
                self.builder.build_store(idx, zero).unwrap();
                let cond_bb = self.context.append_basic_block(function, "map_cond");
                let body_bb = self.context.append_basic_block(function, "map_body");
                let exit_bb = self.context.append_basic_block(function, "map_exit");
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(cond_bb);
                let cur = self
                    .builder
                    .build_load(i64_type, idx, "i")
                    .unwrap()
                    .into_int_value();
                let cond = self
                    .builder
                    .build_int_compare(IntPredicate::SLT, cur, len, "cmp")
                    .unwrap();
                self.builder
                    .build_conditional_branch(cond, body_bb, exit_bb)
                    .unwrap();

                self.builder.position_at_end(body_bb);
                let elem_ptr = unsafe {
                    self.builder
                        .build_gep(i64_type, data_ptr, &[cur], "ep")
                        .unwrap()
                };
                let elem = self.builder.build_load(i64_type, elem_ptr, "elem").unwrap();
                // Call closure(elem)
                let fn_type = i64_type.fn_type(&[i64_type.into()], false);
                let mapped = self
                    .builder
                    .build_indirect_call(fn_type, closure_ptr, &[elem.into()], "mapped")
                    .unwrap();
                let mapped_val = match mapped.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v,
                    _ => i64_type.const_int(0, false).into(),
                };
                // Push to result
                let cur_arr = self
                    .builder
                    .build_load(arr_type, result, "cur_arr")
                    .unwrap();
                let new_arr = self
                    .builder
                    .build_call(push_fn, &[cur_arr.into(), mapped_val.into()], "pushed")
                    .unwrap();
                let new_arr_val = match new_arr.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v,
                    _ => arr_type.const_zero().into(),
                };
                self.builder.build_store(result, new_arr_val).unwrap();
                // Increment
                let next = self
                    .builder
                    .build_int_add(cur, i64_type.const_int(1, false), "next")
                    .unwrap();
                self.builder.build_store(idx, next).unwrap();
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(exit_bb);
                Ok(self
                    .builder
                    .build_load(arr_type, result, "map_out")
                    .unwrap())
            }
            "filter" => {
                let push_fn = self.get_or_declare_array_push();
                let result = self.builder.build_alloca(arr_type, "filt_result").unwrap();
                let empty_data = ptr_type.const_null();
                let zero = i64_type.const_int(0, false);
                let data_field = self
                    .builder
                    .build_struct_gep(arr_type, result, 0, "rd")
                    .unwrap();
                self.builder.build_store(data_field, empty_data).unwrap();
                let len_field = self
                    .builder
                    .build_struct_gep(arr_type, result, 1, "rl")
                    .unwrap();
                self.builder.build_store(len_field, zero).unwrap();

                let idx = self.builder.build_alloca(i64_type, "idx").unwrap();
                self.builder.build_store(idx, zero).unwrap();
                let cond_bb = self.context.append_basic_block(function, "filt_cond");
                let body_bb = self.context.append_basic_block(function, "filt_body");
                let exit_bb = self.context.append_basic_block(function, "filt_exit");
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(cond_bb);
                let cur = self
                    .builder
                    .build_load(i64_type, idx, "i")
                    .unwrap()
                    .into_int_value();
                let cond = self
                    .builder
                    .build_int_compare(IntPredicate::SLT, cur, len, "cmp")
                    .unwrap();
                self.builder
                    .build_conditional_branch(cond, body_bb, exit_bb)
                    .unwrap();

                self.builder.position_at_end(body_bb);
                let elem_ptr = unsafe {
                    self.builder
                        .build_gep(i64_type, data_ptr, &[cur], "ep")
                        .unwrap()
                };
                let elem = self.builder.build_load(i64_type, elem_ptr, "elem").unwrap();
                // Call closure(elem) — returns bool (i1 or i64)
                let fn_type = i64_type.fn_type(&[i64_type.into()], false);
                let keep = self
                    .builder
                    .build_indirect_call(fn_type, closure_ptr, &[elem.into()], "keep")
                    .unwrap();
                let keep_val = match keep.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v.into_int_value(),
                    _ => i64_type.const_int(0, false),
                };
                // If truthy, push
                let push_bb = self.context.append_basic_block(function, "filt_push");
                let skip_bb = self.context.append_basic_block(function, "filt_skip");
                let is_true = self
                    .builder
                    .build_int_compare(
                        IntPredicate::NE,
                        keep_val,
                        i64_type.const_int(0, false),
                        "truthy",
                    )
                    .unwrap();
                self.builder
                    .build_conditional_branch(is_true, push_bb, skip_bb)
                    .unwrap();

                self.builder.position_at_end(push_bb);
                let cur_arr = self
                    .builder
                    .build_load(arr_type, result, "cur_arr")
                    .unwrap();
                let new_arr = self
                    .builder
                    .build_call(push_fn, &[cur_arr.into(), elem.into()], "pushed")
                    .unwrap();
                let new_arr_val = match new_arr.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v,
                    _ => arr_type.const_zero().into(),
                };
                self.builder.build_store(result, new_arr_val).unwrap();
                self.builder.build_unconditional_branch(skip_bb).unwrap();

                self.builder.position_at_end(skip_bb);
                let next = self
                    .builder
                    .build_int_add(cur, i64_type.const_int(1, false), "next")
                    .unwrap();
                self.builder.build_store(idx, next).unwrap();
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(exit_bb);
                Ok(self
                    .builder
                    .build_load(arr_type, result, "filt_out")
                    .unwrap())
            }
            "fold" => {
                // fold(init, |acc, x| body)
                let init = self.compile_expr(&args[0], function)?;
                let acc = self.builder.build_alloca(i64_type, "acc").unwrap();
                self.builder.build_store(acc, init).unwrap();

                let idx = self.builder.build_alloca(i64_type, "idx").unwrap();
                let zero = i64_type.const_int(0, false);
                self.builder.build_store(idx, zero).unwrap();
                let cond_bb = self.context.append_basic_block(function, "fold_cond");
                let body_bb = self.context.append_basic_block(function, "fold_body");
                let exit_bb = self.context.append_basic_block(function, "fold_exit");
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(cond_bb);
                let cur = self
                    .builder
                    .build_load(i64_type, idx, "i")
                    .unwrap()
                    .into_int_value();
                let cond = self
                    .builder
                    .build_int_compare(IntPredicate::SLT, cur, len, "cmp")
                    .unwrap();
                self.builder
                    .build_conditional_branch(cond, body_bb, exit_bb)
                    .unwrap();

                self.builder.position_at_end(body_bb);
                let elem_ptr = unsafe {
                    self.builder
                        .build_gep(i64_type, data_ptr, &[cur], "ep")
                        .unwrap()
                };
                let elem = self.builder.build_load(i64_type, elem_ptr, "elem").unwrap();
                let cur_acc = self.builder.build_load(i64_type, acc, "cur_acc").unwrap();
                // Call closure(acc, elem)
                let fn_type = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);
                let new_acc = self
                    .builder
                    .build_indirect_call(
                        fn_type,
                        closure_ptr,
                        &[cur_acc.into(), elem.into()],
                        "new_acc",
                    )
                    .unwrap();
                let new_acc_val = match new_acc.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v,
                    _ => i64_type.const_int(0, false).into(),
                };
                self.builder.build_store(acc, new_acc_val).unwrap();
                let next = self
                    .builder
                    .build_int_add(cur, i64_type.const_int(1, false), "next")
                    .unwrap();
                self.builder.build_store(idx, next).unwrap();
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(exit_bb);
                Ok(self.builder.build_load(i64_type, acc, "fold_out").unwrap())
            }
            "each" => {
                let idx = self.builder.build_alloca(i64_type, "idx").unwrap();
                let zero = i64_type.const_int(0, false);
                self.builder.build_store(idx, zero).unwrap();
                let cond_bb = self.context.append_basic_block(function, "each_cond");
                let body_bb = self.context.append_basic_block(function, "each_body");
                let exit_bb = self.context.append_basic_block(function, "each_exit");
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(cond_bb);
                let cur = self
                    .builder
                    .build_load(i64_type, idx, "i")
                    .unwrap()
                    .into_int_value();
                let cond = self
                    .builder
                    .build_int_compare(IntPredicate::SLT, cur, len, "cmp")
                    .unwrap();
                self.builder
                    .build_conditional_branch(cond, body_bb, exit_bb)
                    .unwrap();

                self.builder.position_at_end(body_bb);
                let elem_ptr = unsafe {
                    self.builder
                        .build_gep(i64_type, data_ptr, &[cur], "ep")
                        .unwrap()
                };
                let elem = self.builder.build_load(i64_type, elem_ptr, "elem").unwrap();
                let fn_type = i64_type.fn_type(&[i64_type.into()], false);
                self.builder
                    .build_indirect_call(fn_type, closure_ptr, &[elem.into()], "")
                    .unwrap();
                let next = self
                    .builder
                    .build_int_add(cur, i64_type.const_int(1, false), "next")
                    .unwrap();
                self.builder.build_store(idx, next).unwrap();
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(exit_bb);
                Ok(i64_type.const_int(0, false).into())
            }
            _ => Err(CodegenError(format!("Unknown array method: {method}"))),
        }
    }

    fn compile_for(
        &mut self,
        binding: &str,
        iter: &HirExpr,
        body: &HirBlock,
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        // For now, only support range-based for loops: `for x in start..end`
        if let HirExprKind::Range {
            start: Some(start_expr),
            end: Some(end_expr),
            ..
        } = &iter.kind
        {
            let start_val = self.compile_expr(start_expr, function)?;
            let end_val = self.compile_expr(end_expr, function)?;

            let i64_type = self.context.i64_type();
            let counter = self.create_alloca(binding, i64_type.into(), function);
            self.builder.build_store(counter, start_val).unwrap();

            let cond_bb = self.context.append_basic_block(function, "for_cond");
            let body_bb = self.context.append_basic_block(function, "for_body");
            let exit_bb = self.context.append_basic_block(function, "for_exit");

            self.builder.build_unconditional_branch(cond_bb).unwrap();

            self.builder.position_at_end(cond_bb);
            let current = self
                .builder
                .build_load(i64_type, counter, binding)
                .unwrap()
                .into_int_value();
            let cond = self
                .builder
                .build_int_compare(
                    IntPredicate::SLT,
                    current,
                    end_val.into_int_value(),
                    "for_cond",
                )
                .unwrap();
            self.builder
                .build_conditional_branch(cond, body_bb, exit_bb)
                .unwrap();

            self.builder.position_at_end(body_bb);
            self.push_scope();
            self.define_var(binding.to_string(), counter, i64_type.into());
            self.compile_block(body, function)?;
            self.pop_scope();

            // Increment counter.
            let next = self
                .builder
                .build_load(i64_type, counter, "next")
                .unwrap()
                .into_int_value();
            let incremented = self
                .builder
                .build_int_add(next, i64_type.const_int(1, false), "inc")
                .unwrap();
            self.builder.build_store(counter, incremented).unwrap();
            if self
                .builder
                .get_insert_block()
                .unwrap()
                .get_terminator()
                .is_none()
            {
                self.builder.build_unconditional_branch(cond_bb).unwrap();
            }

            self.builder.position_at_end(exit_bb);
            return Ok(self.context.i64_type().const_int(0, false).into());
        }

        // Array iteration: for x in arr { ... }
        let iter_val = self.compile_expr(iter, function)?;
        if iter_val.get_type() == self.array_type().into() {
            let i64_type = self.context.i64_type();
            let arr_type = self.array_type();
            let ptr_type = self.context.ptr_type(AddressSpace::default());

            // Store the array struct
            let arr_alloca = self.builder.build_alloca(arr_type, "iter_arr").unwrap();
            self.builder.build_store(arr_alloca, iter_val).unwrap();

            // Get data ptr and len
            let data_gep = self
                .builder
                .build_struct_gep(arr_type, arr_alloca, 0, "data_gep")
                .unwrap();
            let data_ptr = self
                .builder
                .build_load(ptr_type, data_gep, "data")
                .unwrap()
                .into_pointer_value();
            let len_gep = self
                .builder
                .build_struct_gep(arr_type, arr_alloca, 1, "len_gep")
                .unwrap();
            let len = self
                .builder
                .build_load(i64_type, len_gep, "len")
                .unwrap()
                .into_int_value();

            // Index counter
            let idx = self.create_alloca("__idx", i64_type.into(), function);
            self.builder
                .build_store(idx, i64_type.const_int(0, false))
                .unwrap();

            let cond_bb = self.context.append_basic_block(function, "for_cond");
            let body_bb = self.context.append_basic_block(function, "for_body");
            let exit_bb = self.context.append_basic_block(function, "for_exit");

            self.builder.build_unconditional_branch(cond_bb).unwrap();

            // Condition: __idx < len
            self.builder.position_at_end(cond_bb);
            let cur_idx = self
                .builder
                .build_load(i64_type, idx, "cur_idx")
                .unwrap()
                .into_int_value();
            let cond = self
                .builder
                .build_int_compare(IntPredicate::SLT, cur_idx, len, "arr_cond")
                .unwrap();
            self.builder
                .build_conditional_branch(cond, body_bb, exit_bb)
                .unwrap();

            // Body: binding = data[__idx]
            self.builder.position_at_end(body_bb);
            self.push_scope();

            let elem_ptr = unsafe {
                self.builder
                    .build_gep(i64_type, data_ptr, &[cur_idx], "elem_ptr")
                    .unwrap()
            };
            let elem = self.builder.build_load(i64_type, elem_ptr, "elem").unwrap();
            let binding_alloca = self.create_alloca(binding, i64_type.into(), function);
            self.builder.build_store(binding_alloca, elem).unwrap();
            self.define_var(binding.to_string(), binding_alloca, i64_type.into());

            self.compile_block(body, function)?;
            self.pop_scope();

            // Increment index
            let next_idx = self
                .builder
                .build_load(i64_type, idx, "next_idx")
                .unwrap()
                .into_int_value();
            let inc = self
                .builder
                .build_int_add(next_idx, i64_type.const_int(1, false), "inc")
                .unwrap();
            self.builder.build_store(idx, inc).unwrap();
            if self
                .builder
                .get_insert_block()
                .unwrap()
                .get_terminator()
                .is_none()
            {
                self.builder.build_unconditional_branch(cond_bb).unwrap();
            }

            self.builder.position_at_end(exit_bb);
            return Ok(i64_type.const_int(0, false).into());
        }

        // Fallback
        Ok(self.context.i64_type().const_int(0, false).into())
    }

    fn compile_match(
        &mut self,
        scrutinee: &HirExpr,
        arms: &[HirMatchArm],
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let val = self.compile_expr(scrutinee, function)?;
        let merge_bb = self.context.append_basic_block(function, "match_merge");

        // For integer match: chain of compare-and-branch.
        // For wildcard/identifier: unconditional fallthrough.
        let mut arm_results: Vec<(BasicValueEnum<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
            Vec::new();

        for (i, arm) in arms.iter().enumerate() {
            let is_last = i == arms.len() - 1;

            match &arm.pattern.kind {
                HirPatternKind::Literal(lit_expr) => {
                    let arm_bb = self.context.append_basic_block(function, "match_arm");
                    let next_bb = if is_last {
                        merge_bb
                    } else {
                        self.context.append_basic_block(function, "match_next")
                    };

                    // Compare scrutinee with literal.
                    let lit_val = self.compile_expr(lit_expr, function)?;
                    if val.is_int_value() && lit_val.is_int_value() {
                        let cmp = self
                            .builder
                            .build_int_compare(
                                IntPredicate::EQ,
                                val.into_int_value(),
                                lit_val.into_int_value(),
                                "match_cmp",
                            )
                            .unwrap();
                        self.builder
                            .build_conditional_branch(cmp, arm_bb, next_bb)
                            .unwrap();
                    } else {
                        // Non-integer: just branch to the arm (fallback).
                        self.builder.build_unconditional_branch(arm_bb).unwrap();
                    }

                    self.builder.position_at_end(arm_bb);
                    self.push_scope();
                    let result = self.compile_expr(&arm.body, function)?;
                    self.pop_scope();
                    let arm_end = self.builder.get_insert_block().unwrap();
                    if arm_end.get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb).unwrap();
                    }
                    arm_results.push((result, arm_end));

                    if !is_last {
                        self.builder.position_at_end(next_bb);
                    }
                }
                HirPatternKind::Wildcard => {
                    let arm_bb = self.context.append_basic_block(function, "match_wild");
                    self.builder.build_unconditional_branch(arm_bb).unwrap();
                    self.builder.position_at_end(arm_bb);
                    self.push_scope();
                    let result = self.compile_expr(&arm.body, function)?;
                    self.pop_scope();
                    let arm_end = self.builder.get_insert_block().unwrap();
                    if arm_end.get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb).unwrap();
                    }
                    arm_results.push((result, arm_end));
                }
                HirPatternKind::Identifier(name) => {
                    let arm_bb = self.context.append_basic_block(function, "match_bind");
                    self.builder.build_unconditional_branch(arm_bb).unwrap();
                    self.builder.position_at_end(arm_bb);
                    self.push_scope();
                    let alloca = self.create_alloca(name, val.get_type(), function);
                    self.builder.build_store(alloca, val).unwrap();
                    self.define_var(name.clone(), alloca, val.get_type());
                    let result = self.compile_expr(&arm.body, function)?;
                    self.pop_scope();
                    let arm_end = self.builder.get_insert_block().unwrap();
                    if arm_end.get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb).unwrap();
                    }
                    arm_results.push((result, arm_end));
                }
                HirPatternKind::Variant { .. } => {
                    // Variant patterns in codegen: for now, treat as wildcard.
                    let arm_bb = self.context.append_basic_block(function, "match_variant");
                    self.builder.build_unconditional_branch(arm_bb).unwrap();
                    self.builder.position_at_end(arm_bb);
                    self.push_scope();
                    let result = self.compile_expr(&arm.body, function)?;
                    self.pop_scope();
                    let arm_end = self.builder.get_insert_block().unwrap();
                    if arm_end.get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb).unwrap();
                    }
                    arm_results.push((result, arm_end));
                }
            }
        }

        self.builder.position_at_end(merge_bb);

        // Build phi node if we have results.
        if let Some((first_val, _)) = arm_results.first()
            && arm_results
                .iter()
                .all(|(v, _)| v.get_type() == first_val.get_type())
        {
            let phi = self
                .builder
                .build_phi(first_val.get_type(), "match_result")
                .unwrap();
            for (val, bb) in &arm_results {
                phi.add_incoming(&[(val, *bb)]);
            }
            return Ok(phi.as_basic_value());
        }

        Ok(self.context.i64_type().const_int(0, false).into())
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn create_alloca(
        &self,
        name: &str,
        ty: BasicTypeEnum<'ctx>,
        function: FunctionValue<'ctx>,
    ) -> PointerValue<'ctx> {
        let entry = function.get_first_basic_block().unwrap();
        let builder = self.context.create_builder();
        match entry.get_first_instruction() {
            Some(inst) => builder.position_before(&inst),
            None => builder.position_at_end(entry),
        }
        builder.build_alloca(ty, name).unwrap()
    }

    fn convert_type(&self, ty: &HirType) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
        self.convert_type_with_self(ty, "")
    }

    fn convert_type_with_self(
        &self,
        ty: &HirType,
        self_type: &str,
    ) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
        match &ty.kind {
            HirTypeKind::Named(name) => {
                let name = if name == "Self" && !self_type.is_empty() {
                    self_type
                } else {
                    name.as_str()
                };
                match name {
                    "i8" | "u8" => Ok(self.context.i8_type().into()),
                    "i16" | "u16" => Ok(self.context.i16_type().into()),
                    "i32" | "u32" => Ok(self.context.i32_type().into()),
                    "i64" | "u64" | "isize" | "usize" => Ok(self.context.i64_type().into()),
                    "i128" | "u128" => Ok(self.context.i128_type().into()),
                    "f32" => Ok(self.context.f32_type().into()),
                    "f64" => Ok(self.context.f64_type().into()),
                    "bool" => Ok(self.context.bool_type().into()),
                    "str" => Ok(self.context.ptr_type(AddressSpace::default()).into()),
                    _ => {
                        // User-defined struct type.
                        if let Some((struct_type, _)) = self.struct_types.get(name) {
                            Ok((*struct_type).into())
                        } else {
                            Ok(self.context.i64_type().into())
                        }
                    }
                }
            }
            HirTypeKind::Reference { .. } => {
                Ok(self.context.ptr_type(AddressSpace::default()).into())
            }
            HirTypeKind::Array { .. } => Ok(self.array_type().into()),
            _ => Ok(self.context.i64_type().into()),
        }
    }
}

/// Compile a program to an object file.
pub fn compile_to_object(program: &HirProgram, output: &Path) -> Result<(), CodegenError> {
    let context = Context::create();
    let mut codegen = Codegen::new(&context, "forge_module");
    codegen.compile_program(program)?;
    codegen.write_object(output)
}

/// Compile and link to a binary.
pub fn compile_to_binary(program: &HirProgram, output: &Path) -> Result<(), CodegenError> {
    let obj_path = output.with_extension("o");
    compile_to_object(program, &obj_path)?;

    let status = std::process::Command::new("cc")
        .arg(&obj_path)
        .arg("-o")
        .arg(output)
        .arg("-lm")
        .status()
        .map_err(|e| CodegenError(format!("Failed to run linker: {e}")))?;

    let _ = std::fs::remove_file(&obj_path);

    if !status.success() {
        return Err(CodegenError("Linking failed".into()));
    }

    Ok(())
}
