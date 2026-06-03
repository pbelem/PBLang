use crate::ast::*;
use crate::error::PBError;
use crate::symbol_table::TabelaDeSimbolos;

// ─────────────────────────────────────────────────────────────────────────────
// Analisador Semântico
// ─────────────────────────────────────────────────────────────────────────────

/// Percorre a AST e verifica:
///
/// 1. **Escopo** — toda variável usada deve ter sido declarada; nenhuma
///    variável pode ser declarada duas vezes.
/// 2. **Tipos** — cada operação recebe operandos do tipo correto; atribuições
///    e condicionais respeitam o tipo esperado.
///
/// Erros são acumulados em `Vec<PBError>` — o analisador continua após cada
/// erro para reportar o máximo de problemas em uma única execução.
struct Analisador {
    tabela: TabelaDeSimbolos,
    erros:  Vec<PBError>,
}

impl Analisador {
    fn new() -> Self {
        Self { tabela: TabelaDeSimbolos::new(), erros: Vec::new() }
    }

    // ── Helpers de erro ───────────────────────────────────────────────────

    fn erro(&mut self, msg: String, span: &Span) {
        self.erros.push(PBError::Semantic {
            message: msg,
            line: span.line,
            col:  span.col,
        });
    }

    // ── Programa ──────────────────────────────────────────────────────────

    fn verificar_programa(&mut self, prog: &Programa) {
        // Fase 1: registra todas as declarações na tabela de símbolos.
        // Feito em passe separado antes de verificar os comandos,
        // para que a tabela esteja completa antes de qualquer uso.
        for decl in &prog.declaracoes {
            if let Err(e) = self.tabela.declarar(&decl.nome, decl.tipo.clone(), &decl.span) {
                self.erros.push(e);
            }
        }

        // Fase 2: verifica todos os comandos.
        for cmd in &prog.comandos {
            self.verificar_comando(cmd);
        }
    }

    // ── Comandos ──────────────────────────────────────────────────────────

    fn verificar_comando(&mut self, cmd: &Comando) {
        match cmd {
            Comando::Atribuicao { nome, expr, span } => {
                self.verificar_atribuicao(nome, expr, span);
            }
            Comando::If { condicao, entao, senao, span } => {
                self.verificar_if(condicao, entao, senao.as_deref(), span);
            }
            Comando::While { condicao, corpo, span } => {
                self.verificar_while(condicao, corpo, span);
            }
            Comando::Print { expr, .. } => {
                // print aceita qualquer tipo — apenas verifica que a expressão
                // é válida (variáveis declaradas, operadores bem tipados).
                self.inferir_tipo(expr);
            }
            Comando::Read { nome, span } => {
                self.verificar_read(nome, span);
            }
            Comando::Bloco(cmds) => {
                for c in cmds { self.verificar_comando(c); }
            }
        }
    }

    /// `ID = expressão ;`
    ///
    /// O tipo da expressão deve ser idêntico ao tipo declarado do ID.
    fn verificar_atribuicao(&mut self, nome: &str, expr: &Expressao, span: &Span) {
        // Consulta o tipo declarado da variável
        let tipo_var = match self.tabela.consultar(nome, span) {
            Ok(t) => t.clone(),
            Err(e) => { self.erros.push(e); return; }
        };

        // Infere o tipo da expressão à direita
        if let Some(tipo_expr) = self.inferir_tipo(expr) {
            if tipo_expr != tipo_var {
                self.erro(
                    format!(
                        "Tipo incompatível na atribuição a '{}': esperado '{}', obtido '{}'",
                        nome, tipo_var, tipo_expr
                    ),
                    span,
                );
            }
        }
    }

    /// `if ( condição ) bloco ( else bloco )?`
    ///
    /// A condição deve ser `bool`.
    fn verificar_if(
        &mut self,
        condicao: &Expressao,
        entao: &[Comando],
        senao: Option<&[Comando]>,
        _span: &Span,
    ) {
        self.exigir_bool(condicao, "condição do 'if'");
        for cmd in entao { self.verificar_comando(cmd); }
        if let Some(cmds) = senao {
            for cmd in cmds { self.verificar_comando(cmd); }
        }
    }

