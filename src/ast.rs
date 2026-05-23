/// Localização no código-fonte, carregada por todos os nós da AST.
/// Permite que erros semânticos e de codegen apontem para a linha/coluna exata.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipos
// ─────────────────────────────────────────────────────────────────────────────

/// Tipos primitivos suportados pela linguagem.
#[derive(Debug, Clone, PartialEq)]
pub enum Tipo {
    Int,
    Bool,
    String,
}

impl std::fmt::Display for Tipo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tipo::Int    => write!(f, "int"),
            Tipo::Bool   => write!(f, "bool"),
            Tipo::String => write!(f, "string"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Programa
// ─────────────────────────────────────────────────────────────────────────────

/// Raiz da AST: lista de declarações seguida de lista de comandos.
///
/// Gramática: `programa → declaração* comando*`
#[derive(Debug, Clone, PartialEq)]
pub struct Programa {
    pub declaracoes: Vec<Declaracao>,
    pub comandos: Vec<Comando>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Declarações
// ─────────────────────────────────────────────────────────────────────────────

/// `var ID : tipo ;`
#[derive(Debug, Clone, PartialEq)]
pub struct Declaracao {
    pub nome: String,
    pub tipo: Tipo,
    pub span: Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Comandos
// ─────────────────────────────────────────────────────────────────────────────

/// Nó de comando — corresponde diretamente à regra `comando` da gramática.
#[derive(Debug, Clone, PartialEq)]
pub enum Comando {
    /// `ID = expressão ;`
    Atribuicao {
        nome: String,
        expr: Expressao,
        span: Span,
    },
    /// `if ( expressão ) bloco ( else bloco )?`
    If {
        condicao: Expressao,
        entao: Vec<Comando>,
        senao: Option<Vec<Comando>>,
        span: Span,
    },
    /// `while ( expressão ) bloco`
    While {
        condicao: Expressao,
        corpo: Vec<Comando>,
        span: Span,
    },
    /// `print ( expressão ) ;`
    Print {
        expr: Expressao,
        span: Span,
    },
    /// `read ( ID ) ;`
    Read {
        nome: String,
        span: Span,
    },
    /// `{ comando* }` — bloco aninhado, tratado como comando composto.
    Bloco(Vec<Comando>),
}

// ─────────────────────────────────────────────────────────────────────────────
// Expressões
// ─────────────────────────────────────────────────────────────────────────────

/// Nó de expressão.
///
/// A precedência de operadores é capturada pela estrutura da árvore:
/// operadores de menor precedência ficam mais próximos da raiz.
/// A gramática tem 7 níveis; o parser os resolve recursivamente e
/// os armazena aqui de forma achatada como `BinOp` / `UnOp`.
#[derive(Debug, Clone, PartialEq)]
pub enum Expressao {
    // ── Literais ──────────────────────────────────────────────────────────
    LitInt(i64, Span),
    LitBool(bool, Span),
    LitString(String, Span),

    // ── Variável ──────────────────────────────────────────────────────────
    Var(String, Span),

    // ── Operação binária ──────────────────────────────────────────────────
    BinOp {
        op: OpBin,
        esq: Box<Expressao>,
        dir: Box<Expressao>,
        span: Span,
    },

    // ── Operação unária ───────────────────────────────────────────────────
    UnOp {
        op: OpUn,
        operando: Box<Expressao>,
        span: Span,
    },
}

impl Expressao {
    /// Retorna o `Span` de qualquer variante — útil para mensagens de erro.
    pub fn span(&self) -> &Span {
        match self {
            Expressao::LitInt(_, s)    => s,
            Expressao::LitBool(_, s)   => s,
            Expressao::LitString(_, s) => s,
            Expressao::Var(_, s)       => s,
            Expressao::BinOp { span, .. } => span,
            Expressao::UnOp { span, .. }  => span,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Operadores
// ─────────────────────────────────────────────────────────────────────────────

/// Operadores binários, ordenados do maior para o menor nível de precedência
/// conforme a gramática (só para documentação — a precedência real é resolvida
/// pelo parser recursivo, não por este enum).
#[derive(Debug, Clone, PartialEq)]
pub enum OpBin {
    // Aritméticos (precedência mais alta)
    Mul, Div, Mod,
    Add, Sub,
    // Relacionais
    Gt, Lt, Ge, Le,
    // Igualdade
    Eq, Neq,
    // Lógicos (precedência mais baixa)
    And,
    Or,
}

impl std::fmt::Display for OpBin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            OpBin::Mul => "*",  OpBin::Div => "/",  OpBin::Mod => "%",
            OpBin::Add => "+",  OpBin::Sub => "-",
            OpBin::Gt  => ">",  OpBin::Lt  => "<",
            OpBin::Ge  => ">=", OpBin::Le  => "<=",
            OpBin::Eq  => "==", OpBin::Neq => "!=",
            OpBin::And => "&&", OpBin::Or  => "||",
        };
        write!(f, "{}", s)
    }
}

/// Operadores unários.
#[derive(Debug, Clone, PartialEq)]
pub enum OpUn {
    Neg, // `-` aritmético
    Not, // `!` lógico
}

impl std::fmt::Display for OpUn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpUn::Neg => write!(f, "-"),
            OpUn::Not => write!(f, "!"),
        }
    }
}
