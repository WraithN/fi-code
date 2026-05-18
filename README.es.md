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

Un Agente de Codificación AI de terminal construido en Rust, que interactúa con los usuarios mediante REPL, TUI, HTTP Server o Desktop. Soporta conversaciones de múltiples turnos, llamadas a herramientas, persistencia de sesiones y extensiones de protocolo MCP.

## Características

- **🤖 Soporte Multi-Modelo**: Interfaces unificadas compatibles con OpenAI y Anthropic con respuestas SSE en streaming. Mecanismo de reintento integrado (retroceso exponencial + full jitter).
- **🔧 Llamadas a Herramientas**: 20 herramientas integradas incluyendo `bash`, `read`, `write`, `edit`, `web_fetch`, `grep`, `glob` y una suite completa de herramientas Git. El Agente se ejecuta automáticamente según las respuestas del modelo y devuelve resultados.
- **💬 Persistencia de Sesiones**: Las sesiones se escriben incrementalmente en el disco local en formato JSON Lines, soportando reanudación después de interrupciones.
- **🖥️ Interacción Multi-Modo**:
  - **CLI REPL**: Interacción tradicional por línea de comandos (`fi-code-cli -i`)
  - **TUI**: Interfaz de terminal completa basada en `ratatui` (`fi-code-tui`)
  - **HTTP Server**: API REST + respuestas SSE en streaming (`fi-code-server` o `fi-code-cli server`)
  - **Desktop**: Aplicación de escritorio Tauri v2 con sidecar integrado (`fi-code-desktop`)
- **🛡️ Validación de Permisos**: Niveles de riesgo para operaciones de alto riesgo como Bash (Allow / Ask / Deny), interceptando `sudo`, `rm -rf` y ataques de inyección comunes.
- **⚙️ Configuración Flexible**: Soporta `~/.config/fi-code/config.json` o `config.jsonc`, con comentarios, marcadores de posición para variables de entorno (`{env:VAR_NAME}`) y recarga en caliente (debounce de 500ms).
- **🔗 Soporte MCP**: Implementación completa del Model Context Protocol, soportando gestión multi-servidor (transporte stdio / HTTP) con reconexión automática (hasta 3 reintentos, retroceso exponencial).
- **📦 Sistema de Skills**: Mecanismo extensible de registro y carga de Skills. El Agente puede cargar instrucciones Skill específicas del proyecto bajo demanda mediante la herramienta `use_skill`.

## Inicio Rápido

### Requisitos

