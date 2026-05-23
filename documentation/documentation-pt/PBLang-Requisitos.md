# Documento de Requisitos – PBLang

**Aluno:** Pedro Belém  
**Disciplina:** Compiladores  
**Professora:** Sheila Tirony  
**Início:** 18/05/2026 · **Entrega:** 08/06/2026 · **Duração:** 21 dias

---

# Histórico de Mudanças

| Data       | Descrição |
|------------|------------|
| 18/05/2026 | Criação inicial do documento. |
| 19/05/2026 | **Adição do tipo `string`** à linguagem (afeta RF-02, RF-05 e regras semânticas). |

---

# 1. Visão Geral

O projeto **PBLang** consiste no desenvolvimento de um compilador funcional para uma linguagem de programação imperativa de propósito didático. O pipeline de compilação passa pelas fases clássicas: análise léxica, sintática, semântica, geração de IR via LLVM e produção de executável nativo.

---

# 2. Requisitos Funcionais

## RF-01 — Pipeline completo de compilação

O sistema deve aceitar um arquivo-fonte `.pb` como entrada e produzir um executável nativo como saída, passando pelas fases de análise léxica, sintática, semântica e geração de código.

## RF-02 — Tipos de dados primitivos

A linguagem deve suportar três tipos primitivos:

- `int` (inteiro com sinal de 32 bits)
- `bool` (booleano)
- `string` (cadeia de caracteres delimitada por aspas duplas)

## RF-03 — Declaração explícita de variáveis

Toda variável deve ser declarada antes de ser usada. A declaração deve especificar o identificador e seu tipo. Variáveis não declaradas devem causar erro semântico.

## RF-04 — Estruturas de controle

A linguagem deve suportar seleção condicional (`if`/`else`) e repetição (`while`). A condição de ambas as estruturas deve ser do tipo `bool`.

## RF-05 — Operações aritméticas

A linguagem deve suportar as operações:

- `+`
- `-`
- `*`
- `/`
- `%`

sobre operandos do tipo `int`.

O operador `+` também deve funcionar como concatenação quando ambos os operandos forem `string`.

## RF-06 — Operações relacionais e lógicas

A linguagem deve suportar:

### Comparações

- `>`
- `<`
- `>=`
- `<=`
- `==`
- `!=`

### Operadores lógicos

- `&&`
- `||`
- `!`

respeitando as regras de tipos definidas na seção 4.2.

## RF-07 — Entrada e saída

A linguagem deve oferecer:

- `print(expr)` para exibir valores
- `read(ID)` para ler valores da entrada padrão

`print` deve suportar expressões de qualquer tipo; `read` deve ser aplicado apenas a variáveis `int` ou `string`.

## RF-08 — Comentários

O compilador deve ignorar comentários de linha iniciados com `//`.

---

# 3. Requisitos Não-Funcionais

## RNF-01 — Mensagens de erro precisas

Erros léxicos, sintáticos e semânticos devem indicar linha e coluna do problema, com descrição clara do que foi encontrado e do que era esperado.

## RNF-02 — Implementação em Rust

Todo o compilador deve ser implementado em Rust estável, sem uso de `unsafe` desnecessário, passando em:

```bash
cargo clippy -- -D warnings
```

sem avisos.

## RNF-03 — Reprodutibilidade do ambiente

O ambiente de compilação deve ser completamente reproduzível via:

```bash
nix develop
```

garantindo as mesmas versões de Rust e LLVM em qualquer máquina.

## RNF-04 — Cobertura de testes

Cada fase do compilador deve ter testes unitários. O pipeline completo deve ter ao menos 5 programas de teste end-to-end com saída verificada.


## RNF-05 — Stack de tecnologias

Linguagem de Implementação — Rust (stable)
Análise Léxica — Logos 0.16
Geração de IR — Inkwell + LLVM 18
Gerenciamento de Ambiente — Nix Flakes
Build System — Cargo + Crane

---

# 4. Especificação da Linguagem

## 4.1 Sintaxe Léxica e Gramatical

