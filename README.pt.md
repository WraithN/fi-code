<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh_CN.md">中文</a> |
  <a href="README.zh_TW.md">繁體中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.pt.md">Português</a>
</p>

# fi-code

Um Agente de Codificação AI de terminal construído em Rust, interagindo com os usuários via REPL, TUI, HTTP Server ou Desktop. Suporta conversas de múltiplos turnos, chamadas de ferramentas, persistência de sessões e extensões de protocolo MCP.

## Funcionalidades

- **🤖 Suporte Multi-Modelo**: Interfaces unificadas compatíveis com OpenAI e Anthropic com respostas SSE em streaming. Mecanismo de repetição integrado (backoff exponencial + full jitter).
- **🔧 Chamadas de Ferramentas**: 20 ferramentas integradas incluindo `bash`, `read`, `write`, `edit`, `web_fetch`, `grep`, `glob` e uma suite completa de ferramentas Git. O Agente executa automaticamente com base nas respostas do modelo e retorna resultados.
- **💬 Persistência de Sessões**: As sessões são escritas incrementalmente no disco local em formato JSON Lines, suportando retomada após interrupções.
- **🖥️ Interação Multi-Modo**:
  - **CLI REPL**: Interação tradicional por linha de comando (`fi-code-cli -i`)
  - **TUI**: Interface de terminal completa baseada em `ratatui` (`fi-code-tui`)
  - **HTTP Server**: API REST + respostas SSE em streaming (`fi-code-server` ou `fi-code-cli server`)
  - **Desktop**: Aplicativo desktop Tauri v2 com sidecar integrado (`fi-code-desktop`)
- **🛡️ Validação de Permissões**: Níveis de risco para operações de alto risco como Bash (Allow / Ask / Deny), interceptando `sudo`, `rm -rf` e ataques de injeção comuns.
- **⚙️ Configuração Flexível**: Suporta `~/.config/fi-code/config.json` ou `config.jsonc`, com comentários, marcadores de posição para variáveis de ambiente (`{env:VAR_NAME}`) e recarga em calor (debounce de 500ms).
- **🔗 Suporte MCP**: Implementação completa do Model Context Protocol, suportando gerenciamento multi-servidor (transporte stdio / HTTP) com reconexão automática (até 3 tentativas, backoff exponencial).
- **📦 Sistema de Skills**: Mecanismo extensível de registro e carregamento de Skills. O Agente pode carregar instruções Skill específicas do projeto sob demanda através da ferramenta `use_skill`.

## Início Rápido

### Requisitos

- [Rust](https://rustup.rs/) 1.70+ (última versão estável recomendada)
- Node.js 18+ (apenas necessário para construir o frontend Desktop)
- Chave API do provedor de AI correspondente

### Instalação

```bash
# Clonar o repositório
git clone <repository-url>
cd fi-code

# Construir todos os binários
cargo build --release

# Executar (ver Uso abaixo)
cargo run --bin fi-code-cli -- --help
```

### Configuração

#### Método 1: Variáveis de Ambiente (Maior Prioridade)

**Compatível com OpenAI:**
```bash
export OPENAI_API_KEY=sk-...
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_MODEL_NAME=gpt-4o
```

**Anthropic:**
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export ANTHROPIC_BASE_URL=https://api.anthropic.com
export ANTHROPIC_MODEL_NAME=claude-3-7-sonnet-20250219
```

Outros provedores predefinidos também suportam variáveis de ambiente: `GLM_*`, `KIMI_*`, `DEEPSEEK_*`, `QWEN_*` / `DASHSCOPE_*`.

#### Método 2: Arquivo de Configuração

Caminhos dos arquivos de configuração (pesquisados em ordem de prioridade):
- Linux/macOS: `~/.config/fi-code/config.jsonc` ou `~/.config/fi-code/config.json`

Exemplo:
```json
{
  "model": "openai/kimi-k2.5",
  "provider": {
    "openai": {
      "provider_type": "openai_compatible",
      "name": "My Provider",
      "options": {
        "apiKey": "{env:MY_API_KEY}",
        "baseURL": "https://api.example.com/v1",
        "timeout": 300000,
        "chunkTimeout": 10000
      },
      "models": {
        "kimi-k2.5": {
          "name": "Kimi K2.5",
          "maxTokens": 128000,
          "modalities": {
            "input": ["text", "image"],
            "output": ["text"]
          }
        }
      }
    }
  },
  "mcp": {
    "filesystem": {
      "type": "local",
      "enabled": true,
      "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/path"]
    }
  },
  "server": {
    "port": 4040,
    "api_token": null,
    "allowed_origins": null
  }
}
```

Suporta comentários `//` e `/* */`. `apiKey` suporta a sintaxe de marcador de posição `{env:VAR_NAME}`. Provedores predefinidos (openai, anthropic, glm, kimi, qwen, deepseek) são automaticamente mesclados na configuração.

