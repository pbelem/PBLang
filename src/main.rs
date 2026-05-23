mod error;
mod lexer;
mod ast;
mod parser;

use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Uso: pblang <arquivo.pb> [--emit-tokens] [--emit-ast]");
        process::exit(1);
    }

    let path        = &args[1];
    let emit_tokens = args.iter().any(|a| a == "--emit-tokens");
    let emit_ast    = args.iter().any(|a| a == "--emit-ast");

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

    // Fases C–F: próximas iterações
    println!("[Fase A] OK — lexer");
    println!("[Fase B] OK — {} declarações, {} comandos",
        ast.declaracoes.len(), ast.comandos.len());
}
