# Plano de Projeto – PBLang

**Aluno:** Pedro Belém  
**Disciplina:** Compiladores  
**Professora:** Sheila Tirony  
**Início:** 18/05/2026 · **Entrega:** 08/06/2026 · **Duração:** 21 dias

---

# 1. Visão Geral

O projeto **PBLang** consiste no desenvolvimento de um compilador funcional para uma linguagem de programação imperativa de propósito didático, implementado em **Rust**. O pipeline de compilação passa pelas fases clássicas: análise léxica, sintática, semântica, geração de IR via LLVM (Inkwell) e produção de executável nativo.

---

# 2. Arquitetura do Compilador

## 2.1 Estrutura de Diretórios

```text
pbelang/
├── Cargo.toml
├── flake.nix
├── .envrc
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

## 2.2 Pipeline de Compilação

```text
Código Fonte (.pb)
       ↓
  [Fase A] Lexer       →  sequência de Tokens
       ↓
  [Fase B] Parser      →  Árvore de Sintaxe Abstrata (AST)
       ↓
  [Fase C] Semântica   →  AST validada + Tabela de Símbolos
       ↓
  [Fase D] Codegen     →  LLVM IR em memória (via Inkwell)
       ↓
  [Fase E] LLVM        →  Otimização (opt) + Executável nativo
