//! Fase D — Geração de LLVM IR via Inkwell
//!
//! Traduz a AST validada diretamente para LLVM IR em memória, sem etapa
//! textual intermediária (TAC). O módulo produzido pode ser:
//!   - Escrito como texto `.ll` para inspeção com `lli`/`llvm-as`
//!   - Passado à Fase E para emissão de objeto nativo
//!
//! # Padrão SSA / alloca
//!
//! O LLVM IR exige SSA: nenhum registrador virtual pode ser atribuído mais
//! de uma vez. Satisfazemos isso com o padrão `alloca / store / load`:
//!
//!   - Cada `var x : int`  → `%x = alloca i32` (memória na pilha)
//!   - `x = expr`          → `store i32 <val>, ptr %x`
//!   - Uso de `x`          → `%t = load i32, ptr %x`
//!
//! O passe `mem2reg` do LLVM (incluído na Fase E com otimizações) promove
//! automaticamente essas alocações para registradores SSA puros.
//!
//! # Strings
//!
//! - Cada variável `string` é representada por dois objetos LLVM:
//!     1. Um buffer `[256 x i8]` alocado na pilha (`alloca [256 x i8]`)
//!     2. Um `alloca ptr` que aponta para esse buffer
//! - Literais string tornam-se constantes globais privadas (`@str.N`)
//! - Atribuição copia via `strcpy(dst_buf, src_ptr)`
//! - Concatenação (`+`) via `strcpy` + `strcat` em buffer temporário de 512 bytes
//! - Igualdade (`==`/`!=`) via `strcmp` retornando 0 para strings iguais

use std::collections::HashMap;
use std::path::Path;

use inkwell::{
    AddressSpace, IntPredicate,
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    values::{AnyValue, BasicValueEnum, FunctionValue, PointerValue},
};

use crate::ast::*;

// ─────────────────────────────────────────────────────────────────────────────
// Estruturas de suporte
// ─────────────────────────────────────────────────────────────────────────────

/// Informação de uma variável alocada no frame da função.
#[derive(Clone)]
struct VarInfo<'ctx> {
    /// Para `int`/`bool`: ponteiro para `alloca i32` / `alloca i1`.
    /// Para `string`:    ponteiro para `alloca ptr` (que por sua vez aponta
    ///                   para o buffer `[256 x i8]` da variável).
    alloca: PointerValue<'ctx>,
    tipo:   Tipo,
}

/// Funções C externas declaradas no módulo e usadas pelo runtime de I/O e strings.
struct Externos<'ctx> {
    printf: FunctionValue<'ctx>,
    scanf:  FunctionValue<'ctx>,
    strcpy: FunctionValue<'ctx>,
    strcat: FunctionValue<'ctx>,
    strcmp: FunctionValue<'ctx>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Gerador de código
// ─────────────────────────────────────────────────────────────────────────────

pub struct Codegen<'ctx> {
    context:     &'ctx Context,
    pub module:  Module<'ctx>,
    builder:     Builder<'ctx>,
    main_fn:     FunctionValue<'ctx>,
    variaveis:   HashMap<String, VarInfo<'ctx>>,
    externos:    Externos<'ctx>,
    str_counter: u32,
}