- [Rust](https://rustup.rs/) 1.70+ (se recomienda la última versión estable)
- Node.js 18+ (solo requerido para construir el frontend Desktop)
- Clave API del proveedor de AI correspondiente

### Instalación

```bash
# Clonar el repositorio
git clone <repository-url>
cd fi-code

# Construir todos los binarios
cargo build --release

# Ejecutar (ver Uso más abajo)
cargo run --bin fi-code-cli -- --help
```

### Configuración

#### Método 1: Variables de Entorno (Mayor Prioridad)

**Compatible con OpenAI:**
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

Otros proveedores preestablecidos también soportan variables de entorno: `GLM_*`, `KIMI_*`, `DEEPSEEK_*`, `QWEN_*` / `DASHSCOPE_*`.

#### Método 2: Archivo de Configuración

Rutas de archivos de configuración (buscadas en orden de prioridad):
- Linux/macOS: `~/.config/fi-code/config.jsonc` o `~/.config/fi-code/config.json`

Ejemplo:
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

Soporta comentarios `//` y `/* */`. `apiKey` soporta la sintaxis de marcador de posición `{env:VAR_NAME}`. Los proveedores preestablecidos (openai, anthropic, glm, kimi, qwen, deepseek) se fusionan automáticamente en la configuración.

### Uso

```bash
# Modo REPL interactivo
cargo run --bin fi-code-cli -- -i

# Modo interfaz TUI de terminal completa
cargo run --bin fi-code-tui

# Ejecutar un solo comando y salir
cargo run --bin fi-code-cli -- -c "Escribe un Hello World en Rust"

# Ver proveedores y modelos configurados
cargo run --bin fi-code-cli -- --models

# Ver lista de sesiones
cargo run --bin fi-code-cli -- -s

# Especificar directorio de trabajo
cargo run --bin fi-code-cli -- -i -w /path/to/project

# Iniciar servidor HTTP
cargo run --bin fi-code-server
# o
cargo run --bin fi-code-cli -- server --port 4040
```

**Nota**: Ejecutar `fi-code-cli` sin ninguna bandera iniciará automáticamente el modo TUI.

### Desarrollo Desktop

La aplicación Desktop usa una arquitectura "shell Tauri + sidecar integrado":

```bash
# Primero construir el binario sidecar
cargo build

# Instalar dependencias del frontend e iniciar modo dev Desktop
cd frontend && npm install
cargo tauri dev

# Build de producción
cargo tauri build
```

## Estructura del Proyecto

Este proyecto utiliza una estructura **Cargo Workspace**:

```
.
├── Cargo.toml              # Definición del Workspace
├── crates/
│   ├── core/               # Biblioteca core (fi-code-core): toda la lógica de negocio
│   ├── cli/                # Entrada binaria CLI (fi-code-cli)
│   ├── tui/                # Entrada binaria TUI (fi-code-tui)
│   ├── server/             # Entrada binaria Server (fi-code-server)
│   ├── shared/             # DTOs compartidos y constantes (fi-code-shared)
│   └── utils/              # Utilidades de prueba (fi-code-utils)
├── src-tauri/              # Aplicación Desktop Tauri (fi-code-desktop)
├── frontend/               # Frontend Tauri (React + Vite + Tailwind)
└── tests/                  # Pruebas E2E y BDD (fi-code-tests)
```

## Herramientas Integradas

| Herramienta | Descripción | Nivel de Riesgo |
|-------------|-------------|-----------------|
| `bash` | Ejecutar comandos shell | Ask (Comandos peligrosos Deny) |
| `read` / `read_file` | Leer contenido de archivos | Allow |
| `write` | Escribir en archivo | Ask |
| `edit` | Editar archivo | Ask |
| `web_fetch` | Obtener página web y convertir a Markdown | Ask |
| `grep` | Búsqueda regex en contenido de archivos | Allow |
| `glob` | Coincidencia de rutas de archivos | Allow |
| `git` | Ejecutar comandos git | Ask |
| `git_status` | Git status | Allow |
| `git_diff` | Git diff | Allow |
| `git_add` | Git add | Ask |
| `git_commit` | Git commit | Ask |
| `git_log` | Git log | Allow |
| `git_worktree` | Operaciones Git worktree | Ask |
| `create_task_plan` | Crear plan de tareas | Ask |
| `handle_task_plan` | Ejecutar plan de tareas | Ask |
| `ask_for_question` | Solicitar entrada del usuario | Ask |
| `use_skill` | Cargar y usar Skill | Ask |
| `mcp:*` | Herramientas MCP (cargadas dinámicamente) | Depende de config MCP |

## Mecanismos de Seguridad

- **Protección contra Escape de Rutas**: Todas las operaciones de archivos pasan por verificaciones `safe_path` para asegurar que no excedan el directorio de trabajo.
- **Sandbox Bash**: Limpia las variables de entorno heredadas, mantiene solo las variables mínimas necesarias (`PATH=/usr/bin:/bin`, `HOME`), y aplica un timeout de 120 segundos.
- **Niveles de Permiso**: Deny (rechazar directamente comandos peligrosos), Ask (confirmación interactiva), Allow (operaciones de solo lectura pasan directamente).
- **Truncamiento de Salida**: El contenido retornado por las herramientas está limitado a 50,000 caracteres para prevenir desbordamiento de contexto.

## Atajos TUI

En modo TUI, los siguientes atajos están disponibles:

| Atajo | Función |
|-------|---------|
| `Tab` / `Shift+Tab` | Cambiar área de enfoque |
| `Ctrl+C` | Detener generación / salir del programa |
| `Ctrl+B` | Abrir/cerrar cajón de archivos izquierdo |
| `Ctrl+H` | Abrir/cerrar cajón de historial de sesiones derecho |
| `Ctrl+M` | Abrir menú desplegable de selección de modelo |
| `Ctrl+T` | Cambiar tema |
| `Ctrl+N` | Nueva sesión |
| `Enter` | Enviar mensaje |
| `Shift+Enter` | Nueva línea en el cuadro de entrada |
| `Esc` | Cerrar cajón/menú desplegable/volver al área principal |
| `Ctrl+Up` / `PageUp` | Desplazar área de chat hacia arriba |
| `Ctrl+Down` / `PageDown` | Desplazar área de chat hacia abajo |

## Desarrollo

```bash
# Ejecutar todas las pruebas (incluyendo pruebas unitarias)
cargo test

# Ejecutar pruebas E2E
cargo test --test e2e_cli
cargo test --test e2e_tui
cargo test --test tui_flow_e2e

# Ejecutar pruebas BDD
cargo test --test bdd

# Formatear código
cargo fmt

# Verificación estática Clippy
cargo clippy
```

## Stack Tecnológico

| Dependencia | Propósito |
|-------------|-----------|
| `tokio` | Runtime asíncrono |
| `reqwest` | Cliente HTTP, peticiones SSE en streaming |
| `serde` / `serde_json` | Serialización y deserialización |
| `anyhow` | Manejo de errores |
| `axum` / `tower-http` | Framework HTTP Server y CORS |
| `ratatui` / `crossterm` | Renderizado TUI y eventos de terminal |
| `colored` | Salida de color en terminal |
| `clap` | Análisis de argumentos de línea de comandos |
| `notify` | Recarga en caliente del archivo de configuración |
| `regex` | Coincidencia regex |
| `html2md` | Conversión de HTML a Markdown |
| `jsonc-parser` | Parsing de archivos de configuración JSONC |
| `tauri` | Framework de aplicación de escritorio (v2) |

## Almacenamiento de Datos

Los datos de sesión se guardan en formato `.jsonl` bajo el directorio de configuración de la plataforma:
- **Linux**: `~/.config/fi-code/sessions/`
- **macOS**: `~/Library/Application Support/fi-code/sessions/`
- **Windows**: `%APPDATA%\fi-code\sessions\`

Los archivos de registro se persisten en `~/.config/fi-code/logs/` (escritos de forma asíncrona por `LogFileWriter`).

## Licencia

Este proyecto es de código abierto bajo la [Licencia MIT](./LICENSE).

Copyright (c) 2025 fi-code contributors.

---

> **Nota**: Este proyecto está en una etapa temprana de desarrollo. Las APIs y los formatos de configuración pueden cambiar.