```

# 3. Diretrizes de Geração de Código (Fases D e E)

## 3.1 Mapeamento AST → LLVM IR

Toda a geração de código é feita **diretamente em memória** via Inkwell, sem etapa textual de TAC intermediária.

| Construção da AST | Instrução LLVM IR |
|---|---|
| `var x : int` | `alloca i32` |
| `var b : bool` | `alloca i1` |
| `var s : string` | `alloca i8*` (ponteiro para char) |
| `x = expr` | `store <valor>, <ponteiro>` |
| `ID` em expressão | `load <tipo>, <ponteiro>` |
| `+`, `-`, `*`, `/`, `%` (int) | `add`, `sub`, `mul`, `sdiv`, `srem` |
| `+` (string) | call a função auxiliar de concatenação |
| `>`, `<`, `>=`, `<=` | `icmp sgt/slt/sge/sle` |
| `==`, `!=` | `icmp eq/ne` |
| `&&`, `\|\|` | `and`, `or` |
| `!` | `xor <val>, true` |
| `if-else` | blocos básicos + `br` condicional |
| `while` | blocos: `cond`, `body`, `end` |
| `print(expr)` | call a `printf` (via declaração externa) |
| `read(ID)` | call a `scanf` (via declaração externa) |

## 3.2 Conformidade com SSA

O LLVM exige SSA (Static Single Assignment): nenhum registrador pode ser atribuído mais de uma vez.

O padrão `alloca/store/load` satisfaz essa exigência automaticamente — variáveis mutáveis ficam na pilha e são acessadas por ponteiro, nunca reatribuídas como registradores SSA.

---

# 4. Ambiente de Desenvolvimento

## 4.1 Pré-requisitos

| Ferramenta | Versão mínima | Finalidade |
|---|---|---|
| Nix | 2.18+ | Gerenciamento de ambiente reproduzível |
| Git | qualquer | Necessário para o Crane rastrear fontes |
| direnv | qualquer (opcional) | Ativação automática do ambiente Nix |

Nix Flakes deve estar habilitado.

Se ainda não estiver, adicione ao `/etc/nix/nix.conf` ou `~/.config/nix/nix.conf`:

```conf
experimental-features = nix-command flakes
```

## 4.2 Instalação do Nix (caso necessário)

```bash
# Instalador oficial multi-usuário (Linux/macOS)
sh <(curl -L https://nixos.org/nix/install) --daemon
```

## 4.3 Configuração inicial do projeto

# 1. Clonar o repositório
git clone https://gitlab.com/pbelem-group/pblang
cd PBLang

# 2. (Opcional) Permitir que o direnv ative o ambiente automaticamente
#    O arquivo .envrc já contém 'use flake'
direnv allow

# 3. Sem direnv: entrar no ambiente manualmente
nix develop

# 6. Plano de Etapas e Cronograma

## FASE A — Análise Léxica (Scanner)

**Prazo:** 18/05 (dom) a 20/05 (ter) · **3 dias**

### Objetivo

Transformar fluxo de caracteres em sequência de tokens.

### Tarefas

- Definir enum `Token` com todas as categorias da seção 4.2, incluindo `StringLit`
- Implementar o lexer com `logos` (macros `#[token]` e `#[regex]`)
- Ignorar espaços em branco e comentários `//`
- Preservar informação de linha/coluna para erros
- Escrever testes unitários: ao menos 1 caso válido e 1 inválido por categoria de token

### Critério de aceite

O lexer tokeniza corretamente 5 programas de exemplo e rejeita entradas com caracteres inválidos com mensagem de erro.

---

## FASE B — Análise Sintática (Parser + AST)

**Prazo:** 21/05 (qua) a 24/05 (sáb) · **4 dias**

### Objetivo

Construir a AST a partir dos tokens.

### Tarefas

- Definir as structs/enums da AST em `src/ast/` (nós para `Programa`, `Declaração`, `Comando`, `Expressão`), incluindo o nó `StringLit`
- Implementar parser recursivo descendente em `src/parser/` seguindo a gramática da seção 4.1
- Implementar corretamente a precedência de operadores (7 níveis)
- Implementar parsing de `if-else`, `while`, `print`, `read`, `atribuição`
- Tratar e reportar erros sintáticos com linha/coluna
- Escrever testes: programas com `if`, `while`, strings e expressões aninhadas

### Critério de aceite

O parser constrói a AST corretamente para 5 programas válidos e reporta erro para 3 programas inválidos.

---

## FASE C — Análise Semântica

**Prazo:** 25/05 (dom) a 27/05 (ter) · **3 dias**

### Objetivo

Garantir corretude lógica além da gramatical.

### Tarefas

- Implementar `SymbolTable` em `src/symbol_table/` (`HashMap<String, Tipo>`)
- Implementar visitor/traversal da AST em `src/semantic/`
- Verificar declaração prévia e ausência de redeclaração de variáveis
- Implementar type checker seguindo todas as regras da seção 5.2, incluindo `string`
- Verificar que condições de `if`/`while` são `bool`
- Verificar compatibilidade de tipos em atribuições e em `read`
- Produzir erros descritivos para cada violação semântica

### Critério de aceite

O analisador detecta 100% dos erros semânticos nos programas de teste, sem falsos positivos.

---

## FASE D — Geração de IR LLVM (Inkwell)

**Prazo:** 28/05 (qua) a 02/06 (ter) · **6 dias**

### Objetivo

Traduzir a AST validada para LLVM IR em memória.

### Tarefas

- Configurar contexto, módulo e builder do Inkwell
- Gerar `alloca` para cada `var` declarada (`i32`, `i1`, `i8*`)
- Gerar `store`/`load` para atribuições e usos de variáveis
- Gerar instruções aritméticas e lógicas (`add`, `sub`, `icmp`, etc.)
- Gerar concatenação de strings via função auxiliar
- Gerar blocos básicos para `if-else` e `while` com `br` condicional
- Declarar e chamar `printf`/`scanf` externos para `print`/`read`
- Emitir IR para arquivo `.ll` para inspeção manual com `lli`

### Critério de aceite

A IR gerada é válida para 3 programas de teste incluindo `while`, `if-else` e strings.

---

## FASE E — Geração de Executável e Testes de Integração

**Prazo:** 03/06 (qua) a 05/06 (sex) · **3 dias**

### Objetivo

Produzir executável nativo e validar saída.

### Tarefas

- Invocar `target_machine.write_to_file()` para gerar objeto nativo
- Linkar com `cc`/`clang` para produzir executável final
- Executar bateria de testes end-to-end (entrada → saída esperada)
- **(Bônus)** Habilitar passes de otimização do LLVM: `PassManager` com eliminação de código morto e constant folding

### Programas de teste obrigatórios

| # | Programa | Saída Esperada |
|---|---|---|
| 1 | Soma de dois números lidos | valor da soma |
| 2 | Fatorial com `while` | fatorial de N |
| 3 | Verificação par/ímpar com `if-else` | saída numérica indicando paridade |
| 4 | Concatenação de strings e `print` | string resultante |
| 5 | Programa com erro semântico de tipo | mensagem de erro |
| 6 | Programa com variável não declarada | mensagem de erro |

### Critério de aceite

Os 4 primeiros programas executam com saída correta; os 2 últimos são rejeitados na fase semântica.

---

## FASE F — Documentação e Entrega

**Prazo:** 06/06 (sáb) a 08/06 (seg) · **3 dias**

### Objetivo

Relatório técnico e code review final.

### Tarefas

- Escrever relatório explicativo (~2-3 páginas) cobrindo: decisões de design, dificuldades encontradas, exemplos de IR gerada
- Revisar qualidade do código: comentários, nomes de funções, organização
- Executar `cargo clippy` e corrigir avisos
- Garantir que `cargo test` passa sem falhas
- Preparar README com instruções de build (`nix develop` + `cargo run`)

---

# 7. Resumo do Cronograma

| Fase | Período | Dias | Entregável |
|---|---|---|---|
| A — Léxica | 18/05 – 20/05 | 3 | `src/lexer/` + testes unitários |
| B — Sintática | 21/05 – 24/05 | 4 | `src/parser/` + `src/ast/` |
| C — Semântica | 25/05 – 27/05 | 3 | `src/semantic/` + `src/symbol_table/` |
| D — Codegen IR | 28/05 – 02/06 | 6 | `src/codegen/` + IR `.ll` válida |
| E — Executável | 03/06 – 05/06 | 3 | Binário funcional + testes e2e |
| F — Docs | 06/06 – 08/06 | 3 | Relatório + README + `cargo test` |

```text
18/Mai                                                08/Jun
  |                                                      |
  [=A=][======B=====][====C====][=========D=========][=E=][F]
  18  21            25         28                   03  06  08
```
