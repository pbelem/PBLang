/// Erros unificados de todas as fases do compilador.
///
/// Cada variante carrega localização (linha/coluna) e uma mensagem legível.
/// As fases acumulam erros em `Vec<PBError>` em vez de parar no primeiro,
/// permitindo que o programador veja todos os problemas de uma só vez.
#[derive(Debug, Clone, PartialEq)]
pub enum PBError {
    Lexical {
        message: String,
        line: usize,
        col: usize,
    },
    Syntactic {
        message: String,
        line: usize,
        col: usize,
    },
    Semantic {
        message: String,
        line: usize,
        col: usize,
    },
}

impl std::fmt::Display for PBError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PBError::Lexical { message, line, col } =>
                write!(f, "[ERRO LÉXICO]    Linha {:3}, Col {:3}  {}", line, col, message),
            PBError::Syntactic { message, line, col } =>
                write!(f, "[ERRO SINTÁTICO] Linha {:3}, Col {:3}  {}", line, col, message),
            PBError::Semantic { message, line, col } =>
                write!(f, "[ERRO SEMÂNTICO] Linha {:3}, Col {:3}  {}", line, col, message),
        }
    }
}

impl std::error::Error for PBError {}

/// Alias: valor `T` ou lista de erros acumulados.
pub type PBResult<T> = Result<T, Vec<PBError>>;