```ebnf
programa           → declaração* comando*
declaração         → 'var' ID ':' tipo ';'
tipo               → 'int' | 'bool' | 'string'

bloco              → '{' comando* '}'
comando            → atribuição | stmt_if | stmt_while | stmt_print | stmt_read | bloco

atribuição         → ID '=' expressão ';'
stmt_if            → 'if' '(' expressão ')' bloco ('else' bloco)?
stmt_while         → 'while' '(' expressão ')' bloco
stmt_print         → 'print' '(' expressão ')' ';'
stmt_read          → 'read' '(' ID ')' ';'

expressão          → exp_or
exp_or             → exp_and ('||' exp_and)*
exp_and            → exp_igualdade ('&&' exp_igualdade)*
exp_igualdade      → exp_relacional (('==' | '!=') exp_relacional)*
exp_relacional     → exp_aditiva (('>' | '<' | '>=' | '<=') exp_aditiva)*
exp_aditiva        → exp_multiplicativa (('+' | '-') exp_multiplicativa)*
exp_multiplicativa → exp_unaria (('*' | '/' | '%') exp_unaria)*
exp_unaria         → ('!' | '-') exp_unaria | primário
primário           → INTEIRO | STRING | 'true' | 'false' | ID | '(' expressão ')'

comentários        → '//' até o fim da linha
```
## 4.2 Tokens Reconhecidos

| Categoria | Padrão / Exemplos |
|---|---|
| Palavras reservadas | `var`, `int`, `bool`, `string`, `true`, `false`, `if`, `else`, `while`, `print`, `read` |
| Identificadores | `[a-zA-Z_][a-zA-Z0-9_]*` |
| Literais inteiros | `[0-9]+` |
| Literais string | `"[^"]*"` (entre aspas duplas, sem quebra de linha) |
| Operadores aritméticos | `+`, `-`, `*`, `/`, `%` |
| Operadores relacionais | `>`, `<`, `>=`, `<=` |
| Operadores de igualdade | `==`, `!=` |
| Operadores lógicos | `&&`, `||`, `!` |
| Delimitadores | `(`, `)`, `{`, `}`, `;`, `:`, `=` |
| Comentários | `// texto` (descartados) |
| Espaços em branco | espaço, tab, newline (descartados) |

---

## 4.3 Regras Semânticas

### Escopo e Declaração

- Toda variável (`ID`) deve ser declarada com `var` antes de qualquer leitura ou atribuição.
- Não há escopos aninhados: todas as declarações são globais à função principal.
- Redeclaração do mesmo identificador é um erro semântico.

### Verificação de Tipos (Type Checking)

| Contexto | Regra | Tipo Resultante |
|---|---|---|
| `stmt_if` / `stmt_while` | Condição **deve** ser `bool` | — |
| `+` com `int` | Ambos os operandos `int` | `int` |
| `+` com `string` | Ambos os operandos `string` (concatenação) | `string` |
| `-`, `*`, `/`, `%`, `-` unário | Ambos os operandos `int` | `int` |
| `>`, `<`, `>=`, `<=` | Ambos os operandos `int` | `bool` |
| `==`, `!=` | Operandos de **tipos idênticos** (qualquer tipo) | `bool` |
| `&&`, `||`, `!` unário | Operandos `bool` | `bool` |
| Atribuição | Tipo da expressão == tipo declarado do `ID` | — |
| `read(ID)` | `ID` deve ser do tipo `int` ou `string` | — |
| `print(expr)` | Qualquer tipo é aceito | — |

---

# 5. Tratamento de Erros (Formato das Mensagens)

O compilador deve produzir mensagens de erro claras com localização (linha e coluna), conforme o seguinte padrão:

```text
[ERRO LÉXICO]    Linha  3, Col  7  —  Caractere inesperado: '@'
[ERRO SINTÁTICO] Linha  5, Col 12  —  Esperado ';', encontrado '}'
[ERRO SEMÂNTICO] Linha  8, Col  4  —  Variável 'x' usada antes de declaração
[ERRO SEMÂNTICO] Linha 10, Col  9  —  Tipo incompatível: esperado 'bool', obtido 'int'
[ERRO SEMÂNTICO] Linha 14, Col  6  —  Operador '+' não suportado entre 'int' e 'string'
```

---