    /// `while ( condição ) bloco`
    ///
    /// A condição deve ser `bool`.
    fn verificar_while(&mut self, condicao: &Expressao, corpo: &[Comando], _span: &Span) {
        self.exigir_bool(condicao, "condição do 'while'");
        for cmd in corpo { self.verificar_comando(cmd); }
    }

    /// `read ( ID ) ;`
    ///
    /// O ID deve ser `int` ou `string` (não `bool`).
    fn verificar_read(&mut self, nome: &str, span: &Span) {
        match self.tabela.consultar(nome, span) {
            Err(e) => self.erros.push(e),
            Ok(Tipo::Bool) => self.erro(
                format!(
                    "'read' não suporta o tipo 'bool' — variável '{}' deve ser 'int' ou 'string'",
                    nome
                ),
                span,
            ),
            Ok(_) => {} // int ou string: OK
        }
    }

    // ── Helpers de verificação de tipos ───────────────────────────────────

    /// Verifica que `expr` resolve para `bool`; caso contrário, registra erro.
    fn exigir_bool(&mut self, expr: &Expressao, contexto: &str) {
        if let Some(t) = self.inferir_tipo(expr) {
            if t != Tipo::Bool {
                self.erro(
                    format!(
                        "A {} deve ser do tipo 'bool', mas obteve '{}'",
                        contexto, t
                    ),
                    expr.span(),
                );
            }
        }
    }

    // ── Inferência de tipos ───────────────────────────────────────────────

    /// Infere o tipo de uma expressão e retorna `Some(Tipo)`.
    ///
    /// Retorna `None` se houver um erro de tipo (já registrado em `self.erros`),
    /// para que o chamador não continue a propagar erros em cascata.
    fn inferir_tipo(&mut self, expr: &Expressao) -> Option<Tipo> {
        match expr {
            // ── Literais ──────────────────────────────────────────────────
            Expressao::LitInt(_, _)    => Some(Tipo::Int),
            Expressao::LitBool(_, _)   => Some(Tipo::Bool),
            Expressao::LitString(_, _) => Some(Tipo::String),

            // ── Variável ──────────────────────────────────────────────────
            Expressao::Var(nome, span) => {
                match self.tabela.consultar(nome, span) {
                    Ok(t)  => Some(t.clone()),
                    Err(e) => { self.erros.push(e); None }
                }
            }

            // ── Operações unárias ─────────────────────────────────────────
            Expressao::UnOp { op, operando, span } => {
                self.verificar_unario(op, operando, span)
            }

            // ── Operações binárias ────────────────────────────────────────
            Expressao::BinOp { op, esq, dir, span } => {
                self.verificar_binario(op, esq, dir, span)
            }
        }
    }

    /// Verifica e retorna o tipo de uma operação unária.
    ///
    /// | Op  | Operando | Resultado |
    /// |-----|----------|-----------|
    /// | `-` | `int`    | `int`     |
    /// | `!` | `bool`   | `bool`    |
    fn verificar_unario(&mut self, op: &OpUn, operando: &Expressao, span: &Span) -> Option<Tipo> {
        let t = self.inferir_tipo(operando)?;
        match op {
            OpUn::Neg => {
                if t != Tipo::Int {
                    self.erro(
                        format!("Operador '-' (negação) requer 'int', obteve '{}'", t),
                        span,
                    );
                    return None;
                }
                Some(Tipo::Int)
            }
            OpUn::Not => {
                if t != Tipo::Bool {
                    self.erro(
                        format!("Operador '!' requer 'bool', obteve '{}'", t),
                        span,
                    );
                    return None;
                }
                Some(Tipo::Bool)
            }
        }
    }

