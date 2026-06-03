use crate::ast::*;
use crate::error::PBError;
use crate::lexer::{SpannedToken, Token};

// ─────────────────────────────────────────────────────────────────────────────
// Parser
// ─────────────────────────────────────────────────────────────────────────────

/// Parser recursivo descendente para a gramática PBLang.
///
/// Consome um `Vec<SpannedToken>` produzido pelo lexer e constrói
/// uma `Programa` (raiz da AST) ou acumula erros sintáticos.
///
/// Estratégia de recuperação de erros: ao encontrar um erro em um comando,
/// o parser avança tokens até encontrar `;` ou `}` (pontos de sincronização)
/// antes de tentar parsear o próximo comando — assim vários erros são
/// reportados numa única execução.
pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    errors: Vec<PBError>,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self { tokens, pos: 0, errors: Vec::new() }
    }

    // ── Interface pública ─────────────────────────────────────────────────

    /// Ponto de entrada: parseia o programa inteiro.
    ///
    /// Retorna `Ok(Programa)` se não houver erros, ou `Err(erros)` com todos
    /// os erros sintáticos encontrados.
    pub fn parse(mut self) -> Result<Programa, Vec<PBError>> {
        let prog = self.parse_programa();
        if self.errors.is_empty() {
            Ok(prog)
        } else {
            Err(self.errors)
        }
    }

    // ── Navegação no stream de tokens ─────────────────────────────────────

    /// Token atual sem consumir.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|s| &s.token)
    }

    /// `SpannedToken` atual (inclui linha/coluna).
    fn peek_spanned(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.pos)
    }

    /// Consome e retorna o token atual.
    fn advance(&mut self) -> Option<&SpannedToken> {
        let t = self.tokens.get(self.pos);
        if t.is_some() { self.pos += 1; }
        t
    }

    /// Linha e coluna do token atual (para erros).
    fn current_pos(&self) -> (usize, usize) {
        self.peek_spanned()
            .map(|s| (s.line, s.col))
            .unwrap_or((0, 0))
    }

    /// Consome o token atual se ele for `expected`; caso contrário,
    /// registra um erro e retorna `false`.
    fn expect(&mut self, expected: &Token) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            let (line, col) = self.current_pos();
            let found = self.peek()
                .map(|t| format!("'{}'", t.kind_name()))
                .unwrap_or_else(|| "fim de arquivo".into());
            self.errors.push(PBError::Syntactic {
                message: format!(
                    "Esperado '{}', encontrado {}",
                    expected.kind_name(),
                    found
                ),
                line,
                col,
            });
            false
        }
    }

    /// Consome um `Ident` e retorna seu nome; registra erro se não for Ident.
    fn expect_ident(&mut self) -> Option<(String, Span)> {
        let (line, col) = self.current_pos();
        if let Some(Token::Ident(name)) = self.peek() {
            let name = name.clone();
            self.advance();
            Some((name, Span::new(line, col)))
        } else {
            let found = self.peek()
                .map(|t| format!("'{}'", t.kind_name()))
                .unwrap_or_else(|| "fim de arquivo".into());
            self.errors.push(PBError::Syntactic {
                message: format!("Esperado identificador, encontrado {}", found),
                line,
                col,
            });
            None
        }
    }

    /// Avança até um ponto de sincronização (`;` ou `}`), para recuperação
    /// de erros em comandos mal-formados.
    fn sync(&mut self) {
        while let Some(tok) = self.peek() {
            match tok {
                Token::Semicolon | Token::RBrace => { self.advance(); return; }
                _ => { self.advance(); }
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Gramática — Programa
    // ─────────────────────────────────────────────────────────────────────

    /// `programa → declaração* comando*`
    fn parse_programa(&mut self) -> Programa {
        let mut declaracoes = Vec::new();
        let mut comandos    = Vec::new();

        // Declarações começam com `var`
        while self.peek() == Some(&Token::Var) {
            if let Some(decl) = self.parse_declaracao() {
                declaracoes.push(decl);
            }
        }

        // Comandos: tudo o que restar até o fim do arquivo
        while self.peek().is_some() {
            match self.parse_comando() {
                Some(cmd) => comandos.push(cmd),
                None      => self.sync(),
            }
        }

        Programa { declaracoes, comandos }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Gramática — Declaração
    // ─────────────────────────────────────────────────────────────────────

    /// `declaração → 'var' ID ':' tipo ';'`
    fn parse_declaracao(&mut self) -> Option<Declaracao> {
        let (_line, _col) = self.current_pos();
        self.expect(&Token::Var);                          // 'var'

        let (nome, span) = self.expect_ident()?;           // ID
        self.expect(&Token::Colon);                        // ':'
        let tipo = self.parse_tipo()?;                     // tipo
        self.expect(&Token::Semicolon);                    // ';'

        Some(Declaracao { nome, tipo, span })
    }

    /// `tipo → 'int' | 'bool' | 'string'`
    fn parse_tipo(&mut self) -> Option<Tipo> {
        let (line, col) = self.current_pos();
        match self.peek() {
            Some(Token::IntType)    => { self.advance(); Some(Tipo::Int)    }
            Some(Token::BoolType)   => { self.advance(); Some(Tipo::Bool)   }
            Some(Token::StringType) => { self.advance(); Some(Tipo::String) }
            _ => {
                let found = self.peek()
                    .map(|t| format!("'{}'", t.kind_name()))
                    .unwrap_or_else(|| "fim de arquivo".into());
                self.errors.push(PBError::Syntactic {
                    message: format!("Esperado tipo ('int', 'bool' ou 'string'), encontrado {}", found),
                    line,
                    col,
                });
                None
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Gramática — Comandos
    // ─────────────────────────────────────────────────────────────────────

    /// `comando → atribuição | stmt_if | stmt_while | stmt_print | stmt_read | bloco`
    fn parse_comando(&mut self) -> Option<Comando> {
        match self.peek()? {
            Token::If         => self.parse_if(),
            Token::While      => self.parse_while(),
            Token::Print      => self.parse_print(),
            Token::Read       => self.parse_read(),
            Token::LBrace     => self.parse_bloco().map(Comando::Bloco),
            Token::Ident(_)   => self.parse_atribuicao(),
            _ => {
                let (line, col) = self.current_pos();
                let found = self.peek()
                    .map(|t| format!("'{}'", t.kind_name()))
                    .unwrap_or_else(|| "fim de arquivo".into());
                self.errors.push(PBError::Syntactic {
                    message: format!("Comando inesperado: {}", found),
                    line,
                    col,
                });
                None
            }
        }
    }

    /// `atribuição → ID '=' expressão ';'`
    fn parse_atribuicao(&mut self) -> Option<Comando> {
        let (nome, span) = self.expect_ident()?;
        self.expect(&Token::Assign);
        let expr = self.parse_expressao()?;
        self.expect(&Token::Semicolon);
        Some(Comando::Atribuicao { nome, expr, span })
    }

    /// `stmt_if → 'if' '(' expressão ')' bloco ('else' bloco)?`
    fn parse_if(&mut self) -> Option<Comando> {
        let (line, col) = self.current_pos();
        self.expect(&Token::If);
        self.expect(&Token::LParen);
        let condicao = self.parse_expressao()?;
        self.expect(&Token::RParen);
        let entao = self.parse_bloco()?;

        let senao = if self.peek() == Some(&Token::Else) {
            self.advance();
            self.parse_bloco()
        } else {
            None
        };

        Some(Comando::If {
            condicao,
            entao,
            senao,
            span: Span::new(line, col),
        })
    }

    /// `stmt_while → 'while' '(' expressão ')' bloco`
    fn parse_while(&mut self) -> Option<Comando> {
        let (line, col) = self.current_pos();
        self.expect(&Token::While);
        self.expect(&Token::LParen);
        let condicao = self.parse_expressao()?;
        self.expect(&Token::RParen);
        let corpo = self.parse_bloco()?;

        Some(Comando::While {
            condicao,
            corpo,
            span: Span::new(line, col),
        })
    }

    /// `stmt_print → 'print' '(' expressão ')' ';'`
    fn parse_print(&mut self) -> Option<Comando> {
        let (line, col) = self.current_pos();
        self.expect(&Token::Print);
        self.expect(&Token::LParen);
        let expr = self.parse_expressao()?;
        self.expect(&Token::RParen);
        self.expect(&Token::Semicolon);
        Some(Comando::Print { expr, span: Span::new(line, col) })
    }

    /// `stmt_read → 'read' '(' ID ')' ';'`
    fn parse_read(&mut self) -> Option<Comando> {
        let (line, col) = self.current_pos();
        self.expect(&Token::Read);
        self.expect(&Token::LParen);
        let (nome, _) = self.expect_ident()?;
        self.expect(&Token::RParen);
        self.expect(&Token::Semicolon);
        Some(Comando::Read { nome, span: Span::new(line, col) })
    }

    /// `bloco → '{' comando* '}'`
    fn parse_bloco(&mut self) -> Option<Vec<Comando>> {
        self.expect(&Token::LBrace);
        let mut cmds = Vec::new();
        while self.peek().is_some() && self.peek() != Some(&Token::RBrace) {
            match self.parse_comando() {
                Some(cmd) => cmds.push(cmd),
                None      => self.sync(),
            }
        }
        self.expect(&Token::RBrace);
        Some(cmds)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Gramática — Expressões (7 níveis de precedência)
    //
    // Cada função chama a do nível imediatamente acima (maior precedência),
    // formando a cadeia:
    //   expressao → exp_or → exp_and → exp_igualdade → exp_relacional
    //             → exp_aditiva → exp_multiplicativa → exp_unaria → primario
    // ─────────────────────────────────────────────────────────────────────

    /// `expressão → exp_or`
    fn parse_expressao(&mut self) -> Option<Expressao> {
        self.parse_exp_or()
    }

    /// `exp_or → exp_and ('||' exp_and)*`
    fn parse_exp_or(&mut self) -> Option<Expressao> {
        let mut esq = self.parse_exp_and()?;
        while self.peek() == Some(&Token::Or) {
            let (line, col) = self.current_pos();
            self.advance();
            let dir = self.parse_exp_and()?;
            esq = Expressao::BinOp {
                op: OpBin::Or,
                esq: Box::new(esq),
                dir: Box::new(dir),
                span: Span::new(line, col),
            };
        }
        Some(esq)
    }

    /// `exp_and → exp_igualdade ('&&' exp_igualdade)*`
    fn parse_exp_and(&mut self) -> Option<Expressao> {
        let mut esq = self.parse_exp_igualdade()?;
        while self.peek() == Some(&Token::And) {
            let (line, col) = self.current_pos();
            self.advance();
            let dir = self.parse_exp_igualdade()?;
            esq = Expressao::BinOp {
                op: OpBin::And,
                esq: Box::new(esq),
                dir: Box::new(dir),
                span: Span::new(line, col),
            };
        }
        Some(esq)
    }

    /// `exp_igualdade → exp_relacional (('==' | '!=') exp_relacional)*`
    fn parse_exp_igualdade(&mut self) -> Option<Expressao> {
        let mut esq = self.parse_exp_relacional()?;
        loop {
            let (line, col) = self.current_pos();
            let op = match self.peek() {
                Some(Token::Eq)  => OpBin::Eq,
                Some(Token::Neq) => OpBin::Neq,
                _ => break,
            };
            self.advance();
            let dir = self.parse_exp_relacional()?;
            esq = Expressao::BinOp {
                op,
                esq: Box::new(esq),
                dir: Box::new(dir),
                span: Span::new(line, col),
            };
        }
        Some(esq)
    }

    /// `exp_relacional → exp_aditiva (('>' | '<' | '>=' | '<=') exp_aditiva)*`
    fn parse_exp_relacional(&mut self) -> Option<Expressao> {
        let mut esq = self.parse_exp_aditiva()?;
        loop {
            let (line, col) = self.current_pos();
            let op = match self.peek() {
                Some(Token::Gt) => OpBin::Gt,
                Some(Token::Lt) => OpBin::Lt,
                Some(Token::Ge) => OpBin::Ge,
                Some(Token::Le) => OpBin::Le,
                _ => break,
            };
            self.advance();
            let dir = self.parse_exp_aditiva()?;
            esq = Expressao::BinOp {
                op,
                esq: Box::new(esq),
                dir: Box::new(dir),
                span: Span::new(line, col),
            };
        }
        Some(esq)
    }

    /// `exp_aditiva → exp_multiplicativa (('+' | '-') exp_multiplicativa)*`
    fn parse_exp_aditiva(&mut self) -> Option<Expressao> {
        let mut esq = self.parse_exp_multiplicativa()?;
        loop {
            let (line, col) = self.current_pos();
            let op = match self.peek() {
                Some(Token::Plus)  => OpBin::Add,
                Some(Token::Minus) => OpBin::Sub,
                _ => break,
            };
            self.advance();
            let dir = self.parse_exp_multiplicativa()?;
            esq = Expressao::BinOp {
                op,
                esq: Box::new(esq),
                dir: Box::new(dir),
                span: Span::new(line, col),
            };
        }
        Some(esq)
    }

    /// `exp_multiplicativa → exp_unaria (('*' | '/' | '%') exp_unaria)*`
    fn parse_exp_multiplicativa(&mut self) -> Option<Expressao> {
        let mut esq = self.parse_exp_unaria()?;
        loop {
            let (line, col) = self.current_pos();
            let op = match self.peek() {
                Some(Token::Star)    => OpBin::Mul,
                Some(Token::Slash)   => OpBin::Div,
                Some(Token::Mod)     => OpBin::Mod,
                _ => break,
            };
            self.advance();
            let dir = self.parse_exp_unaria()?;
            esq = Expressao::BinOp {
                op,
                esq: Box::new(esq),
                dir: Box::new(dir),
                span: Span::new(line, col),
            };
        }
        Some(esq)
    }

    /// `exp_unaria → ('!' | '-') exp_unaria | primário`
    fn parse_exp_unaria(&mut self) -> Option<Expressao> {
        let (line, col) = self.current_pos();
        match self.peek() {
            Some(Token::Not) => {
                self.advance();
                let operando = self.parse_exp_unaria()?;
                Some(Expressao::UnOp {
                    op: OpUn::Not,
                    operando: Box::new(operando),
                    span: Span::new(line, col),
                })
            }
            Some(Token::Minus) => {
                self.advance();
                let operando = self.parse_exp_unaria()?;
                Some(Expressao::UnOp {
                    op: OpUn::Neg,
                    operando: Box::new(operando),
                    span: Span::new(line, col),
                })
            }
            _ => self.parse_primario(),
        }
    }

    /// `primário → INTEIRO | 'true' | 'false' | STRING | ID | '(' expressão ')'`
    fn parse_primario(&mut self) -> Option<Expressao> {
        let (line, col) = self.current_pos();
        match self.peek()? {
            Token::Number(_) => {
                if let Some(Token::Number(n)) = self.peek().cloned() {
                    self.advance();
                    Some(Expressao::LitInt(n, Span::new(line, col)))
                } else { unreachable!() }
            }
            Token::True => {
                self.advance();
                Some(Expressao::LitBool(true, Span::new(line, col)))
            }
            Token::False => {
                self.advance();
                Some(Expressao::LitBool(false, Span::new(line, col)))
            }
            Token::StringLit(_) => {
                if let Some(Token::StringLit(s)) = self.peek().cloned() {
                    self.advance();
                    Some(Expressao::LitString(s, Span::new(line, col)))
                } else { unreachable!() }
            }
            Token::Ident(_) => {
                if let Some(Token::Ident(name)) = self.peek().cloned() {
                    self.advance();
                    Some(Expressao::Var(name, Span::new(line, col)))
                } else { unreachable!() }
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expressao()?;
                self.expect(&Token::RParen);
                Some(expr)
            }
            _ => {
                let found = self.peek()
                    .map(|t| format!("'{}'", t.kind_name()))
                    .unwrap_or_else(|| "fim de arquivo".into());
                self.errors.push(PBError::Syntactic {
                    message: format!("Expressão inválida: encontrado {}", found),
                    line,
                    col,
                });
                None
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Ponto de entrada público
// ─────────────────────────────────────────────────────────────────────────────

/// Parseia `tokens` e retorna a AST ou lista de erros sintáticos.
pub fn parse(tokens: Vec<SpannedToken>) -> Result<Programa, Vec<PBError>> {
    Parser::new(tokens).parse()
}

// ─────────────────────────────────────────────────────────────────────────────
// Testes
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    /// Executa lexer + parser sobre `src`, esperando sucesso.
    fn parse_ok(src: &str) -> Programa {
        let tokens = lexer::lex(src)
            .unwrap_or_else(|e| panic!("erro léxico inesperado: {:?}", e));
        parse(tokens)
            .unwrap_or_else(|e| panic!("erro sintático inesperado: {:?}", e))
    }

    /// Executa lexer + parser sobre `src`, esperando falha sintática.
    fn parse_err(src: &str) -> Vec<PBError> {
        let tokens = lexer::lex(src)
            .unwrap_or_else(|e| panic!("erro léxico inesperado: {:?}", e));
        parse(tokens).expect_err("parse deveria ter falhado")
    }

    // ── Declarações ───────────────────────────────────────────────────────

    #[test]
    fn test_declaracao_int() {
        let prog = parse_ok("var x : int;");
        assert_eq!(prog.declaracoes.len(), 1);
        assert_eq!(prog.declaracoes[0].nome, "x");
        assert_eq!(prog.declaracoes[0].tipo, Tipo::Int);
    }

    #[test]
    fn test_declaracao_bool() {
        let prog = parse_ok("var flag : bool;");
        assert_eq!(prog.declaracoes[0].tipo, Tipo::Bool);
    }

    #[test]
    fn test_declaracao_string() {
        let prog = parse_ok("var nome : string;");
        assert_eq!(prog.declaracoes[0].tipo, Tipo::String);
    }

    #[test]
    fn test_multiplas_declaracoes() {
        let prog = parse_ok("var a : int; var b : bool; var c : string;");
        assert_eq!(prog.declaracoes.len(), 3);
    }

    // ── Atribuição ────────────────────────────────────────────────────────

    #[test]
    fn test_atribuicao_literal_int() {
        let prog = parse_ok("var x : int; x = 42;");
        assert!(matches!(&prog.comandos[0],
            Comando::Atribuicao { nome, expr: Expressao::LitInt(42, _), .. }
            if nome == "x"
        ));
    }

    #[test]
    fn test_atribuicao_literal_bool() {
        let prog = parse_ok("var b : bool; b = true;");
        assert!(matches!(&prog.comandos[0],
            Comando::Atribuicao { expr: Expressao::LitBool(true, _), .. }
        ));
    }

    #[test]
    fn test_atribuicao_literal_string() {
        let prog = parse_ok(r#"var s : string; s = "olá";"#);
        assert!(matches!(&prog.comandos[0],
            Comando::Atribuicao { expr: Expressao::LitString(s, _), .. }
            if s == "olá"
        ));
    }

    // ── Estruturas de controle ────────────────────────────────────────────

    #[test]
    fn test_if_simples() {
        let prog = parse_ok("var x : int; if (x > 0) { x = 1; }");
        assert!(matches!(&prog.comandos[0],
            Comando::If { senao: None, .. }
        ));
    }

    #[test]
    fn test_if_else() {
        let prog = parse_ok("var x : int; if (x > 0) { x = 1; } else { x = 0; }");
        assert!(matches!(&prog.comandos[0],
            Comando::If { senao: Some(_), .. }
        ));
    }

    #[test]
    fn test_while() {
        let prog = parse_ok("var n : int; while (n > 0) { n = n - 1; }");
        assert!(matches!(&prog.comandos[0], Comando::While { .. }));
    }

    #[test]
    fn test_print() {
        let prog = parse_ok("var x : int; print(x);");
        assert!(matches!(&prog.comandos[0], Comando::Print { .. }));
    }

    #[test]
    fn test_read() {
        let prog = parse_ok("var x : int; read(x);");
        assert!(matches!(&prog.comandos[0],
            Comando::Read { nome, .. } if nome == "x"
        ));
    }

    // ── Expressões e precedência ──────────────────────────────────────────

    #[test]
    fn test_precedencia_multiplicacao_antes_de_adicao() {
        // 2 + 3 * 4 deve ser parseado como 2 + (3 * 4)
        let prog = parse_ok("var x : int; x = 2 + 3 * 4;");
        let Comando::Atribuicao { expr, .. } = &prog.comandos[0] else { panic!() };

        // raiz deve ser Add
        let Expressao::BinOp { op: OpBin::Add, dir, .. } = expr else {
            panic!("raiz deveria ser Add, obteve: {:?}", expr)
        };
        // direito de Add deve ser Mul
        assert!(matches!(dir.as_ref(), Expressao::BinOp { op: OpBin::Mul, .. }),
            "direito de Add deveria ser Mul");
    }

    #[test]
    fn test_precedencia_parenteses_sobrepoe_multiplicacao() {
        // (2 + 3) * 4 — Add deve ser filho esquerdo de Mul
        let prog = parse_ok("var x : int; x = (2 + 3) * 4;");
        let Comando::Atribuicao { expr, .. } = &prog.comandos[0] else { panic!() };

        let Expressao::BinOp { op: OpBin::Mul, esq, .. } = expr else {
            panic!("raiz deveria ser Mul")
        };
        assert!(matches!(esq.as_ref(), Expressao::BinOp { op: OpBin::Add, .. }));
    }

    #[test]
    fn test_precedencia_and_antes_de_or() {
        // a || b && c → a || (b && c)
        let prog = parse_ok("var x : bool; x = true || false && true;");
        let Comando::Atribuicao { expr, .. } = &prog.comandos[0] else { panic!() };

        let Expressao::BinOp { op: OpBin::Or, dir, .. } = expr else {
            panic!("raiz deveria ser Or")
        };
        assert!(matches!(dir.as_ref(), Expressao::BinOp { op: OpBin::And, .. }));
    }

    #[test]
    fn test_unario_negacao() {
        let prog = parse_ok("var x : int; x = -42;");
        let Comando::Atribuicao { expr, .. } = &prog.comandos[0] else { panic!() };
        assert!(matches!(expr, Expressao::UnOp { op: OpUn::Neg, .. }));
    }

    #[test]
    fn test_unario_not() {
        let prog = parse_ok("var b : bool; b = !true;");
        let Comando::Atribuicao { expr, .. } = &prog.comandos[0] else { panic!() };
        assert!(matches!(expr, Expressao::UnOp { op: OpUn::Not, .. }));
    }

    // ── Programas completos ───────────────────────────────────────────────

    #[test]
    fn test_programa_fatorial() {
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
        let prog = parse_ok(src);
        assert_eq!(prog.declaracoes.len(), 2);
        assert_eq!(prog.comandos.len(), 4); // read, atrib, while, print
    }

    #[test]
    fn test_programa_if_else_completo() {
        let src = r#"
            var x   : int;
            var par : bool;
            read(x);
            par = false;
            if (x == 0) {
                par = true;
            } else {
                par = false;
            }
            print(par);
        "#;
        let prog = parse_ok(src);
        assert_eq!(prog.declaracoes.len(), 2);
        assert_eq!(prog.comandos.len(), 4); // read, atrib, if, print
        assert!(matches!(&prog.comandos[2], Comando::If { senao: Some(_), .. }));
    }

    #[test]
    fn test_programa_com_strings() {
        let src = r#"
            var nome     : string;
            var saudacao : string;
            read(nome);
            saudacao = "Olá";
            print(saudacao);
        "#;
        let prog = parse_ok(src);
        assert_eq!(prog.declaracoes.len(), 2);
        assert_eq!(prog.comandos.len(), 3);
    }

    #[test]
    fn test_programa_expressoes_aninhadas() {
        let src = r#"
            var a : int;
            var b : int;
            var c : bool;
            read(a);
            read(b);
            c = (a + b) * 2 > 10 && b != 0;
            print(c);
        "#;
        let prog = parse_ok(src);
        assert_eq!(prog.declaracoes.len(), 3);
    }

    #[test]
    fn test_programa_while_aninhado() {
        let src = r#"
            var i : int;
            var j : int;
            i = 0;
            while (i < 3) {
                j = 0;
                while (j < 3) {
                    j = j + 1;
                }
                i = i + 1;
            }
        "#;
        let prog = parse_ok(src);
        // O programa tem exatamente 2 comandos: i=0 (índice 0) e while (índice 1)
        assert_eq!(prog.comandos.len(), 2);
        assert!(matches!(&prog.comandos[1], Comando::While { .. }));
    }

    // ── Erros sintáticos ──────────────────────────────────────────────────

    #[test]
    fn test_erro_falta_ponto_e_virgula() {
        // `x = 1` sem `;` — deve reportar erro
        let errs = parse_err("var x : int; x = 1");
        assert!(!errs.is_empty());
        assert!(errs.iter().any(|e| matches!(e, PBError::Syntactic { .. })));
    }

    #[test]
    fn test_erro_falta_fecha_paren_no_if() {
        let errs = parse_err("var x : int; if (x > 0 { x = 1; }");
        assert!(errs.iter().any(|e| matches!(e, PBError::Syntactic { .. })));
    }

    #[test]
    fn test_erro_tipo_invalido() {
        let errs = parse_err("var x : float;");
        assert!(errs.iter().any(|e| {
            matches!(e, PBError::Syntactic { message, .. } if message.contains("tipo"))
        }));
    }
}
