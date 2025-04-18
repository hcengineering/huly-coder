## Building Huly Coder

To build the Huly Coder image, run the following command:

```bash
docker build -t huly-coder -f "./Dockerfile" .
```

## Running Huly Coder

To run the Huly Coder image, run the following command:

```bash
docker run -it --rm -v "$(pwd)/target/workspace:/workspace" -e WORKSPACE_DIR=/workspace -e OPEN_ROUTER_API_KEY=<your-api-key> huly-coder
```

Replace `<your-api-key>` with your OpenRouter API key.

The agent will use the `target/workspace` directory as working directory and perform the simple task "Create Sokoban
game"
