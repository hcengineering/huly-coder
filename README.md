# Huly Coder

Huly Coder is an AI coding agent that helps you develop software through natural language interaction. It provides a powerful terminal-based interface with a suite of tools for file manipulation, code generation, and project management.

## Features

- **Terminal User Interface (TUI)**: Clean and intuitive terminal interface with file tree, message history, and task status display
- **Smart File Operations**: Tools for reading, writing, searching, and modifying files with precision
- **Web Integration**: Built-in web search and URL fetching capabilities
- **Memory System**: Persistent knowledge graph for maintaining context across sessions
- **Multiple LLM Providers**: Support for OpenRouter, LMStudio, and OpenAI
- **Docker Support**: Easy containerization for portable development environments

## Requirements

- Rust 1.75 or higher
- OpenRouter API key (or alternative LLM provider credentials)
- Terminal with Unicode support

## Configuration

The agent's configuration is stored in `huly-coder.yaml`:

```yaml
provider: OpenRouter        # LLM provider (OpenRouter, LMStudio or OpenAI)
model: anthropic/claude-3.5-sonnet  # LLM model to use
workspace: ./target/workspace       # Working directory for the agent
user_instructions: |               # Custom personality/role instructions
    You are dedicated software engineer working alone. You're free to choose any technology, 
    approach, and solution. If in doubt please choose the best way you think. 
    Your goal is to build working software based on user request.
```

## Local Run

To run Huly Coder locally, run:

```bash
cargo run
```

## Docker

### Building Huly Coder

To build the Huly Coder image, run:

```bash
docker build -t huly-coder -f "./Dockerfile" .
```

### Running Huly Coder

To run the Huly Coder image:

```bash
docker run -it --rm -v "$(pwd)/target/workspace:/target/workspace" -e OPENROUTER_API_KEY=<your-api-key> huly-coder
```

Replace `<your-api-key>` with your OpenRouter API key.

The agent uses `target/workspace` as its working directory.

## Available Tools

Huly Coder comes with a comprehensive set of tools:

- **File Operations**
  - Read and write files with automatic directory creation
  - Smart file content replacement with context awareness
  - Recursive file listing and searching with regex support
  - File content search with contextual results

- **System Tools**
  - Execute system commands with safety checks
  - Interactive command execution support
  - Background process management

- **Web Tools**
  - Web search capabilities
  - URL content fetching with markdown conversion
  - API integration support

- **Memory System**
  - Knowledge graph for persistent memory
  - Entity and relationship management
  - Context-aware observation storage
  - Search and retrieval capabilities

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