    /// Verifica e retorna o tipo de uma operação binária.
    ///
    /// Consultar a tabela de regras na seção 4.3 do documento de requisitos.
    fn verificar_binario(
        &mut self,
        op: &OpBin,
        esq: &Expressao,
        dir: &Expressao,
        span: &Span,
    ) -> Option<Tipo> {
        // Infere os dois lados antes de checar — acumula erros em ambos
        // mesmo que o primeiro já falhe.
        let t_esq = self.inferir_tipo(esq);
        let t_dir = self.inferir_tipo(dir);

        match op {
            // ── Aritméticos (exceto +) → (int, int) → int ─────────────────
            OpBin::Sub | OpBin::Mul | OpBin::Div | OpBin::Mod => {
                self.exigir_ambos_int(op, t_esq, t_dir, span)?;
                Some(Tipo::Int)
            }

            // ── Adição: (int,int)→int  ou  (string,string)→string ─────────
            OpBin::Add => {
                let (te, td) = (t_esq?, t_dir?);
                match (&te, &td) {
                    (Tipo::Int, Tipo::Int)       => Some(Tipo::Int),
                    (Tipo::String, Tipo::String) => Some(Tipo::String),
                    _ => {
                        self.erro(
                            format!(
                                "Operador '+' não suportado entre '{}' e '{}' \
                                 (use '+' apenas com int+int ou string+string)",
                                te, td
                            ),
                            span,
                        );
                        None
                    }
                }
            }

            // ── Relacionais → (int, int) → bool ──────────────────────────
            OpBin::Gt | OpBin::Lt | OpBin::Ge | OpBin::Le => {
                self.exigir_ambos_int(op, t_esq, t_dir, span)?;
                Some(Tipo::Bool)
            }

            // ── Igualdade → (T, T) → bool  (qualquer tipo, mas igual) ─────
            OpBin::Eq | OpBin::Neq => {
                let (te, td) = (t_esq?, t_dir?);
                if te != td {
                    self.erro(
                        format!(
                            "Operador '{}' exige operandos do mesmo tipo, \
                             mas obteve '{}' e '{}'",
                            op, te, td
                        ),
                        span,
                    );
                    return None;
                }
                Some(Tipo::Bool)
            }

            // ── Lógicos → (bool, bool) → bool ────────────────────────────
            OpBin::And | OpBin::Or => {
                let (te, td) = (t_esq?, t_dir?);
                let mut ok = true;
                if te != Tipo::Bool {
                    self.erro(
                        format!("Operador '{}' requer 'bool' à esquerda, obteve '{}'", op, te),
                        span,
                    );
                    ok = false;
                }
                if td != Tipo::Bool {
                    self.erro(
                        format!("Operador '{}' requer 'bool' à direita, obteve '{}'", op, td),
                        span,
                    );
                    ok = false;
                }
                if ok { Some(Tipo::Bool) } else { None }
            }
        }
    }

