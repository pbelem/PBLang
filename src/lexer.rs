use logos::Logos;

use crate::error::PBError;

// ─────────────────────────────────────────────────────────────────────────────
// Token
// ─────────────────────────────────────────────────────────────────────────────

/// Todos os tokens da linguagem PBLang.
///
/// A macro `#[derive(Logos)]` combina todas as anotações abaixo em um único
/// AFD (Autômato Finito Determinístico) em tempo de compilação, garantindo
/// O(n) no tamanho da entrada.
///
/// Regra de prioridade do logos:
///   1. Tokens mais longos vencem (por isso `==` bate antes de `=`).
///   2. Entre tokens de mesmo comprimento, vence o declarado primeiro —
///      razão pela qual as palavras-chave aparecem ANTES de `Ident`.
#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\r\n\f]+")] // descarta espaços em branco e quebras de linha
#[logos(skip(r"//[^\n]*", allow_greedy = true))] // descarta comentários de linha (// até \n)
pub enum Token {
    // ── Palavras-chave ────────────────────────────────────────────────────
    // Declaradas antes de `Ident` para que "if", "while", etc. nunca
    // sejam reconhecidos como identificadores.
    #[token("var")]
    Var,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("print")]
    Print,
    #[token("read")]
    Read,

    // ── Tipos e literais booleanos ────────────────────────────────────────
    #[token("int")]
    IntType,
    #[token("bool")]
    BoolType,
    #[token("string")]
    StringType,
    #[token("true")]
    True,
    #[token("false")]
    False,

    // ── Identificadores ───────────────────────────────────────────────────
    // Declarado APÓS as palavras-chave; logos prefere o match mais longo,
    // mas para strings de mesmo comprimento a ordem de declaração decide.
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    // ── Literais inteiros ─────────────────────────────────────────────────
    // `.ok()` converte overflow de parse em `None`, que o logos trata como
    // token não reconhecido (erro léxico), evitando panic em números grandes.
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Number(i64),

    // ── Literais string ───────────────────────────────────────────────────
    // Aceita qualquer caractere exceto `"` e quebra de linha dentro das aspas.
    // O closure remove as aspas e retorna apenas o conteúdo interno.
    #[regex(r#""[^"\n]*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len() - 1].to_string())
    })]
    StringLit(String),

    // ── Operadores (dois caracteres declarados antes dos de um) ───────────
    // logos escolhe o match mais longo automaticamente, mas a ordem explícita
    // documenta a intenção e evita surpresas em versões futuras da lib.
    #[token("==")]
    Eq,
    #[token("!=")]
    Neq,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("&&")]
    And,
    #[token("||")]
    Or,

    #[token("=")]
    Assign,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Mod,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("!")]
    Not,

    // ── Pontuação / Delimitadores ─────────────────────────────────────────
    #[token(";")]
    Semicolon,
    #[token(":")]
    Colon,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
}

