# PBLang — Compilador em Rust
## Documento de Requisitos e Plano de Projeto

**Aluno:** [Seu Nome]  
**Disciplina:** Teoria de Linguagens Formais e Autômatos  
**Início:** 18/05/2026 · **Entrega:** 08/06/2026 · **Duração:** 21 dias

---

## 1. Visão Geral

O projeto **PBLang** consiste no desenvolvimento de um compilador funcional para uma linguagem de programação imperativa de propósito didático, implementado em **Rust**. O pipeline de compilação passa pelas fases clássicas: análise léxica, sintática, semântica, geração de IR via LLVM (Inkwell) e produção de executável nativo.

---

## 2. Especificação da Linguagem

### 2.1 Sintaxe Léxica e Gramatical

```
programa          → declaração* comando*
declaração        → 'var' ID ':' tipo ';'
tipo              → 'int' | 'bool'

bloco             → '{' comando* '}'
comando           → atribuição | stmt_if | stmt_while | stmt_print | stmt_read | bloco

atribuição        → ID '=' expressão ';'
stmt_if           → 'if' '(' expressão ')' bloco ('else' bloco)?
stmt_while        → 'while' '(' expressão ')' bloco
stmt_print        → 'print' '(' expressão ')' ';'
stmt_read         → 'read' '(' ID ')' ';'

expressão         → exp_or
exp_or            → exp_and ('||' exp_and)*
exp_and           → exp_igualdade ('&&' exp_igualdade)*
exp_igualdade     → exp_relacional (('==' | '!=') exp_relacional)*
exp_relacional    → exp_aditiva (('>' | '<' | '>=' | '<=') exp_aditiva)*
exp_aditiva       → exp_multiplicativa (('+' | '-') exp_multiplicativa)*
exp_multiplicativa → exp_unaria (('*' | '/' | '%') exp_unaria)*
exp_unaria        → ('!' | '-') exp_unaria | primário
primário          → INTEIRO | 'true' | 'false' | ID | '(' expressão ')'

comentários       → '//' até o fim da linha
```

### 2.2 Tokens Reconhecidos

| Categoria | Exemplos |
|---|---|
| Palavras reservadas | `var`, `int`, `bool`, `true`, `false`, `if`, `else`, `while`, `print`, `read` |
| Identificadores | `[a-zA-Z_][a-zA-Z0-9_]*` |
| Literais inteiros | `[0-9]+` |
| Operadores aritméticos | `+`, `-`, `*`, `/`, `%` |
| Operadores relacionais | `>`, `<`, `>=`, `<=` |
| Operadores de igualdade | `==`, `!=` |
| Operadores lógicos | `&&`, `\|\|`, `!` |
| Delimitadores | `(`, `)`, `{`, `}`, `;`, `:`, `=` |
| Comentários | `// texto` (ignorados) |
| Espaços em branco | espaço, tab, newline (ignorados) |

---

## 3. Regras Semânticas

### 3.1 Escopo e Declaração
- Toda variável (`ID`) deve ser declarada com `var` antes de qualquer leitura ou atribuição.
- Não há escopos aninhados: todas as declarações são globais à função principal.

### 3.2 Verificação de Tipos (Type Checking)

| Contexto | Regra | Tipo Resultante |
|---|---|---|
| `stmt_if` / `stmt_while` | Expressão de condição **deve** ser `bool` | — |
| `+`, `-`, `*`, `/`, `%`, `-` unário | Ambos os operandos `int` | `int` |
| `>`, `<`, `>=`, `<=` | Ambos os operandos `int` | `bool` |
| `==`, `!=` | Operandos de **tipos idênticos** | `bool` |
| `&&`, `\|\|`, `!` unário | Operandos `bool` | `bool` |
| Atribuição | Tipo da expressão == tipo declarado do `ID` | — |

---

## 4. Arquitetura do Compilador

### 4.1 Estrutura de Diretórios

```
pbelang/
├── Cargo.toml
└── src/
    ├── main.rs           ← lê arquivo .pb, invoca pipeline completo
    ├── error/            ← tipos de erro unificados (léxico, sintático, semântico)
    ├── lexer/            ← tokens + scanner via logos
    ├── ast/              ← definição das structs/enums da AST
    ├── parser/           ← parser recursivo descendente
    ├── symbol_table/     ← tabela de símbolos (HashMap<String, Tipo>)
    ├── semantic/         ← verificação de tipos e escopo
    └── codegen/          ← geração de LLVM IR via inkwell
```

### 4.2 Pipeline de Compilação

```
Código Fonte (.pb)
       ↓
  [Fase A] Lexer          → sequência de Tokens
       ↓
  [Fase B] Parser         → Árvore de Sintaxe Abstrata (AST)
       ↓
  [Fase C] Semântica      → AST validada + Tabela de Símbolos
       ↓
  [Fase D] Codegen        → LLVM IR em memória (via Inkwell)
       ↓
  [Fase E] LLVM Backend   → Otimização (opt) + Executável nativo
```

### 4.3 Dependências Rust