    /// Verifica que ambos os lados são `int`; registra erro descritivo caso contrário.
    fn exigir_ambos_int(
        &mut self,
        op: &OpBin,
        t_esq: Option<Tipo>,
        t_dir: Option<Tipo>,
        span: &Span,
    ) -> Option<()> {
        let (te, td) = (t_esq?, t_dir?);
        let mut ok = true;
        if te != Tipo::Int {
            self.erro(
                format!("Operador '{}' requer 'int' à esquerda, obteve '{}'", op, te),
                span,
            );
            ok = false;
        }
        if td != Tipo::Int {
            self.erro(
                format!("Operador '{}' requer 'int' à direita, obteve '{}'", op, td),
                span,
            );
            ok = false;
        }
        if ok { Some(()) } else { None }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Ponto de entrada público
// ─────────────────────────────────────────────────────────────────────────────

/// Verifica semanticamente a AST e retorna a tabela de símbolos preenchida,
/// ou uma lista de todos os erros semânticos encontrados.
///
/// A tabela de símbolos retornada será usada pela Fase D (codegen).
pub fn verificar(prog: &Programa) -> Result<TabelaDeSimbolos, Vec<PBError>> {
    let mut analisador = Analisador::new();
    analisador.verificar_programa(prog);

    if analisador.erros.is_empty() {
        Ok(analisador.tabela)
    } else {
        Err(analisador.erros)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Testes
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser};

    // ── Helpers ───────────────────────────────────────────────────────────

    fn analisar_ok(src: &str) -> TabelaDeSimbolos {
        let tokens = lexer::lex(src).expect("erro léxico inesperado");
        let ast    = parser::parse(tokens).expect("erro sintático inesperado");
        verificar(&ast).unwrap_or_else(|e| panic!("erro semântico inesperado: {:#?}", e))
    }

    fn analisar_err(src: &str) -> Vec<PBError> {
        let tokens = lexer::lex(src).expect("erro léxico inesperado");
        let ast    = parser::parse(tokens).expect("erro sintático inesperado");
        verificar(&ast).expect_err("verificação deveria ter falhado")
    }

    // ── Escopo e declaração ───────────────────────────────────────────────

    #[test]
    fn test_declaracao_valida() {
        analisar_ok("var x : int; var b : bool; var s : string;");
    }

    #[test]
    fn test_uso_de_variavel_nao_declarada() {
        let errs = analisar_err("x = 10;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'x'")
        )));
    }

    #[test]
    fn test_redeclaracao_e_erro() {
        let errs = analisar_err("var x : int; var x : bool;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("já foi declarada")
        )));
    }

    #[test]
    fn test_variavel_usada_na_expressao_sem_declaracao() {
        let errs = analisar_err("var x : int; x = y + 1;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'y'")
        )));
    }

    // ── Atribuição ────────────────────────────────────────────────────────

    #[test]
    fn test_atribuicao_tipo_correto_int() {
        analisar_ok("var x : int; x = 42;");
    }

    #[test]
    fn test_atribuicao_tipo_correto_bool() {
        analisar_ok("var b : bool; b = true;");
    }

    #[test]
    fn test_atribuicao_tipo_correto_string() {
        analisar_ok(r#"var s : string; s = "hello";"#);
    }

    #[test]
    fn test_atribuicao_tipo_errado_int_recebe_bool() {
        let errs = analisar_err("var x : int; x = true;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. }
            if message.contains("'x'") && message.contains("'int'") && message.contains("'bool'")
        )));
    }

    #[test]
    fn test_atribuicao_tipo_errado_bool_recebe_int() {
        let errs = analisar_err("var b : bool; b = 1;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. }
            if message.contains("'bool'") && message.contains("'int'")
        )));
    }

    #[test]
    fn test_atribuicao_tipo_errado_string_recebe_int() {
        let errs = analisar_err(r#"var s : string; s = 42;"#);
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. }
            if message.contains("'string'") && message.contains("'int'")
        )));
    }

    // ── Operações aritméticas ─────────────────────────────────────────────

    #[test]
    fn test_aritmetica_int_valida() {
        analisar_ok("var x : int; x = 2 + 3 * 4 - 1 / 2 % 3;");
    }

    #[test]
    fn test_aritmetica_com_bool_e_erro() {
        let errs = analisar_err("var x : int; x = 1 + true;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'+'")
        )));
    }

    #[test]
    fn test_subtracao_com_string_e_erro() {
        let errs = analisar_err(r#"var x : int; var s : string; s = "a"; x = x - s;"#);
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'-'")
        )));
    }

    #[test]
    fn test_negacao_unaria_int_valida() {
        analisar_ok("var x : int; x = -42;");
    }

    #[test]
    fn test_negacao_unaria_bool_e_erro() {
        let errs = analisar_err("var x : int; x = -true;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("negação")
        )));
    }

    // ── Concatenação de strings ───────────────────────────────────────────

    #[test]
    fn test_concatenacao_string_valida() {
        analisar_ok(r#"var s : string; var t : string; t = "a"; s = t + "b";"#);
    }

    #[test]
    fn test_adicao_int_com_string_e_erro() {
        let errs = analisar_err(r#"var x : int; var s : string; s = "a"; x = x + s;"#);
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'+'")
        )));
    }

    // ── Operações relacionais ─────────────────────────────────────────────

    #[test]
    fn test_relacional_int_valido() {
        analisar_ok("var b : bool; var x : int; b = x > 0;");
    }

    #[test]
    fn test_relacional_com_bool_e_erro() {
        let errs = analisar_err("var b : bool; b = true > false;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'>'")
        )));
    }

    // ── Igualdade ─────────────────────────────────────────────────────────

    #[test]
    fn test_igualdade_int_valida() {
        analisar_ok("var b : bool; var x : int; b = x == 0;");
    }

    #[test]
    fn test_igualdade_bool_valida() {
        analisar_ok("var b : bool; b = true == false;");
    }

    #[test]
    fn test_igualdade_string_valida() {
        analisar_ok(r#"var b : bool; var s : string; s = "x"; b = s == "y";"#);
    }

    #[test]
    fn test_igualdade_tipos_diferentes_e_erro() {
        let errs = analisar_err("var b : bool; var x : int; b = x == true;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("mesmo tipo")
        )));
    }

    // ── Operações lógicas ─────────────────────────────────────────────────

    #[test]
    fn test_logico_and_bool_valido() {
        analisar_ok("var b : bool; b = true && false;");
    }

    #[test]
    fn test_logico_or_bool_valido() {
        analisar_ok("var b : bool; b = true || false;");
    }

    #[test]
    fn test_not_bool_valido() {
        analisar_ok("var b : bool; b = !true;");
    }

    #[test]
    fn test_logico_and_com_int_e_erro() {
        let errs = analisar_err("var b : bool; b = 1 && true;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'&&'")
        )));
    }

    #[test]
    fn test_not_com_int_e_erro() {
        let errs = analisar_err("var x : int; var b : bool; x = 5; b = !x;");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'!'")
        )));
    }

    // ── Estruturas de controle ────────────────────────────────────────────

    #[test]
    fn test_if_condicao_bool_valida() {
        analisar_ok("var x : int; if (x > 0) { x = 1; }");
    }

    #[test]
    fn test_if_condicao_int_e_erro() {
        let errs = analisar_err("var x : int; if (x) { x = 1; }");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. }
            if message.contains("'if'") && message.contains("'bool'")
        )));
    }

    #[test]
    fn test_while_condicao_bool_valida() {
        analisar_ok("var n : int; while (n > 0) { n = n - 1; }");
    }

    #[test]
    fn test_while_condicao_int_e_erro() {
        let errs = analisar_err("var n : int; while (n) { n = n - 1; }");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. }
            if message.contains("'while'") && message.contains("'bool'")
        )));
    }

    // ── Comando read ──────────────────────────────────────────────────────

    #[test]
    fn test_read_int_valido() {
        analisar_ok("var x : int; read(x);");
    }

    #[test]
    fn test_read_string_valido() {
        analisar_ok("var s : string; read(s);");
    }

    #[test]
    fn test_read_bool_e_erro() {
        let errs = analisar_err("var b : bool; read(b);");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. }
            if message.contains("'read'") && message.contains("'bool'")
        )));
    }

    #[test]
    fn test_read_variavel_nao_declarada() {
        let errs = analisar_err("read(x);");
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'x'")
        )));
    }

    // ── Acumulação de múltiplos erros ─────────────────────────────────────

    #[test]
    fn test_multiplos_erros_acumulados() {
        // Três erros distintos: variável não declarada, tipo errado no if,
        // e tipo errado na atribuição
        let src = r#"
            var x : int;
            if (x) { x = true; }
            x = y + 1;
        "#;
        let errs = analisar_err(src);
        assert!(errs.len() >= 3, "esperados pelo menos 3 erros, obteve {}", errs.len());
    }

    // ── Programas completos válidos ───────────────────────────────────────

    #[test]
    fn test_programa_fatorial_valido() {
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
        analisar_ok(src);
    }

    #[test]
    fn test_programa_if_else_valido() {
        let src = r#"
            var x   : int;
            var par : bool;
            read(x);
            par = false;
            if (x == 0) { par = true; } else { par = false; }
            print(par);
        "#;
        analisar_ok(src);
    }

    #[test]
    fn test_programa_strings_valido() {
        let src = r#"
            var nome     : string;
            var saudacao : string;
            read(nome);
            saudacao = "Olá, " + nome;
            print(saudacao);
        "#;
        analisar_ok(src);
    }

    #[test]
    fn test_programa_expressoes_aninhadas_valido() {
        let src = r#"
            var a : int;
            var b : int;
            var ok : bool;
            read(a);
            read(b);
            ok = (a + b) * 2 > 10 && b != 0;
            print(ok);
        "#;
        analisar_ok(src);
    }

    #[test]
    fn test_programa_erro_semantico_tipo() {
        // Programa dos testes de integração #5: erro de tipo
        let src = r#"
            var x : int;
            var b : bool;
            read(x);
            b = x;
        "#;
        let errs = analisar_err(src);
        assert!(!errs.is_empty());
        assert!(errs.iter().all(|e| matches!(e, PBError::Semantic { .. })));
    }

    #[test]
    fn test_programa_variavel_nao_declarada() {
        // Programa dos testes de integração #6: variável não declarada
        let src = r#"
            var x : int;
            read(x);
            print(resultado);
        "#;
        let errs = analisar_err(src);
        assert!(errs.iter().any(|e| matches!(e,
            PBError::Semantic { message, .. } if message.contains("'resultado'")
        )));
    }
}
