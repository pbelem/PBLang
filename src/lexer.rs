use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\f]+")] // Delega a ignorância de espaços para o nível do enum
#[logos(skip(r"//.*", allow_greedy = true))]   // Delega os comentários
pub enum Token {
    // Palavras reservadas
    #[token("if")] If,
    #[token("else")] Else,
    #[token("while")] While,
    #[token("print")] Print,
    #[token("read")] Read,
    
    // Tipos e Literais Booleanos
    #[token("int")] IntType,
    #[token("bool")] BoolType,
    #[token("true")] True,
    #[token("false")] False,
    
    // Identificadores
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())] 
    Ident(String),
    
    // Números (O .ok() transforma o erro de parsing em um Token não reconhecido,
    // evitando o panic do unwrap se o número for grande demais)
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())] 
    Number(i64),
    
    // Operadores
    #[token("=")] Assign,
    #[token("+")] Plus, 
    #[token("-")] Minus,
    #[token("*")] Star, 
    #[token("/")] Slash,
    #[token("%")] Mod,
    #[token("==")] Eq, 
    #[token("!=")] Neq,
    #[token("<")] Lt, 
    #[token(">")] Gt,
    #[token("<=")] Le, 
    #[token(">=")] Ge,
    #[token("&&")] And, 
    #[token("||")] Or,
    #[token("!")] Not,
    
    // Pontuação
    #[token(";")] Semicolon,
    #[token("(")] LParen, 
    #[token(")")] RParen,
    #[token("{")] LBrace, 
    #[token("}")] RBrace,
}

#[cfg(test)]
mod tests {
    use super::*;
    use logos::Logos;

    fn lex(source: &str) -> Vec<Result<Token, ()>> {
        Token::lexer(source).collect()
    }

    #[test]
    fn test_palavras_reservadas() {
        let tokens = lex("if else while print read int bool true false");
        assert_eq!(
            tokens,
            vec![
                Ok(Token::If), Ok(Token::Else), Ok(Token::While),
                Ok(Token::Print), Ok(Token::Read), Ok(Token::IntType),
                Ok(Token::BoolType), Ok(Token::True), Ok(Token::False),
            ]
        );
    }

    #[test]
    fn test_identificadores_e_numeros() {
        let tokens = lex("var_1 = 42;");
        assert_eq!(
            tokens,
            vec![
                Ok(Token::Ident("var_1".to_string())),
                Ok(Token::Assign),
                Ok(Token::Number(42)),
                Ok(Token::Semicolon),
            ]
        );
    }

    #[test]
    fn test_operadores() {
        let tokens = lex("+ - * / % == != < > <= >= && || !");
        assert_eq!(
            tokens,
            vec![
                Ok(Token::Plus), Ok(Token::Minus), Ok(Token::Star),
                Ok(Token::Slash), Ok(Token::Mod), Ok(Token::Eq),
                Ok(Token::Neq), Ok(Token::Lt), Ok(Token::Gt),
                Ok(Token::Le), Ok(Token::Ge), Ok(Token::And),
                Ok(Token::Or), Ok(Token::Not),
            ]
        );
    }

    #[test]
    fn test_comentarios_e_espacos_ignorados() {
        let codigo = "10 // comentario ignorado \n 20";
        let tokens = lex(codigo);
        assert_eq!(
            tokens,
            vec![
                Ok(Token::Number(10)),
                Ok(Token::Number(20)),
            ]
        );
    }

    #[test]
    fn test_caractere_invalido() {
        let tokens = lex("int x = 10 @ 5;");
        assert_eq!(
            tokens,
            vec![
                Ok(Token::IntType),
                Ok(Token::Ident("x".to_string())),
                Ok(Token::Assign),
                Ok(Token::Number(10)),
                Err(()), // O caractere '@' não faz parte do alfabeto da linguagem
                Ok(Token::Number(5)),
                Ok(Token::Semicolon),
            ]
        );
    }
}
