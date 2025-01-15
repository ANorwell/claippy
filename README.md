# Claippy - AI-Powered Coding Assistant

Claippy is a command-line tool that provides an interactive interface for code assistance and conversation management.

## Commands

### Basic Commands

- `repl` or no command: Start an interactive REPL session
  ```bash
  claippy
  # or
  claippy repl
  ```

- `query` or `q`: Send a one-off query
  ```bash
  claippy query How do I implement a binary search?
  # or
  claippy q How do I implement a binary search?
  ```

### Conversation Management

- `new` or `n`: Create a new conversation
  ```bash
  claippy new my-project
  # Creates a conversation with ID "my-project"
  ```

- `clear`: Clear the current conversation history
  ```bash
  claippy clear
  ```

- `history`: Display the full conversation history
  ```bash
  claippy history
  ```

### Context Management

- `add` or `a`: Add workspace context (files/directories)
  ```bash
  claippy add src/main.rs
  # or multiple files
  claippy a src/*.rs
  ```

- `ls`: List current workspace context
  ```bash
  claippy ls
  ```

#### Usage with fzf

[fzf](https://github.com/junegunn/fzf) is a command line fuzzy finder tool. Setting
up fzf reduces friction with selecting source code files with the add command.

For example, in `zsh` the following turns ctrl-h into a project file selection shortcut:

``` sh
populate-git-file() {
    dir=$(git rev-parse --show-toplevel 2>/dev/null || echo '.')
    files=$(rg --files $dir | fzf -m --height=15 --reverse | paste -s -)
    if [[ -n $files ]]; then
        LBUFFER+="${files}"
    fi
    zle reset-prompt
}
zle -N populate-git-file
bindkey "^h" populate-git-file
```


### REPL Commands

When in REPL mode, you can use these commands by prefixing them with `!`:

- `!new <name>`: Create new conversation
- `!clear`: Clear current conversation
- `!add <paths>`: Add context files
- `!ls`: List context
- `!history`: Show conversation history
- `!q <query>`: Execute a query

To exit the REPL, use Ctrl+C or Ctrl+D.

## Examples

```bash
# Start a new conversation about a specific project
claippy new rust-project

# Add some context files
claippy add src/*.rs

# Start an interactive session
claippy repl

# In REPL mode:
claippy> How can I improve this code?
claippy> !clear  # Clear the conversation
claippy> !ls     # Check current context



## TODO

- REPL-based file selection
- Better configuration
- Better streaming source code highlighting?