```toml
[dependencies]
logos    = "0.16.1"                        # Fase A — tokenização via DFA
inkwell = { git = "[https://github.com/TheDan64/inkwell](https://github.com/TheDan64/inkwell)", branch = "master", features = ["llvm18-1"] }   # Fases D/E — LLVM IR

---

## 5. Diretrizes de Geração de Código (Fases D e E)

### 5.1 Mapeamento AST → LLVM IR

Toda a geração de código é feita **diretamente em memória** via Inkwell, sem etapa textual de TAC intermediária.

| Construção da AST | Instrução LLVM IR |
|---|---|
| `var x : int` | `alloca i32` |
| `var b : bool` | `alloca i1` |
| `x = expr` | `store <valor>, <ponteiro>` |
| `ID` em expressão | `load <tipo>, <ponteiro>` |
| `+`, `-`, `*`, `/`, `%` | `add`, `sub`, `mul`, `sdiv`, `srem` |
| `>`, `<`, `>=`, `<=` | `icmp sgt/slt/sge/sle` |
| `==`, `!=` | `icmp eq/ne` |
| `&&`, `\|\|` | `and`, `or` |
| `!` | `xor <val>, true` |
| `if-else` | blocos básicos + `br` condicional |
| `while` | blocos: `cond`, `body`, `end` |
| `print(expr)` | call a `printf` (via declaração externa) |
| `read(ID)` | call a `scanf` (via declaração externa) |

### 5.2 Conformidade com SSA
O LLVM exige SSA (Static Single Assignment). O padrão `alloca/store/load` satisfaz essa exigência automaticamente: variáveis mutáveis ficam na pilha e são acessadas por ponteiro, nunca reatribuídas como registradores SSA.

---

## 6. Tratamento de Erros

O compilador deve produzir mensagens de erro claras com localização (linha e coluna):

```
[ERRO LÉXICO]    Linha 3, Col 7 — Caractere inesperado: '@'
[ERRO SINTÁTICO] Linha 5, Col 12 — Esperado ';', encontrado '}'
[ERRO SEMÂNTICO] Linha 8, Col 4 — Variável 'x' usada antes de declaração
[ERRO SEMÂNTICO] Linha 10, Col 9 — Tipo incompatível: esperado 'bool', obtido 'int'
```

---

## 7. Plano de Etapas e Cronograma

> **Premissa:** 21 dias úteis (18/05 a 08/06). As fases são sequenciais com pequenas sobreposições no final.

### FASE A — Análise Léxica (Scanner)
**Prazo:** 18/05 (dom) a 20/05 (ter) · **3 dias**

**Objetivo:** Transformar fluxo de caracteres em sequência de tokens.

**Tarefas:**
- Definir enum `Token` com todas as categorias da seção 2.2
- Implementar o lexer com `logos` (macros `#[token]` e `#[regex]`)
- Ignorar espaços em branco e comentários `//`
- Preservar informação de linha/coluna para erros
- Escrever testes unitários: ao menos 1 caso válido e 1 inválido por categoria de token

**Critério de aceite:** O lexer tokeniza corretamente 5 programas de exemplo e rejeita entradas com caracteres inválidos com mensagem de erro.

---

### FASE B — Análise Sintática (Parser + AST)
**Prazo:** 21/05 (qua) a 24/05 (sáb) · **4 dias**

**Objetivo:** Construir a AST a partir dos tokens.

**Tarefas:**
- Definir as structs/enums da AST em `src/ast/` (nós para `Programa`, `Declaração`, `Comando`, `Expressão`)
- Implementar parser recursivo descendente em `src/parser/` seguindo a gramática da seção 2.1
- Implementar corretamente a precedência de operadores (7 níveis)
- Implementar parsing de `if-else`, `while`, `print`, `read`, `atribuição`
- Tratar e reportar erros sintáticos com linha/coluna
- Escrever testes: programas com `if`, `while` e expressões aninhadas

**Critério de aceite:** O parser constrói a AST corretamente para 5 programas válidos e reporta erro para 3 programas inválidos.

---

### FASE C — Análise Semântica
**Prazo:** 25/05 (dom) a 27/05 (ter) · **3 dias**

**Objetivo:** Garantir corretude lógica além da gramatical.

**Tarefas:**
- Implementar `SymbolTable` em `src/symbol_table/` (`HashMap<String, Tipo>`)
- Implementar visitor/traversal da AST em `src/semantic/`
- Verificar declaração prévia de todas as variáveis usadas
- Implementar type checker seguindo todas as regras da seção 3.2
- Verificar que condições de `if`/`while` são `bool`
- Verificar compatibilidade de tipos em atribuições
- Produzir erros descritivos para cada violação semântica

**Critério de aceite:** O analisador detecta 100% dos erros semânticos nos programas de teste, sem falsos positivos.

---

### FASE D — Geração de IR LLVM (Inkwell)
**Prazo:** 28/05 (qua) a 02/06 (ter) · **6 dias**

**Objetivo:** Traduzir a AST validada para LLVM IR em memória.

