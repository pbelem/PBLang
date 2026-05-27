use std::collections::HashMap;

use crate::ast::{Span, Tipo};
use crate::error::PBError;

// ─────────────────────────────────────────────────────────────────────────────
// Entrada da tabela de símbolos
// ─────────────────────────────────────────────────────────────────────────────

/// Informação armazenada para cada variável declarada.
#[derive(Debug, Clone)]
pub struct Simbolo {
    pub tipo: Tipo,
    /// Localização da declaração — usada na mensagem de redeclaração
    /// para mostrar onde a variável já havia sido declarada.
    pub span_decl: Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tabela de símbolos
// ─────────────────────────────────────────────────────────────────────────────

/// Mapeia identificadores para seus tipos e localização de declaração.
///
/// A PBLang não tem escopos aninhados — todas as variáveis são declaradas
/// no escopo global da função principal — portanto um único `HashMap` é
/// suficiente. Se no futuro a linguagem ganhar funções ou blocos com escopo
/// próprio, basta substituir por uma pilha de `HashMap`.
#[derive(Debug, Default)]
pub struct TabelaDeSimbolos {
    tabela: HashMap<String, Simbolo>,
}

impl TabelaDeSimbolos {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra uma nova variável.
    ///
    /// Retorna `Err` se o identificador já foi declarado, com mensagem
    /// indicando a localização original e a nova tentativa.
    pub fn declarar(
        &mut self,
        nome: &str,
        tipo: Tipo,
        span: &Span,
    ) -> Result<(), PBError> {
        if let Some(original) = self.tabela.get(nome) {
            return Err(PBError::Semantic {
                message: format!(
                    "Variável '{}' já foi declarada (declaração original na linha {}, coluna {})",
                    nome, original.span_decl.line, original.span_decl.col
                ),
                line: span.line,
                col: span.col,
            });
        }
        self.tabela.insert(nome.to_string(), Simbolo { tipo, span_decl: span.clone() });
        Ok(())
    }

    /// Consulta o tipo de uma variável declarada.
    ///
    /// Retorna `Err` se o identificador não foi declarado.
    pub fn consultar(&self, nome: &str, span: &Span) -> Result<&Tipo, PBError> {
        self.tabela.get(nome).map(|s| &s.tipo).ok_or_else(|| PBError::Semantic {
            message: format!("Variável '{}' usada antes de ser declarada", nome),
            line: span.line,
            col: span.col,
        })
    }
}
