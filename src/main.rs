mod error;
mod lexer;
mod ast;
mod parser;
mod symbol_table;
mod semantic;
mod codegen;

use std::{env, fs, process};
use inkwell::context::Context;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Uso: pblang <arquivo.pb> [--emit-tokens] [--emit-ast] [--emit-ir]");
        eprintln!();
        eprintln!("  --emit-tokens   imprime stream de tokens (Fase A)");
        eprintln!("  --emit-ast      imprime AST (Fase B)");
        eprintln!("  --emit-ir       escreve IR LLVM em <arquivo>.ll (Fase D)");
        process::exit(1);
    }

    let path        = &args[1];
    let emit_tokens = args.iter().any(|a| a == "--emit-tokens");
    let emit_ast    = args.iter().any(|a| a == "--emit-ast");
    let emit_ir     = args.iter().any(|a| a == "--emit-ir");

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => { eprintln!("Erro ao ler '{}': {}", path, e); process::exit(1); }
    };

    // ── Fase A — Análise Léxica ───────────────────────────────────────────
    let tokens = match lexer::lex(&source) {
        Ok(toks) => toks,
        Err(errors) => {
            for err in &errors { eprintln!("{}", err); }
            process::exit(1);
        }
    };

    if emit_tokens {
        println!("=== Tokens ({}) ===", tokens.len());
        for st in &tokens {
            println!("  {:3}:{:<3}  {:?}", st.line, st.col, st.token);
        }
    }

    // ── Fase B — Análise Sintática ────────────────────────────────────────
    let ast = match parser::parse(tokens) {
        Ok(prog) => prog,
        Err(errors) => {
            for err in &errors { eprintln!("{}", err); }
            process::exit(1);
        }
    };

    if emit_ast {
        println!("=== AST ===\n{:#?}", ast);
    }

    // ── Fase C — Análise Semântica ────────────────────────────────────────
    let _tabela = match semantic::verificar(&ast) {
        Ok(tabela) => tabela,
        Err(errors) => {
            for err in &errors { eprintln!("{}", err); }
            process::exit(1);
        }
    };

    // ── Fase D — Geração de IR LLVM ───────────────────────────────────────
    let nome_modulo = path.trim_end_matches(".pb");
    let context     = Context::create();
    let cg          = codegen::gerar_ir(&context, &ast, nome_modulo);

    if let Err(e) = cg.verificar() {
        eprintln!("[ERRO CODEGEN] {}", e);
        process::exit(1);
    }

    if emit_ir {
        let ir_path = format!("{}.ll", nome_modulo);
        cg.escrever_ir(std::path::Path::new(&ir_path))
            .unwrap_or_else(|e| {
                eprintln!("Erro ao escrever '{}': {}", ir_path, e);
                process::exit(1);
            });
        println!("[Fase D] IR escrita em '{}'", ir_path);
    }

    println!("[Fase A] OK — lexer");
    println!("[Fase B] OK — {} declarações, {} comandos",
        ast.declaracoes.len(), ast.comandos.len());
    println!("[Fase C] OK — análise semântica");
    println!("[Fase D] OK — IR LLVM gerada (use --emit-ir para gravar .ll)");
}