### Uso

```bash
# Modo REPL interativo
cargo run --bin fi-code-cli -- -i

# Modo interface TUI de terminal completa
cargo run --bin fi-code-tui

# Executar um único comando e sair
cargo run --bin fi-code-cli -- -c "Escreva um Hello World em Rust"

# Ver provedores e modelos configurados
cargo run --bin fi-code-cli -- --models

# Ver lista de sessões
cargo run --bin fi-code-cli -- -s

# Especificar diretório de trabalho
cargo run --bin fi-code-cli -- -i -w /path/to/project

# Iniciar servidor HTTP
cargo run --bin fi-code-server
# ou
cargo run --bin fi-code-cli -- server --port 4040
```

**Nota**: Executar `fi-code-cli` sem nenhuma flag iniciará automaticamente o modo TUI.

### Desenvolvimento Desktop

O aplicativo Desktop usa uma arquitetura "shell Tauri + sidecar integrado":

```bash
# Primeiro construir o binário sidecar
cargo build

# Instalar dependências do frontend e iniciar modo dev Desktop
cd frontend && npm install
cargo tauri dev

# Build de produção
cargo tauri build
```

## Estrutura do Projeto

Este projeto utiliza uma estrutura **Cargo Workspace**:

```
.
├── Cargo.toml              # Definição do Workspace
├── crates/
│   ├── core/               # Biblioteca core (fi-code-core): toda a lógica de negócio
│   ├── cli/                # Entrada binária CLI (fi-code-cli)
│   ├── tui/                # Entrada binária TUI (fi-code-tui)
│   ├── server/             # Entrada binária Server (fi-code-server)
│   ├── shared/             # DTOs compartilhados e constantes (fi-code-shared)
│   └── utils/              # Utilitários de teste (fi-code-utils)
├── src-tauri/              # Aplicativo Desktop Tauri (fi-code-desktop)
├── frontend/               # Frontend Tauri (React + Vite + Tailwind)
└── tests/                  # Testes E2E e BDD (fi-code-tests)
```

## Ferramentas Integradas

| Ferramenta | Descrição | Nível de Risco |
|------------|-----------|----------------|
| `bash` | Executar comandos shell | Ask (Comandos perigosos Deny) |
| `read` / `read_file` | Ler conteúdo de arquivos | Allow |
| `write` | Escrever em arquivo | Ask |
| `edit` | Editar arquivo | Ask |
| `web_fetch` | Obter página web e converter para Markdown | Ask |
| `grep` | Busca regex no conteúdo de arquivos | Allow |
| `glob` | Correspondência de caminhos de arquivos | Allow |
| `git` | Executar comandos git | Ask |
| `git_status` | Git status | Allow |
| `git_diff` | Git diff | Allow |
| `git_add` | Git add | Ask |
| `git_commit` | Git commit | Ask |
| `git_log` | Git log | Allow |
| `git_worktree` | Operações Git worktree | Ask |
| `create_task_plan` | Criar plano de tarefas | Ask |
| `handle_task_plan` | Executar plano de tarefas | Ask |
| `ask_for_question` | Solicitar entrada do usuário | Ask |
| `use_skill` | Carregar e usar Skill | Ask |
| `mcp:*` | Ferramentas MCP (carregadas dinamicamente) | Depende da config MCP |