impl<'ctx> Codegen<'ctx> {
    /// Cria o módulo LLVM e declara as funções C externas necessárias.
    pub fn new(context: &'ctx Context, nome_modulo: &str) -> Self {
        let module  = context.create_module(nome_modulo);
        let builder = context.create_builder();

        // Tipos comuns
        let ptr_ty = context.ptr_type(AddressSpace::default());
        let i32_ty = context.i32_type();

        // ── Declarações externas (libc) ───────────────────────────────────
        //
        // O linker (clang) resolve essas referências ao linkar com libc.
        // `true` no fn_type indica função variádica (printf/scanf).

        let printf = module.add_function(
            "printf",
            i32_ty.fn_type(&[ptr_ty.into()], true),
            None,
        );
        let scanf = module.add_function(
            "scanf",
            i32_ty.fn_type(&[ptr_ty.into()], true),
            None,
        );
        let strcpy = module.add_function(
            "strcpy",
            ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false),
            None,
        );
        let strcat = module.add_function(
            "strcat",
            ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false),
            None,
        );
        let strcmp = module.add_function(
            "strcmp",
            i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false),
            None,
        );

        // ── Função principal ──────────────────────────────────────────────
        //
        // Todo programa PBLang compila para uma única função `main() → i32`.
        let main_fn = module.add_function("main", i32_ty.fn_type(&[], false), None);
        let entry   = context.append_basic_block(main_fn, "entry");
        builder.position_at_end(entry);

        Self {
            context,
            module,
            builder,
            main_fn,
            variaveis:   HashMap::new(),
            externos:    Externos { printf, scanf, strcpy, strcat, strcmp },
            str_counter: 0,
        }
    }

    // ── Atalhos de tipo ───────────────────────────────────────────────────

    fn i32_ty(&self) -> inkwell::types::IntType<'ctx> { self.context.i32_type() }
    fn i1_ty(&self)  -> inkwell::types::IntType<'ctx> { self.context.bool_type() }
    fn i8_ty(&self)  -> inkwell::types::IntType<'ctx> { self.context.i8_type() }
    fn ptr_ty(&self) -> inkwell::types::PointerType<'ctx> {
        self.context.ptr_type(AddressSpace::default())
    }

    // ── Constantes globais de string ──────────────────────────────────────

    /// Cria uma constante global privada `[N x i8]` com o texto fornecido
    /// (null-terminated) e retorna um ponteiro para ela.
    ///
    /// Usado para formatos de printf/scanf e literais string do programa.
    fn global_str(&mut self, s: &str) -> PointerValue<'ctx> {
        let name = format!("str.{}", self.str_counter);
        self.str_counter += 1;

        // Converte para bytes + '\0'
        let mut bytes: Vec<u8> = s.as_bytes().to_vec();
        bytes.push(0u8);

        // Constrói o tipo array e a constante
        let arr_ty   = self.i8_ty().array_type(bytes.len() as u32);
        let global   = self.module.add_global(arr_ty, Some(AddressSpace::default()), &name);
        let elements: Vec<_> = bytes
            .iter()
            .map(|&b| self.i8_ty().const_int(b as u64, false))
            .collect();
        global.set_initializer(&self.i8_ty().const_array(&elements));
        global.set_constant(true);
        global.set_linkage(Linkage::Private);
        global.set_unnamed_addr(true); // permite fusão de strings idênticas

        global.as_pointer_value()
    }

    // ─────────────────────────────────────────────────────────────────────
    // Ponto de entrada público
    // ─────────────────────────────────────────────────────────────────────

    /// Emite IR para o programa inteiro e finaliza a função `main`.
    pub fn emitir_programa(&mut self, prog: &Programa) {
        // Passo 1: aloca todas as variáveis declaradas
        for decl in &prog.declaracoes {
            self.emitir_declaracao(decl);
        }

        // Passo 2: emite cada comando
        for cmd in &prog.comandos {
            self.emitir_comando(cmd);
        }

        // Passo 3: `return 0` — encerra a função main
        let zero = self.i32_ty().const_int(0, false);
        self.builder.build_return(Some(&zero)).unwrap();
    }

    // ─────────────────────────────────────────────────────────────────────
    // Declarações — alloca + inicialização
    // ─────────────────────────────────────────────────────────────────────

    fn emitir_declaracao(&mut self, decl: &Declaracao) {
        let nome = &decl.nome;
        match &decl.tipo {
            // var x : int  →  %x = alloca i32; store i32 0, ptr %x
            Tipo::Int => {
                let alloca = self.builder.build_alloca(self.i32_ty(), nome).unwrap();
                self.builder.build_store(alloca, self.i32_ty().const_int(0, false)).unwrap();
                self.variaveis.insert(nome.clone(), VarInfo { alloca, tipo: Tipo::Int });
            }

            // var b : bool  →  %b = alloca i1; store i1 false, ptr %b
            Tipo::Bool => {
                let alloca = self.builder.build_alloca(self.i1_ty(), nome).unwrap();
                self.builder.build_store(alloca, self.i1_ty().const_int(0, false)).unwrap();
                self.variaveis.insert(nome.clone(), VarInfo { alloca, tipo: Tipo::Bool });
            }

            // var s : string  →  buffer [256 x i8] na pilha
            //                    + alloca ptr que aponta para o buffer
            Tipo::String => {
                let buf_nome  = format!("{}_buf", nome);
                let buf_alloca = self.builder
                    .build_alloca(self.i8_ty().array_type(256), &buf_nome)
                    .unwrap();

                // ptr_alloca guarda o endereço do buffer
                let ptr_alloca = self.builder.build_alloca(self.ptr_ty(), nome).unwrap();
                self.builder.build_store(buf_alloca, ptr_alloca).unwrap();

                // Inicializa o buffer com '\0' para que o buffer seja string vazia
                let zero = self.i8_ty().const_int(0, false);
                self.builder.build_store(buf_alloca, zero).unwrap();

                self.variaveis.insert(nome.clone(), VarInfo { alloca: ptr_alloca, tipo: Tipo::String });
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Comandos
    // ─────────────────────────────────────────────────────────────────────

    fn emitir_comando(&mut self, cmd: &Comando) {
        match cmd {
            Comando::Atribuicao { nome, expr, .. } => self.emitir_atribuicao(nome, expr),
            Comando::If { condicao, entao, senao, .. } =>
                self.emitir_if(condicao, entao, senao.as_deref()),
            Comando::While { condicao, corpo, .. } =>
                self.emitir_while(condicao, corpo),
            Comando::Print { expr, .. } => self.emitir_print(expr),
            Comando::Read  { nome, .. } => self.emitir_read(nome),
            Comando::Bloco(cmds) => { for c in cmds { self.emitir_comando(c); } }
        }
    }

    /// `ID = expressão ;`
    ///
    /// - int/bool: `store <val>, ptr %alloca`
    /// - string:   `strcpy(buf_ptr, src_ptr)`
    fn emitir_atribuicao(&mut self, nome: &str, expr: &Expressao) {
        let info  = self.variaveis[nome].clone();
        match info.tipo {
            Tipo::Int | Tipo::Bool => {
                let val = self.emitir_expressao(expr).into_int_value();
                self.builder.build_store(info.alloca, val).unwrap();
            }
            Tipo::String => {
                // Carrega o ponteiro do buffer destino
                let dst = self.builder
                    .build_load(self.ptr_ty(), info.alloca, "dst_ptr")
                    .unwrap()
                    .into_pointer_value();
                // Gera o ponteiro fonte (literal global, buffer de concat, ou buffer de outra var)
                let src = self.emitir_expressao(expr).into_pointer_value();
                // strcpy(dst, src)
                self.builder.build_call(
                    self.externos.strcpy,
                    &[dst.into(), src.into()],
                    "",
                ).unwrap();
            }
        }
    }

    /// `if ( cond ) { entao } else { senao }`
    ///
    /// Blocos gerados:
    /// ```text
    ///   br i1 %cond, label %if.then, label %if.else  (ou %if.merge se sem else)
    /// if.then:
    ///   <entao>
    ///   br label %if.merge
    /// if.else:                                         (só se houver else)
    ///   <senao>
    ///   br label %if.merge
    /// if.merge:
    ///   <continuação>
    /// ```
    fn emitir_if(
        &mut self,
        condicao: &Expressao,
        entao: &[Comando],
        senao: Option<&[Comando]>,
    ) {
        let cond_val = self.emitir_expressao(condicao).into_int_value();

        let then_bb  = self.context.append_basic_block(self.main_fn, "if.then");
        let else_bb  = if senao.is_some() {
            Some(self.context.append_basic_block(self.main_fn, "if.else"))
        } else {
            None
        };
        let merge_bb = self.context.append_basic_block(self.main_fn, "if.merge");

        // Branch condicional
        let false_dest = else_bb.unwrap_or(merge_bb);
        self.builder.build_conditional_branch(cond_val, then_bb, false_dest).unwrap();

        // Bloco then
        self.builder.position_at_end(then_bb);
        for cmd in entao { self.emitir_comando(cmd); }
        self.branch_se_sem_terminator(merge_bb);

        // Bloco else (opcional)
        if let (Some(eb), Some(cmds)) = (else_bb, senao) {
            self.builder.position_at_end(eb);
            for cmd in cmds { self.emitir_comando(cmd); }
            self.branch_se_sem_terminator(merge_bb);
        }

        self.builder.position_at_end(merge_bb);
    }

    /// `while ( cond ) { corpo }`
    ///
    /// Blocos gerados:
    /// ```text
    ///   br label %while.cond
    /// while.cond:
    ///   %t = <cond>
    ///   br i1 %t, label %while.body, label %while.end
    /// while.body:
    ///   <corpo>
    ///   br label %while.cond
    /// while.end:
    ///   <continuação>
    /// ```
    fn emitir_while(&mut self, condicao: &Expressao, corpo: &[Comando]) {
        let cond_bb = self.context.append_basic_block(self.main_fn, "while.cond");
        let body_bb = self.context.append_basic_block(self.main_fn, "while.body");
        let end_bb  = self.context.append_basic_block(self.main_fn, "while.end");

        // Salta do bloco atual para a verificação da condição
        self.builder.build_unconditional_branch(cond_bb).unwrap();

        // Bloco de condição
        self.builder.position_at_end(cond_bb);
        let cond_val = self.emitir_expressao(condicao).into_int_value();
        self.builder.build_conditional_branch(cond_val, body_bb, end_bb).unwrap();

        // Bloco do corpo
        self.builder.position_at_end(body_bb);
        for cmd in corpo { self.emitir_comando(cmd); }
        self.branch_se_sem_terminator(cond_bb); // volta para a condição

        // Continuação após o laço
        self.builder.position_at_end(end_bb);
    }

    /// `print ( expr ) ;`
    ///
    /// Mapeia para `printf`:
    /// - `int`    → `printf("%d\n", val_i32)`
    /// - `bool`   → `printf("%d\n", zext_i32(val_i1))`
    /// - `string` → `printf("%s\n", ptr)`
    fn emitir_print(&mut self, expr: &Expressao) {
        let tipo = self.inferir_tipo(expr);
        let val  = self.emitir_expressao(expr);

        match tipo {
            Tipo::Int => {
                let fmt = self.global_str("%d\n");
                self.builder.build_call(
                    self.externos.printf,
                    &[fmt.into(), val.into_int_value().into()],
                    "",
                ).unwrap();
            }
            Tipo::Bool => {
                // i1 não é um tipo válido para printf — zero-extend para i32
                let fmt = self.global_str("%d\n");
                let extended = self.builder
                    .build_int_z_extend(val.into_int_value(), self.i32_ty(), "bool_ext")
                    .unwrap();
                self.builder.build_call(
                    self.externos.printf,
                    &[fmt.into(), extended.into()],
                    "",
                ).unwrap();
            }
            Tipo::String => {
                let fmt = self.global_str("%s\n");
                self.builder.build_call(
                    self.externos.printf,
                    &[fmt.into(), val.into_pointer_value().into()],
                    "",
                ).unwrap();
            }
        }
    }

    /// `read ( ID ) ;`
    ///
    /// - `int`    → `scanf("%d", ptr_alloca)`
    /// - `string` → `scanf("%255s", buf_ptr)` (limite de 255 para evitar overflow)
    fn emitir_read(&mut self, nome: &str) {
        let info = self.variaveis[nome].clone();
        match info.tipo {
            Tipo::Int => {
                let fmt = self.global_str("%d");
                // `info.alloca` é o `alloca i32` — passamos direto como int*
                self.builder.build_call(
                    self.externos.scanf,
                    &[fmt.into(), info.alloca.into()],
                    "",
                ).unwrap();
            }
            Tipo::String => {
                // Carrega o ponteiro do buffer para onde scanf vai escrever
                let buf_ptr = self.builder
                    .build_load(self.ptr_ty(), info.alloca, "buf_ptr")
                    .unwrap()
                    .into_pointer_value();
                let fmt = self.global_str("%255s");
                self.builder.build_call(
                    self.externos.scanf,
                    &[fmt.into(), buf_ptr.into()],
                    "",
                ).unwrap();
            }
            Tipo::Bool => unreachable!("análise semântica garante que read não recebe bool"),
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Expressões
    // ─────────────────────────────────────────────────────────────────────

    /// Emite IR para uma expressão e retorna o `BasicValueEnum` resultante.
    ///
    /// Tipos de retorno esperados pelo chamador:
    /// - `int`/`bool` → `BasicValueEnum::IntValue`
    /// - `string`     → `BasicValueEnum::PointerValue`
    fn emitir_expressao(&mut self, expr: &Expressao) -> BasicValueEnum<'ctx> {
        match expr {
            // ── Literais ──────────────────────────────────────────────────
            Expressao::LitInt(n, _) =>
                self.i32_ty().const_int(*n as u64, true).into(),

            Expressao::LitBool(b, _) =>
                self.i1_ty().const_int(if *b { 1 } else { 0 }, false).into(),

            Expressao::LitString(s, _) =>
                self.global_str(s).into(),

            // ── Variável — `load` do alloca ───────────────────────────────
            Expressao::Var(nome, _) => {
                let info = self.variaveis[nome.as_str()].clone();
                match info.tipo {
                    Tipo::Int    => self.builder.build_load(self.i32_ty(), info.alloca, nome).unwrap(),
                    Tipo::Bool   => self.builder.build_load(self.i1_ty(),  info.alloca, nome).unwrap(),
                    // Para string: carrega o ponteiro do buffer (não o conteúdo)
                    Tipo::String => self.builder.build_load(self.ptr_ty(), info.alloca, nome).unwrap(),
                }
            }

            // ── Operações unárias ─────────────────────────────────────────
            Expressao::UnOp { op, operando, .. } => self.emitir_unop(op, operando),

            // ── Operações binárias ────────────────────────────────────────
            Expressao::BinOp { op, esq, dir, .. } => self.emitir_binop(op, esq, dir),
        }
    }

    fn emitir_unop(&mut self, op: &OpUn, operando: &Expressao) -> BasicValueEnum<'ctx> {
        let val = self.emitir_expressao(operando).into_int_value();
        match op {
            // `-x`  →  `sub i32 0, x`
            OpUn::Neg => self.builder.build_int_neg(val, "neg").unwrap().into(),
            // `!b`  →  `xor i1 b, true`
            OpUn::Not => {
                let um = self.i1_ty().const_int(1, false);
                self.builder.build_xor(val, um, "not").unwrap().into()
            }
        }
    }

    fn emitir_binop(
        &mut self,
        op: &OpBin,
        esq: &Expressao,
        dir: &Expressao,
    ) -> BasicValueEnum<'ctx> {
        // Operações sobre strings tratadas separadamente
        // (inferir_tipo não toma &mut, então podemos chamar antes do emitir)
        let tipo_esq = self.inferir_tipo(esq);

        if *op == OpBin::Add && tipo_esq == Tipo::String {
            return self.emitir_concat_string(esq, dir);
        }
        if matches!(op, OpBin::Eq | OpBin::Neq) && tipo_esq == Tipo::String {
            return self.emitir_cmp_string(op, esq, dir);
        }

        // Operações inteiras e booleanas (operandos são IntValue)
        let lhs = self.emitir_expressao(esq).into_int_value();
        let rhs = self.emitir_expressao(dir).into_int_value();

        match op {
            OpBin::Add => self.builder.build_int_add(lhs, rhs, "add").unwrap().into(),
            OpBin::Sub => self.builder.build_int_sub(lhs, rhs, "sub").unwrap().into(),
            OpBin::Mul => self.builder.build_int_mul(lhs, rhs, "mul").unwrap().into(),
            OpBin::Div => self.builder.build_int_signed_div(lhs, rhs, "div").unwrap().into(),
            OpBin::Mod => self.builder.build_int_signed_rem(lhs, rhs, "mod").unwrap().into(),

            OpBin::Gt  => self.builder.build_int_compare(IntPredicate::SGT, lhs, rhs, "gt").unwrap().into(),
            OpBin::Lt  => self.builder.build_int_compare(IntPredicate::SLT, lhs, rhs, "lt").unwrap().into(),
            OpBin::Ge  => self.builder.build_int_compare(IntPredicate::SGE, lhs, rhs, "ge").unwrap().into(),
            OpBin::Le  => self.builder.build_int_compare(IntPredicate::SLE, lhs, rhs, "le").unwrap().into(),

            OpBin::Eq  => self.builder.build_int_compare(IntPredicate::EQ, lhs, rhs, "eq").unwrap().into(),
            OpBin::Neq => self.builder.build_int_compare(IntPredicate::NE, lhs, rhs, "neq").unwrap().into(),

            // `&&` e `||` sobre `i1` — instrução bitwise AND/OR
            // (curto-circuito não é necessário pois a semântica da PBLang não
            //  define efeitos colaterais em expressões)
            OpBin::And => self.builder.build_and(lhs, rhs, "and").unwrap().into(),
            OpBin::Or  => self.builder.build_or(lhs, rhs, "or").unwrap().into(),
        }
    }

    /// Concatenação de strings: `a + b`
    ///
    /// ```llvm
    ///   %buf = alloca [512 x i8]        ; buffer temporário na pilha
    ///   call strcpy(ptr %buf, ptr %a)   ; copia a
    ///   call strcat(ptr %buf, ptr %b)   ; concatena b
    ///   ; resultado: ptr %buf
    /// ```
    fn emitir_concat_string(&mut self, esq: &Expressao, dir: &Expressao) -> BasicValueEnum<'ctx> {
        let lhs = self.emitir_expressao(esq).into_pointer_value();
        let rhs = self.emitir_expressao(dir).into_pointer_value();

        let buf = self.builder
            .build_alloca(self.i8_ty().array_type(512), "concat_buf")
            .unwrap();

        self.builder.build_call(self.externos.strcpy, &[buf.into(), lhs.into()], "").unwrap();
        self.builder.build_call(self.externos.strcat, &[buf.into(), rhs.into()], "").unwrap();

        buf.into()
    }

    /// Comparação de strings: `a == b` ou `a != b`
    ///
    /// ```llvm
    ///   %r = call i32 strcmp(ptr %a, ptr %b)
    ///   %eq = icmp eq i32 %r, 0    ; (ou ne para !=)
    /// ```
    fn emitir_cmp_string(
        &mut self,
        op: &OpBin,
        esq: &Expressao,
        dir: &Expressao,
    ) -> BasicValueEnum<'ctx> {
        let lhs = self.emitir_expressao(esq).into_pointer_value();
        let rhs = self.emitir_expressao(dir).into_pointer_value();

        let cmp_result = self.builder
            .build_call(self.externos.strcmp, &[lhs.into(), rhs.into()], "strcmp_res")
            .unwrap()
            .as_any_value_enum()
            .into_int_value();

        let zero = self.i32_ty().const_int(0, false);
        match op {
            OpBin::Eq  => self.builder.build_int_compare(IntPredicate::EQ, cmp_result, zero, "str_eq").unwrap().into(),
            OpBin::Neq => self.builder.build_int_compare(IntPredicate::NE, cmp_result, zero, "str_neq").unwrap().into(),
            _          => unreachable!(),
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Inferência de tipo (sem erros — roda pós-análise semântica)
    // ─────────────────────────────────────────────────────────────────────

    /// Determina o tipo de uma expressão sem emitir IR.
    ///
    /// Necessário antes de emitir certas operações (print, string +) para
    /// decidir qual instrução/formato usar.
    fn inferir_tipo(&self, expr: &Expressao) -> Tipo {
        match expr {
            Expressao::LitInt(_, _)    => Tipo::Int,
            Expressao::LitBool(_, _)   => Tipo::Bool,
            Expressao::LitString(_, _) => Tipo::String,
            Expressao::Var(nome, _)    => self.variaveis[nome.as_str()].tipo.clone(),
            Expressao::UnOp { op, .. } => match op {
                OpUn::Neg => Tipo::Int,
                OpUn::Not => Tipo::Bool,
            },
            Expressao::BinOp { op, esq, .. } => match op {
                OpBin::Add => self.inferir_tipo(esq), // int+int→int, string+string→string
                OpBin::Sub | OpBin::Mul | OpBin::Div | OpBin::Mod => Tipo::Int,
                OpBin::Gt  | OpBin::Lt  | OpBin::Ge  | OpBin::Le
                | OpBin::Eq | OpBin::Neq | OpBin::And | OpBin::Or => Tipo::Bool,
            },
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Utilitário de bloco básico
    // ─────────────────────────────────────────────────────────────────────

    /// Insere `br <dest>` somente se o bloco atual ainda não tem terminador.
    ///
    /// Evita "duplicate terminator" quando o último comando de um bloco é
    /// um `while` que já gerou seu próprio `br`.
    fn branch_se_sem_terminator(&self, dest: inkwell::basic_block::BasicBlock<'ctx>) {
        if self.builder.get_insert_block()
            .and_then(|bb| bb.get_terminator())
            .is_none()
        {
            self.builder.build_unconditional_branch(dest).unwrap();
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Saída
    // ─────────────────────────────────────────────────────────────────────

    /// Verifica a integridade interna da IR (útil para debugging).
    ///
    /// Retorna `Ok(())` se a IR está bem formada, `Err(msg)` caso contrário.
    pub fn verificar(&self) -> Result<(), String> {
        self.module.verify().map_err(|e| e.to_string())
    }

    /// Escreve a IR em formato texto (`.ll`) para inspeção com `lli`/`llvm-dis`.
    pub fn escrever_ir(&self, path: &Path) -> Result<(), String> {
        self.module.print_to_file(path).map_err(|e| e.to_string())
    }

    /// Retorna a IR como `String` — usado nos testes para verificar padrões.
    pub fn ir_para_string(&self) -> String {
        self.module.print_to_string().to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Ponto de entrada público
// ─────────────────────────────────────────────────────────────────────────────

/// Gera a LLVM IR para `prog` (já validado semanticamente).
///
/// Retorna o `Codegen` com o módulo preenchido — o chamador pode então
/// chamar `verificar()`, `escrever_ir()` ou usar o módulo na Fase E.
pub fn gerar_ir<'ctx>(
    context: &'ctx Context,
    prog: &Programa,
    nome_modulo: &str,
) -> Codegen<'ctx> {
    let mut cg = Codegen::new(context, nome_modulo);
    cg.emitir_programa(prog);
    cg
}

// ─────────────────────────────────────────────────────────────────────────────
// Testes
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser, semantic};

    /// Compila `src` até IR e retorna o codegen (IR já verificada internamente).
    fn compilar(src: &str) -> (Context, String) {
        let tokens  = lexer::lex(src).expect("erro léxico");
        let ast     = parser::parse(tokens).expect("erro sintático");
        let _tabela = semantic::verificar(&ast).expect("erro semântico");
        let ctx     = Context::create();
        // Need to move ctx out — use a workaround with Box
        // We return the IR string instead
        let ir = {
            let cg = gerar_ir(&ctx, &ast, "test");
            cg.verificar().expect("IR inválida");
            cg.ir_para_string()
        };
        (ctx, ir)
    }

    fn ir_de(src: &str) -> String {
        compilar(src).1
    }

    // ── Declarações → alloca ──────────────────────────────────────────────

    #[test]
    fn test_ir_alloca_int() {
        let ir = ir_de("var x : int;");
        assert!(ir.contains("alloca i32"), "esperado alloca i32:\n{}", ir);
    }

    #[test]
    fn test_ir_alloca_bool() {
        let ir = ir_de("var b : bool;");
        assert!(ir.contains("alloca i1"), "esperado alloca i1:\n{}", ir);
    }

    #[test]
    fn test_ir_alloca_string() {
        let ir = ir_de("var s : string;");
        assert!(ir.contains("alloca [256 x i8]"), "esperado buffer [256 x i8]:\n{}", ir);
        assert!(ir.contains("alloca ptr"), "esperado alloca ptr:\n{}", ir);
    }

    // ── Atribuição → store ────────────────────────────────────────────────

    #[test]
    fn test_ir_store_int() {
        let ir = ir_de("var x : int; x = 42;");
        assert!(ir.contains("store i32"), "esperado store i32:\n{}", ir);
        assert!(ir.contains("42"), "esperado literal 42:\n{}", ir);
    }

    #[test]
    fn test_ir_store_bool_true() {
        let ir = ir_de("var b : bool; b = true;");
        assert!(ir.contains("store i1"), "esperado store i1:\n{}", ir);
    }

    // ── Operações aritméticas ─────────────────────────────────────────────
    // NOTA: operandos devem ser variáveis, não literais.
    // O LLVM dobra (constant-fold) operações entre dois literais diretamente
    // na construção da IR — "1 + 2" vira "store i32 3" sem emitir "add".
    // Usando variáveis forçamos a emissão das instruções de load + operação.

    #[test]
    fn test_ir_add() {
        let ir = ir_de("var x : int; var y : int; x = x + y;");
        assert!(ir.contains("add i32"), "esperado add i32:\n{}", ir);
    }

    #[test]
    fn test_ir_sub() {
        let ir = ir_de("var x : int; var y : int; x = x - y;");
        assert!(ir.contains("sub i32"), "esperado sub i32:\n{}", ir);
    }

    #[test]
    fn test_ir_mul() {
        let ir = ir_de("var x : int; var y : int; x = x * y;");
        assert!(ir.contains("mul i32"), "esperado mul i32:\n{}", ir);
    }

    #[test]
    fn test_ir_div() {
        let ir = ir_de("var x : int; var y : int; x = x / y;");
        assert!(ir.contains("sdiv i32"), "esperado sdiv i32:\n{}", ir);
    }

    #[test]
    fn test_ir_mod() {
        let ir = ir_de("var x : int; var y : int; x = x % y;");
        assert!(ir.contains("srem i32"), "esperado srem i32:\n{}", ir);
    }

    #[test]
    fn test_ir_neg_unario() {
        let ir = ir_de("var x : int; var y : int; x = -y;");
        // negação → sub i32 0, %y_carregado
        assert!(ir.contains("sub i32"), "esperado sub (neg) i32:\n{}", ir);
    }

    // ── Operações lógicas e relacionais ───────────────────────────────────

    #[test]
    fn test_ir_icmp_gt() {
        let ir = ir_de("var b : bool; var x : int; b = x > 0;");
        assert!(ir.contains("icmp sgt"), "esperado icmp sgt:\n{}", ir);
    }

    #[test]
    fn test_ir_icmp_eq() {
        let ir = ir_de("var b : bool; var x : int; b = x == 0;");
        assert!(ir.contains("icmp eq"), "esperado icmp eq:\n{}", ir);
    }

    #[test]
    fn test_ir_and() {
        let ir = ir_de("var b : bool; var c : bool; b = b && c;");
        assert!(ir.contains("and i1"), "esperado and i1:\n{}", ir);
    }

    #[test]
    fn test_ir_or() {
        let ir = ir_de("var b : bool; var c : bool; b = b || c;");
        assert!(ir.contains("or i1"), "esperado or i1:\n{}", ir);
    }

    #[test]
    fn test_ir_not() {
        let ir = ir_de("var b : bool; var c : bool; b = !c;");
        assert!(ir.contains("xor i1"), "esperado xor i1 (not):\n{}", ir);
    }

    // ── Estruturas de controle ────────────────────────────────────────────

    #[test]
    fn test_ir_if_simples() {
        let ir = ir_de("var x : int; if (x > 0) { x = 1; }");
        assert!(ir.contains("if.then"),  "esperado bloco if.then:\n{}", ir);
        assert!(ir.contains("if.merge"), "esperado bloco if.merge:\n{}", ir);
        assert!(ir.contains("br i1"),    "esperado br condicional:\n{}", ir);
    }

    #[test]
    fn test_ir_if_else() {
        let ir = ir_de("var x : int; if (x > 0) { x = 1; } else { x = 0; }");
        assert!(ir.contains("if.then"),  "esperado bloco if.then:\n{}", ir);
        assert!(ir.contains("if.else"),  "esperado bloco if.else:\n{}", ir);
        assert!(ir.contains("if.merge"), "esperado bloco if.merge:\n{}", ir);
    }

    #[test]
    fn test_ir_while() {
        let ir = ir_de("var n : int; while (n > 0) { n = n - 1; }");
        assert!(ir.contains("while.cond"), "esperado while.cond:\n{}", ir);
        assert!(ir.contains("while.body"), "esperado while.body:\n{}", ir);
        assert!(ir.contains("while.end"),  "esperado while.end:\n{}", ir);
    }

    // ── I/O ───────────────────────────────────────────────────────────────

    #[test]
    fn test_ir_print_int() {
        let ir = ir_de("var x : int; print(x);");
        assert!(ir.contains("printf"), "esperado chamada printf:\n{}", ir);
        assert!(ir.contains("%d"),     "esperado formato %d:\n{}", ir);
    }

    #[test]
    fn test_ir_print_bool() {
        let ir = ir_de("var b : bool; b = true; print(b);");
        assert!(ir.contains("printf"),   "esperado chamada printf:\n{}", ir);
        assert!(ir.contains("zext i1"),  "esperado zext i1 (bool para i32):\n{}", ir);
    }

    #[test]
    fn test_ir_print_string() {
        let ir = ir_de(r#"var s : string; s = "hello"; print(s);"#);
        assert!(ir.contains("printf"), "esperado chamada printf:\n{}", ir);
        assert!(ir.contains("%s"),     "esperado formato %s:\n{}", ir);
    }

    #[test]
    fn test_ir_read_int() {
        let ir = ir_de("var x : int; read(x);");
        assert!(ir.contains("scanf"), "esperado chamada scanf:\n{}", ir);
        assert!(ir.contains("%d"),    "esperado formato %d:\n{}", ir);
    }

    #[test]
    fn test_ir_read_string() {
        let ir = ir_de("var s : string; read(s);");
        assert!(ir.contains("scanf"),  "esperado chamada scanf:\n{}", ir);
        assert!(ir.contains("%255s"),  "esperado formato %255s:\n{}", ir);
    }

    // ── Strings ───────────────────────────────────────────────────────────

    #[test]
    fn test_ir_literal_string() {
        let ir = ir_de(r#"var s : string; s = "PBLang";"#);
        assert!(ir.contains("PBLang"), "esperado literal PBLang na IR:\n{}", ir);
        assert!(ir.contains("strcpy"), "esperado strcpy na atribuição:\n{}", ir);
    }

    #[test]
    fn test_ir_concat_string() {
        let ir = ir_de(r#"var s : string; var t : string; s = "a"; t = s + "b";"#);
        assert!(ir.contains("strcpy"),           "esperado strcpy:\n{}", ir);
        assert!(ir.contains("strcat"),           "esperado strcat:\n{}", ir);
        assert!(ir.contains("alloca [512 x i8]"),"esperado buffer de concat:\n{}", ir);
    }

    #[test]
    fn test_ir_cmp_string_eq() {
        let ir = ir_de(r#"var b : bool; var s : string; s = "x"; b = s == "x";"#);
        assert!(ir.contains("strcmp"),  "esperado strcmp:\n{}", ir);
        assert!(ir.contains("icmp eq"), "esperado icmp eq após strcmp:\n{}", ir);
    }

    // ── Programas completos ───────────────────────────────────────────────

    #[test]
    fn test_programa_fatorial_ir_valida() {
        let src = r#"
            var n         : int;
            var resultado : int;
            read(n);
            resultado = 1;
            while (n > 1) {
                resultado = resultado * n;
                n = n - 1;
            }
            print(resultado);
        "#;
        let ir = ir_de(src);
        assert!(ir.contains("while.cond"));
        assert!(ir.contains("scanf"));
        assert!(ir.contains("printf"));
        assert!(ir.contains("mul i32"));
    }

    #[test]
    fn test_programa_if_else_ir_valida() {
        let src = r#"
            var x   : int;
            var par : bool;
            read(x);
            par = false;
            if (x == 0) { par = true; } else { par = false; }
            print(par);
        "#;
        let ir = ir_de(src);
        assert!(ir.contains("if.then"));
        assert!(ir.contains("if.else"));
        assert!(ir.contains("icmp eq"));
        assert!(ir.contains("zext i1")); // bool para printf
    }

    #[test]
    fn test_programa_strings_ir_valida() {
        let src = r#"
            var nome     : string;
            var saudacao : string;
            read(nome);
            saudacao = "Olá, ";
            saudacao = saudacao + nome;
            print(saudacao);
        "#;
        let ir = ir_de(src);
        assert!(ir.contains("strcpy"));
        assert!(ir.contains("strcat"));
        assert!(ir.contains("%255s"));
        assert!(ir.contains("%s"));
    }

    #[test]
    fn test_main_tem_ret_i32() {
        let ir = ir_de("var x : int; x = 1;");
        assert!(ir.contains("ret i32 0"), "esperado ret i32 0:\n{}", ir);
    }

    #[test]
    fn test_declaracoes_externas_presentes() {
        let ir = ir_de("var x : int; read(x); print(x);");
        assert!(ir.contains("declare"), "esperado declarações externas:\n{}", ir);
        assert!(ir.contains("printf"));
        assert!(ir.contains("scanf"));
    }
}
