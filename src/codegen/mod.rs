//! LLVM code generation for Forge.
//!
//! Lowers the HIR into LLVM IR using inkwell, then compiles to a native binary.
//! Currently supports a minimal subset: functions, integers, floats, booleans,
//! arithmetic, comparisons, let bindings, if/else, print, and string literals.

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
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, PointerValue};

use crate::hir::*;

/// Code generation errors.
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
    /// Variable name → (LLVM alloca pointer, pointee type).
    variables: Vec<HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>>,
    /// Function name → LLVM function value.
    functions: HashMap<String, FunctionValue<'ctx>>,
    /// Printf function (for print()).
    printf_fn: FunctionValue<'ctx>,
}

impl<'ctx> Codegen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();

        // Declare printf for print() support.
        let printf_type = context.i32_type().fn_type(
            &[context.ptr_type(AddressSpace::default()).into()],
            true, // variadic
        );
        let printf_fn = module.add_function("printf", printf_type, None);

        Self {
            context,
            module,
            builder,
            variables: vec![HashMap::new()],
            functions: HashMap::new(),
            printf_fn,
        }
    }

    /// Generate LLVM IR for a program.
    pub fn compile_program(&mut self, program: &HirProgram) -> Result<(), CodegenError> {
        // First pass: declare all functions.
        for item in &program.items {
            if let HirItemKind::Function(func) = &item.kind {
                self.declare_function(func)?;
            }
        }

        // Second pass: generate function bodies.
        for item in &program.items {
            if let HirItemKind::Function(func) = &item.kind {
                self.compile_function(func)?;
            }
        }

        Ok(())
    }

    /// Write LLVM IR to a file (for debugging).
    pub fn write_ir(&self, path: &Path) -> Result<(), CodegenError> {
        self.module
            .print_to_file(path)
            .map_err(|e| CodegenError(format!("Failed to write IR: {}", e.to_string())))
    }

    /// Compile to an object file.
    pub fn write_object(&self, path: &Path) -> Result<(), CodegenError> {
        Target::initialize_native(&InitializationConfig::default())
            .map_err(|e| CodegenError(format!("Failed to initialize target: {e}")))?;

        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple)
            .map_err(|e| CodegenError(format!("Failed to get target: {}", e.to_string())))?;

        let machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                OptimizationLevel::Default,
                RelocMode::PIC,
                CodeModel::Default,
            )
            .ok_or_else(|| CodegenError("Failed to create target machine".into()))?;

        machine
            .write_to_file(&self.module, FileType::Object, path)
            .map_err(|e| CodegenError(format!("Failed to write object: {}", e.to_string())))
    }

    /// Get the LLVM IR as a string (for testing).
    pub fn get_ir(&self) -> String {
        self.module.print_to_string().to_string()
    }

    // ── Scope management ────────────────────────────────────────────────

    fn push_scope(&mut self) {
        self.variables.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.variables.pop();
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

    // ── Functions ───────────────────────────────────────────────────────

    fn declare_function(&mut self, func: &HirFunction) -> Result<(), CodegenError> {
        let ret_type = func
            .return_type
            .as_ref()
            .map(|t| self.convert_type(t))
            .transpose()?;

        let param_types: Vec<BasicMetadataTypeEnum<'ctx>> = func
            .params
            .iter()
            .map(|p| self.convert_type(&p.ty).map(|t| t.into()))
            .collect::<Result<Vec<_>, _>>()?;

        let fn_type = match ret_type {
            Some(ty) => ty.fn_type(&param_types, false),
            None => self.context.void_type().fn_type(&param_types, false),
        };

        let llvm_fn = self.module.add_function(&func.name, fn_type, None);
        self.functions.insert(func.name.clone(), llvm_fn);
        Ok(())
    }

    fn compile_function(&mut self, func: &HirFunction) -> Result<(), CodegenError> {
        let llvm_fn = *self
            .functions
            .get(&func.name)
            .ok_or_else(|| CodegenError(format!("Function '{}' not declared", func.name)))?;

        let entry = self.context.append_basic_block(llvm_fn, "entry");
        self.builder.position_at_end(entry);

        self.push_scope();

        // Bind parameters to allocas.
        for (i, param) in func.params.iter().enumerate() {
            let param_val = llvm_fn.get_nth_param(i as u32).unwrap();
            let alloca = self.create_alloca(&param.name, param_val.get_type(), llvm_fn);
            self.builder.build_store(alloca, param_val).unwrap();
            self.define_var(param.name.clone(), alloca, param_val.get_type());
        }

        // Compile body.
        let result = self.compile_block(&func.body, llvm_fn)?;

        // Build return.
        if let Some(ret_type) = &func.return_type {
            if let Some(val) = result {
                self.builder.build_return(Some(&val)).unwrap();
            } else {
                let ret_ty = self.convert_type(ret_type)?;
                let zero = ret_ty.const_zero();
                self.builder.build_return(Some(&zero)).unwrap();
            }
        } else {
            self.builder.build_return(None).unwrap();
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
            match &stmt.kind {
                HirStmtKind::Let { name, value, .. } => {
                    if let Some(init_expr) = value {
                        let val = self.compile_expr(init_expr, function)?;
                        let alloca = self.create_alloca(name, val.get_type(), function);
                        self.builder.build_store(alloca, val).unwrap();
                        self.define_var(name.clone(), alloca, val.get_type());
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
                    // TODO: loop support in codegen
                    last_val = None;
                }
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
                // For now, build a format string and call printf-style logic.
                // Simple approach: concat all parts into one format string at compile time
                // where possible, falling back to runtime for expressions.
                //
                // For the initial version, we'll build individual prints.
                // A proper implementation would use snprintf or a string builder.
                //
                // Simplest: print each part individually and return an empty string.
                for part in parts {
                    let val = self.compile_expr(part, function)?;
                    self.emit_print_value(val)?;
                }
                // Return an empty string pointer for now.
                let global = self
                    .builder
                    .build_global_string_ptr("", "empty_str")
                    .unwrap();
                Ok(global.as_pointer_value().into())
            }

            HirExprKind::Identifier(name) => {
                if let Some((ptr, ty)) = self.lookup_var(name) {
                    let val = self.builder.build_load(ty, ptr, name).unwrap();
                    Ok(val)
                } else {
                    Err(CodegenError(format!("Undefined variable: {name}")))
                }
            }

            HirExprKind::BinaryOp { left, op, right } => {
                let lhs = self.compile_expr(left, function)?;
                let rhs = self.compile_expr(right, function)?;
                self.compile_binop(lhs, *op, rhs)
            }

            HirExprKind::UnaryOp { op, expr } => {
                let val = self.compile_expr(expr, function)?;
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

            HirExprKind::If {
                condition,
                then_block,
                else_block,
            } => self.compile_if(condition, then_block, else_block.as_deref(), function),

            HirExprKind::Block(block) => {
                self.push_scope();
                let val = self.compile_block(block, function)?;
                self.pop_scope();
                Ok(val.unwrap_or_else(|| self.context.i64_type().const_int(0, false).into()))
            }

            HirExprKind::Array(elements) => {
                // For now, arrays aren't fully supported in codegen.
                // Return a dummy value.
                if elements.is_empty() {
                    return Ok(self.context.i64_type().const_int(0, false).into());
                }
                // Compile the first element as a stand-in.
                self.compile_expr(&elements[0], function)
            }

            // Unsupported expressions return a default for now.
            _ => Ok(self.context.i64_type().const_int(0, false).into()),
        }
    }

    // ── Binary operators ────────────────────────────────────────────────

    fn compile_binop(
        &self,
        lhs: BasicValueEnum<'ctx>,
        op: BinOp,
        rhs: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        // Float operations.
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

        // Integer operations.
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

        Err(CodegenError("Mismatched types in binary operation".into()))
    }

    // ── Calls ───────────────────────────────────────────────────────────

    fn compile_call(
        &mut self,
        callee: &HirExpr,
        args: &[HirExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        // Check for print() call.
        if let HirExprKind::Identifier(name) = &callee.kind {
            if name == "print" {
                return self.compile_print(args, function);
            }

            // Regular function call.
            if let Some(&llvm_fn) = self.functions.get(name.as_str()) {
                let mut compiled_args: Vec<BasicMetadataValueEnum> = Vec::new();
                for arg in args {
                    let val = self.compile_expr(arg, function)?;
                    compiled_args.push(val.into());
                }

                let result = self
                    .builder
                    .build_call(llvm_fn, &compiled_args, "call")
                    .unwrap();

                return match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(val) => Ok(val),
                    _ => Ok(self.context.i64_type().const_int(0, false).into()),
                };
            }
        }

        // Unsupported call targets.
        Err(CodegenError("Unsupported call target".into()))
    }

    fn compile_print(
        &mut self,
        args: &[HirExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        if args.is_empty() {
            // Print newline.
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

        // Print newline after.
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
                // Bool: print "true" or "false".
                // For simplicity, print as 0/1 for now.
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
            // Assume it's a string pointer.
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

    // ── If/else ─────────────────────────────────────────────────────────

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

        // Then block.
        self.builder.position_at_end(then_bb);
        self.push_scope();
        let then_val = self.compile_block(then_block, function)?;
        self.pop_scope();
        let then_val =
            then_val.unwrap_or_else(|| self.context.i64_type().const_int(0, false).into());
        self.builder.build_unconditional_branch(merge_bb).unwrap();
        let then_bb_end = self.builder.get_insert_block().unwrap();

        // Else block.
        self.builder.position_at_end(else_bb);
        let else_val = if let Some(else_expr) = else_block {
            self.compile_expr(else_expr, function)?
        } else {
            self.context.i64_type().const_int(0, false).into()
        };
        self.builder.build_unconditional_branch(merge_bb).unwrap();
        let else_bb_end = self.builder.get_insert_block().unwrap();

        // Merge block with phi node.
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
        match &ty.kind {
            HirTypeKind::Named(name) => match name.as_str() {
                "i8" | "u8" => Ok(self.context.i8_type().into()),
                "i16" | "u16" => Ok(self.context.i16_type().into()),
                "i32" | "u32" => Ok(self.context.i32_type().into()),
                "i64" | "u64" | "isize" | "usize" => Ok(self.context.i64_type().into()),
                "i128" | "u128" => Ok(self.context.i128_type().into()),
                "f32" => Ok(self.context.f32_type().into()),
                "f64" => Ok(self.context.f64_type().into()),
                "bool" => Ok(self.context.bool_type().into()),
                "str" => Ok(self.context.ptr_type(AddressSpace::default()).into()),
                _ => Ok(self.context.i64_type().into()), // Default for user types.
            },
            HirTypeKind::Reference { .. } => {
                Ok(self.context.ptr_type(AddressSpace::default()).into())
            }
            HirTypeKind::Array { .. } => Ok(self.context.ptr_type(AddressSpace::default()).into()),
            _ => Ok(self.context.i64_type().into()),
        }
    }
}

/// Convenience function: compile a program to an object file.
pub fn compile_to_object(program: &HirProgram, output: &Path) -> Result<(), CodegenError> {
    let context = Context::create();
    let mut codegen = Codegen::new(&context, "forge_module");
    codegen.compile_program(program)?;
    codegen.write_object(output)
}

/// Convenience function: compile and link to a binary.
pub fn compile_to_binary(program: &HirProgram, output: &Path) -> Result<(), CodegenError> {
    let obj_path = output.with_extension("o");
    compile_to_object(program, &obj_path)?;

    // Link with cc.
    let status = std::process::Command::new("cc")
        .arg(&obj_path)
        .arg("-o")
        .arg(output)
        .arg("-lm") // math library
        .status()
        .map_err(|e| CodegenError(format!("Failed to run linker: {e}")))?;

    // Clean up object file.
    let _ = std::fs::remove_file(&obj_path);

    if !status.success() {
        return Err(CodegenError("Linking failed".into()));
    }

    Ok(())
}
