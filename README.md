# Huly Coder

Huly Coder is an AI coding agent that helps you develop software through natural language interaction.

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