## Mecanismos de Segurança

- **Proteção contra Escape de Caminhos**: Todas as operações de arquivos passam por verificações `safe_path` para garantir que não excedam o diretório de trabalho.
- **Sandbox Bash**: Limpa as variáveis de ambiente herdadas, mantém apenas as variáveis mínimas necessárias (`PATH=/usr/bin:/bin`, `HOME`), e aplica um timeout de 120 segundos.
- **Níveis de Permissão**: Deny (rejeitar diretamente comandos perigosos), Ask (confirmação interativa), Allow (operações somente leitura passam diretamente).
- **Truncamento de Saída**: O conteúdo retornado pelas ferramentas está limitado a 50.000 caracteres para prevenir estouro de contexto.

## Atalhos TUI

No modo TUI, os seguintes atalhos estão disponíveis:

| Atalho | Função |
|--------|--------|
| `Tab` / `Shift+Tab` | Alternar área de foco |
| `Ctrl+C` | Parar geração / sair do programa |
| `Ctrl+B` | Abrir/fechar gaveta de arquivos à esquerda |
| `Ctrl+H` | Abrir/fechar gaveta de histórico de sessões à direita |
| `Ctrl+M` | Abrir dropdown de seleção de modelo |
| `Ctrl+T` | Alternar tema |
| `Ctrl+N` | Nova sessão |
| `Enter` | Enviar mensagem |
| `Shift+Enter` | Nova linha na caixa de entrada |
| `Esc` | Fechar gaveta/dropdown/voltar para área principal |
| `Ctrl+Up` / `PageUp` | Rolar área de chat para cima |
| `Ctrl+Down` / `PageDown` | Rolar área de chat para baixo |

## Desenvolvimento

```bash
# Executar todos os testes (incluindo testes unitários)
cargo test

# Executar testes E2E
cargo test --test e2e_cli
cargo test --test e2e_tui
cargo test --test tui_flow_e2e

# Executar testes BDD
cargo test --test bdd

# Formatar código
cargo fmt

# Verificação estática Clippy
cargo clippy
```

## Stack Tecnológico

| Dependência | Propósito |
|-------------|-----------|
| `tokio` | Runtime assíncrono |
| `reqwest` | Cliente HTTP, requisições SSE em streaming |
| `serde` / `serde_json` | Serialização e desserialização |
| `anyhow` | Tratamento de erros |
| `axum` / `tower-http` | Framework HTTP Server e CORS |
| `ratatui` / `crossterm` | Renderização TUI e eventos de terminal |
| `colored` | Saída colorida no terminal |
| `clap` | Análise de argumentos de linha de comando |
| `notify` | Recarga em calor do arquivo de configuração |
| `regex` | Correspondência regex |
| `html2md` | Conversão de HTML para Markdown |
| `jsonc-parser` | Parsing de arquivos de configuração JSONC |
| `tauri` | Framework de aplicativo desktop (v2) |

## Armazenamento de Dados

Os dados de sessão são salvos em formato `.jsonl` sob o diretório de configuração da plataforma:
- **Linux**: `~/.config/fi-code/sessions/`
- **macOS**: `~/Library/Application Support/fi-code/sessions/`
- **Windows**: `%APPDATA%\fi-code\sessions\`

Os arquivos de log são persistidos em `~/.config/fi-code/logs/` (escritos de forma assíncrona por `LogFileWriter`).

## Licença

Este projeto é de código aberto sob a [Licença MIT](./LICENSE).

Copyright (c) 2025 fi-code contributors.

---

> **Nota**: Este projeto está em uma fase inicial de desenvolvimento. As APIs e os formatos de configuração podem mudar.