impl Token {
    /// Nome legível da variante — usado em mensagens de erro do parser.
    /// ("esperado ';', encontrado 'identificador'")
    pub fn kind_name(&self) -> &'static str {
        match self {
            Token::Var => "var",
            Token::If => "if",
            Token::Else => "else",
            Token::While => "while",
            Token::Print => "print",
            Token::Read => "read",
            Token::IntType => "int",
            Token::BoolType => "bool",
            Token::StringType => "string",
            Token::True => "true",
            Token::False => "false",
            Token::Ident(_) => "identificador",
            Token::Number(_) => "literal inteiro",
            Token::StringLit(_) => "literal string",
            Token::Eq => "==",
            Token::Neq => "!=",
            Token::Le => "<=",
            Token::Ge => ">=",
            Token::And => "&&",
            Token::Or => "||",
            Token::Assign => "=",
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Star => "*",
            Token::Slash => "/",
            Token::Mod => "%",
            Token::Lt => "<",
            Token::Gt => ">",
            Token::Not => "!",
            Token::Semicolon => ";",
            Token::Colon => ":",
            Token::LParen => "(",
            Token::RParen => ")",
            Token::LBrace => "{",
            Token::RBrace => "}",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Token enriquecido com posição
// ─────────────────────────────────────────────────────────────────────────────

/// Token com localização no código-fonte.
/// Usado pelo parser para gerar mensagens de erro com linha e coluna precisas.
#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    /// Texto exato como aparece no fonte (e.g. `"while"`, `"42"`, `"x"`).
    pub lexeme: String,
    /// Linha 1-indexada.
    pub line: usize,
    /// Coluna 1-indexada (offset de byte dentro da linha).
    pub col: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Função pública do lexer
// ─────────────────────────────────────────────────────────────────────────────

/// Transforma `source` em um stream de `SpannedToken`.
///
/// Estratégia de erro: acumula **todos** os erros léxicos antes de retornar,
/// para que o programador veja todos os problemas de uma vez.
pub fn lex(source: &str) -> Result<Vec<SpannedToken>, Vec<PBError>> {
    let mut tokens: Vec<SpannedToken> = Vec::new();
    let mut errors: Vec<PBError> = Vec::new();

    let mut lexer = Token::lexer(source);

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        let lexeme = lexer.slice().to_string();
        let (line, col) = byte_offset_to_line_col(source, span.start);

        match result {
            Ok(token) => tokens.push(SpannedToken {
                token,
                lexeme,
                line,
                col,
            }),
            Err(_) => errors.push(PBError::Lexical {
                message: format!("Caractere inesperado: '{}'", lexeme),
                line,
                col,
            }),
        }
    }

    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}

/// Converte offset de byte em (linha, coluna), ambos 1-indexados.
/// Chamada apenas em posições de erro, então O(n) é aceitável.
fn byte_offset_to_line_col(src: &str, offset: usize) -> (usize, usize) {
    let mut line: usize = 1;
    let mut col: usize = 1;
    for (i, ch) in src.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += ch.len_utf8();
        }
    }
    (line, col)
}