**Tarefas:**
- Configurar contexto, módulo e builder do Inkwell
- Gerar `alloca` para cada `var` declarada
- Gerar `store`/`load` para atribuições e usos de variáveis
- Gerar instruções aritméticas e lógicas (`add`, `sub`, `icmp`, etc.)
- Gerar blocos básicos para `if-else` e `while` com `br` condicional
- Declarar e chamar `printf`/`scanf` externos para `print`/`read`
- Emitir IR para arquivo `.ll` para inspeção manual

**Critério de aceite:** A IR gerada é válida (`llvm-as`/`lli` sem erros) para 3 programas de teste incluindo `while` e `if-else`.

---

### FASE E — Geração de Executável e Testes de Integração
**Prazo:** 03/06 (qua) a 05/06 (sex) · **3 dias**

**Objetivo:** Produzir executável nativo e validar saída.

**Tarefas:**
- Invocar `target_machine.write_to_file()` para gerar objeto nativo
- Linkar com `cc`/`clang` para produzir executável final
- Executar bateria de testes end-to-end (entrada → saída esperada)
- **(Bônus)** Habilitar passes de otimização do LLVM: `PassManager` com eliminação de código morto e constant folding

**Programas de teste obrigatórios:**

| # | Programa | Saída Esperada |
|---|---|---|
| 1 | Soma de dois números lidos | valor da soma |
| 2 | Fatorial com `while` | fatorial de N |
| 3 | Verificação par/ímpar com `if-else` | "par" ou "impar" simulado via int |
| 4 | Programa com erro semântico de tipo | mensagem de erro |
| 5 | Programa com variável não declarada | mensagem de erro |

**Critério de aceite:** Os 3 primeiros programas executam com saída correta; os 2 últimos são rejeitados na fase semântica.

---

### FASE F — Documentação e Entrega
**Prazo:** 06/06 (sáb) a 08/06 (seg) · **3 dias**

**Objetivo:** Relatório técnico e code review final.

**Tarefas:**
- Escrever relatório explicativo (~2-3 páginas) cobrindo: decisões de design, dificuldades encontradas, exemplos de IR gerada
- Revisar qualidade do código: comentários, nomes de funções, organização
- Executar `cargo clippy` e corrigir avisos
- Garantir que `cargo test` passa sem falhas
- Preparar README com instruções de build (`nix develop` + `cargo run`)

---

## 8. Resumo do Cronograma

```
Mai 18  ████████████░░░░░░░░░░░░░░░░░░░░░░░  Jun 08
        |  A  |   B   |  C  |    D    |E |F|
        18   21      25    28        03 06 08
        └─3d─┘ └──4d──┘ └─3d─┘ └──6d──┘└3d┘└3d┘
```

| Fase | Período | Dias | Entregável |
|---|---|---|---|
| A — Léxica | 18/05 – 20/05 | 3 | `src/lexer/` + testes unitários |
| B — Sintática | 21/05 – 24/05 | 4 | `src/parser/` + `src/ast/` |
| C — Semântica | 25/05 – 27/05 | 3 | `src/semantic/` + `src/symbol_table/` |
| D — Codegen IR | 28/05 – 02/06 | 6 | `src/codegen/` + IR `.ll` válida |
| E — Executável | 03/06 – 05/06 | 3 | Binário funcional + testes e2e |
| F — Docs | 06/06 – 08/06 | 3 | Relatório + README + `cargo test` |

---

## 9. Critérios de Avaliação

| Critério | Peso | Meta |
|---|---|---|
| Corretude Léxica/Sintática | 30% | Aceitar programas válidos; rejeitar inválidos com mensagem clara |
| Análise Semântica | 20% | Detectar 100% dos erros de tipo e variáveis não declaradas |
| Geração de Código | 30% | Executável produz saída correta para todos os programas de teste |
| Documentação/Código | 20% | Código comentado, clippy limpo, relatório explicativo |

---

## 10. Ambiente de Desenvolvimento

### Pré-requisitos
- **Nix** com flakes habilitado
- **LLVM 18** (fornecido pelo flake)
- **Git** (arquivos do projeto precisam ser rastreados para o Crane compilar)

### Setup

# Inicializar o repositório e rastrear os arquivos
git init
cargo generate-lockfile
git add .

```bash
# Entrar no ambiente de desenvolvimento
nix develop

# Verificar ambiente
rustc --version   # stable
llc --version     # LLVM 18.x

# Executar o compilador
cargo run -- exemplos/hello.pb

# Testes
cargo test

# Linting
cargo clippy -- -D warnings
```

### Notas sobre o Flake
O arquivo `flake.nix` está corretamente configurado. Recomendações adicionais:
- Mover `pkg-config` e `cmake` para `nativeBuildInputs` (boa prática para cross-compilation)
- Adicionar `llvm.clang` ao `devShell` para inspecionar IR manualmente com `lli arquivo.ll`
- Considerar adicionar `ncurses` ao `commonBuildInputs` para evitar falhas em certas distribuições Linux

---

*Documento gerado em 18/05/2026*