// ─────────────────────────────────────────────────────────────────────────────
// Testes
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ───────────────────────────────────────────────────────────

    /// Tokeniza `source` esperando sucesso; falha o teste se houver erros.
    fn tokens_of(source: &str) -> Vec<Token> {
        lex(source)
            .unwrap_or_else(|e| panic!("lex falhou inesperadamente: {:?}", e))
            .into_iter()
            .map(|s| s.token)
            .collect()
    }

    /// Tokeniza `source` esperando falha; retorna os erros.
    fn errors_of(source: &str) -> Vec<PBError> {
        lex(source).expect_err("lex deveria ter falhado mas não falhou")
    }

    // ── Palavras-chave ────────────────────────────────────────────────────

    #[test]
    fn test_palavras_reservadas() {
        let tokens = tokens_of("if else while print read var int bool string true false");
        assert_eq!(
            tokens,
            vec![
                Token::If,
                Token::Else,
                Token::While,
                Token::Print,
                Token::Read,
                Token::Var,
                Token::IntType,
                Token::BoolType,
                Token::StringType,
                Token::True,
                Token::False,
            ]
        );
    }

    #[test]
    fn test_keyword_nao_e_identificador() {
        // "if" deve ser If, não Ident("if")
        assert_eq!(tokens_of("if"), vec![Token::If]);
        assert_eq!(tokens_of("while"), vec![Token::While]);
        assert_eq!(tokens_of("string"), vec![Token::StringType]);
        assert_eq!(tokens_of("var"), vec![Token::Var]);
    }

    #[test]
    fn test_identificador_prefixado_com_keyword_e_ident() {
        // "iff" começa com "if" mas é identificador válido
        assert_eq!(tokens_of("iff"), vec![Token::Ident("iff".into())]);
        assert_eq!(
            tokens_of("whileTrue"),
            vec![Token::Ident("whileTrue".into())]
        );
        assert_eq!(tokens_of("printVal"), vec![Token::Ident("printVal".into())]);
        assert_eq!(tokens_of("variable"), vec![Token::Ident("variable".into())]);
    }

    // ── Identificadores ───────────────────────────────────────────────────

    #[test]
    fn test_identificadores_validos() {
        assert_eq!(
            tokens_of("x _y z123 camelCase _CONST"),
            vec![
                Token::Ident("x".into()),
                Token::Ident("_y".into()),
                Token::Ident("z123".into()),
                Token::Ident("camelCase".into()),
                Token::Ident("_CONST".into()),
            ]
        );
    }

    #[test]
    fn test_identificadores_e_numeros() {
        let tokens = tokens_of("var_1 = 42;");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("var_1".into()),
                Token::Assign,
                Token::Number(42),
                Token::Semicolon,
            ]
        );
    }

    // ── Literais inteiros ─────────────────────────────────────────────────

    #[test]
    fn test_literais_inteiros() {
        assert_eq!(
            tokens_of("0 42 1000 999999"),
            vec![
                Token::Number(0),
                Token::Number(42),
                Token::Number(1000),
                Token::Number(999999),
            ]
        );
    }

    // ── Literais string ───────────────────────────────────────────────────

    #[test]
    fn test_literais_string() {
        assert_eq!(
            tokens_of(r#""hello" "mundo" "123""#),
            vec![
                Token::StringLit("hello".into()),
                Token::StringLit("mundo".into()),
                Token::StringLit("123".into()),
            ]
        );
    }

    #[test]
    fn test_string_vazia() {
        assert_eq!(tokens_of(r#""""#), vec![Token::StringLit("".into())]);
    }

    #[test]
    fn test_string_com_espacos() {
        assert_eq!(
            tokens_of(r#""olá mundo""#),
            vec![Token::StringLit("olá mundo".into())]
        );
    }

    #[test]
    fn test_string_com_quebra_de_linha_e_invalida() {
        // String que cruza uma quebra de linha não deve ser reconhecida
        let resultado = lex("\"abc\ndef\"");
        assert!(
            resultado.is_err(),
            "string multiline deveria produzir erro léxico"
        );
    }

    // ── Operadores ────────────────────────────────────────────────────────

    #[test]
    fn test_operadores_dois_chars() {
        assert_eq!(
            tokens_of("== != <= >= && ||"),
            vec![
                Token::Eq,
                Token::Neq,
                Token::Le,
                Token::Ge,
                Token::And,
                Token::Or
            ]
        );
    }

    #[test]
    fn test_operadores_um_char() {
        assert_eq!(
            tokens_of("+ - * / % < > !"),
            vec![
                Token::Plus,
                Token::Minus,
                Token::Star,
                Token::Slash,
                Token::Mod,
                Token::Lt,
                Token::Gt,
                Token::Not,
            ]
        );
    }

    #[test]
    fn test_assign_distinto_de_igualdade() {
        // `=` é atribuição; `==` é comparação; nunca devem se confundir
        let toks = tokens_of("x = 1 ; x == 1");
        assert_eq!(toks[1], Token::Assign, "= deve ser Assign");
        assert_eq!(toks[5], Token::Eq, "== deve ser Eq");
    }

    #[test]
    fn test_gt_com_espaco_antes_de_assign_nao_e_ge() {
        // `> =` com espaço NÃO é `>=`
        assert_eq!(tokens_of("> ="), vec![Token::Gt, Token::Assign]);
    }

    // ── Pontuação e delimitadores ─────────────────────────────────────────

    #[test]
    fn test_pontuacao_completa() {
        assert_eq!(
            tokens_of("; : ( ) { } ="),
            vec![
                Token::Semicolon,
                Token::Colon,
                Token::LParen,
                Token::RParen,
                Token::LBrace,
                Token::RBrace,
                Token::Assign,
            ]
        );
    }

    // ── Comentários e espaços ─────────────────────────────────────────────

    #[test]
    fn test_comentarios_e_espacos_ignorados() {
        let codigo = "10 // comentario ignorado \n 20";
        assert_eq!(
            tokens_of(codigo),
            vec![Token::Number(10), Token::Number(20)]
        );
    }

    #[test]
    fn test_comentario_no_fim_sem_newline() {
        assert_eq!(tokens_of("42 // fim"), vec![Token::Number(42)]);
    }

    #[test]
    fn test_tabs_e_newlines_ignorados() {
        assert_eq!(
            tokens_of("var\t\tx\n:\nint  ;"),
            vec![
                Token::Var,
                Token::Ident("x".into()),
                Token::Colon,
                Token::IntType,
                Token::Semicolon
            ]
        );
    }

    // ── Localização (linha/coluna) ────────────────────────────────────────

    #[test]
    fn test_linha_e_coluna_corretos() {
        let src = "var x : int ;\ny = 42 ;";
        let spanned = lex(src).unwrap();

        // var  → linha 1, col 1
        assert_eq!((spanned[0].line, spanned[0].col), (1, 1), "var");
        // x    → linha 1, col 5
        assert_eq!((spanned[1].line, spanned[1].col), (1, 5), "x");
        // y    → linha 2, col 1
        assert_eq!((spanned[5].line, spanned[5].col), (2, 1), "y");
        // =    → linha 2, col 3
        assert_eq!((spanned[6].line, spanned[6].col), (2, 3), "=");
        // 42   → linha 2, col 5
        assert_eq!((spanned[7].line, spanned[7].col), (2, 5), "42");
    }

    // ── Casos de erro ─────────────────────────────────────────────────────

    #[test]
    fn test_caractere_invalido() {
        // '@' não pertence ao alfabeto da linguagem
        let errs = errors_of("int x = 10 @ 5;");
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            PBError::Lexical { message, .. } => assert!(message.contains('@')),
            _ => panic!("esperado PBError::Lexical"),
        }
    }

    #[test]
    fn test_erro_tem_posicao_correta() {
        // '@' está na linha 2, coluna 5 ("    @")
        let errs = errors_of("var x\n    @ int ;");
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            PBError::Lexical { line, col, .. } => {
                assert_eq!(*line, 2, "linha errada");
                assert_eq!(*col, 5, "coluna errada");
            }
            _ => panic!("esperado PBError::Lexical"),
        }
    }

    #[test]
    fn test_multiplos_erros_acumulados() {
        // Todos os três caracteres inválidos devem ser reportados de uma vez
        let errs = errors_of("@ # $");
        assert_eq!(
            errs.len(),
            3,
            "esperados 3 erros, encontrados {}",
            errs.len()
        );
    }

    // ── Programas completos ───────────────────────────────────────────────

    #[test]
    fn test_programa_fatorial() {
        let src = r#"
            var n         : int;
            var resultado : int;
            // lê n do usuário
            read(n);
            resultado = 1;
            while (n > 1) {
                resultado = resultado * n;
                n = n - 1;
            }
            print(resultado);
        "#;
        let toks = tokens_of(src);
        assert!(toks.contains(&Token::Var));
        assert!(toks.contains(&Token::While));
        assert!(toks.contains(&Token::Print));
        assert!(toks.contains(&Token::Read));
        assert!(toks.contains(&Token::Star));
        assert!(toks.contains(&Token::Gt));
        assert!(toks.contains(&Token::Number(1)));
        assert!(toks.contains(&Token::IntType));
    }

    #[test]
    fn test_programa_if_else() {
        let src = r#"
            var x   : int;
            var par : bool;
            read(x);
            if (x == 0) {
                par = true;
            } else {
                par = false;
            }
            print(par);
        "#;
        let toks = tokens_of(src);
        assert!(toks.contains(&Token::If));
        assert!(toks.contains(&Token::Else));
        assert!(toks.contains(&Token::True));
        assert!(toks.contains(&Token::False));
        assert!(toks.contains(&Token::Eq));
        assert!(toks.contains(&Token::BoolType));
    }

    #[test]
    fn test_programa_com_strings() {
        let src = r#"
            var nome     : string;
            var saudacao : string;
            read(nome);
            saudacao = "Olá, ";
            print(saudacao);
        "#;
        let toks = tokens_of(src);
        assert!(toks.contains(&Token::StringType));
        assert!(toks.contains(&Token::StringLit("Olá, ".into())));
        assert!(toks.contains(&Token::Read));
        assert!(toks.contains(&Token::Print));
    }

    #[test]
    fn test_programa_expressoes_logicas() {
        let src = "if (x > 0 && y < 10 || !flag) { x = 1; }";
        let toks = tokens_of(src);
        assert!(toks.contains(&Token::And));
        assert!(toks.contains(&Token::Or));
        assert!(toks.contains(&Token::Not));
        assert!(toks.contains(&Token::Gt));
        assert!(toks.contains(&Token::Lt));
    }

    #[test]
    fn test_programa_com_erro_lexico() {
        let src = r#"
            var x : int;
            x = 10 @ 5;
            print(x#);
        "#;
        let errs = errors_of(src);
        // Dois caracteres inválidos: '@' e '#'
        assert_eq!(errs.len(), 2, "esperados 2 erros léxicos");
        assert!(errs.iter().all(|e| matches!(e, PBError::Lexical { .. })));
    }
}
