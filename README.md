<div align="center">
<p align="center">
  <a href="https://www.edgee.cloud">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://cdn.edgee.cloud/img/component-dark.svg">
      <img src="https://cdn.edgee.cloud/img/component.svg" height="100" alt="Edgee">
    </picture>
  </a>
</p>
</div>

<h1 align="center">OpenAI Chat component for Edgee</h1>

[![Coverage Status](https://coveralls.io/repos/github/edgee-cloud/openai-chat-component/badge.svg)](https://coveralls.io/github/edgee-cloud/openai-chat-component)
[![GitHub issues](https://img.shields.io/github/issues/edgee-cloud/openai-chat-component.svg)](https://github.com/edgee-cloud/openai-chat-component/issues)
[![Edgee Component Registry](https://img.shields.io/badge/Edgee_Component_Registry-Public-green.svg)](https://www.edgee.cloud/edgee/openai-chat)


This component provides a simple way to integrate the OpenAI Chat API (or other OpenAI-compatible APIs) on [Edgee](https://www.edgee.cloud),
served directly at the edge. You map the component to a specific endpoint such as `/chat`, and
then you invoke it from your frontend code.


## Quick Start

1. Download the latest component version from our [releases page](../../releases)
2. Place the `openai.wasm` file in your server (e.g., `/var/edgee/components`)
3. Add the following configuration to your `edgee.toml`:

```toml
[[components.edge_functions]]
id = "openai"
file = "/var/edgee/components/openai.wasm"
settings.edgee_path = "/chat"
settings.api_key = "sk-XYZ"
settings.model = "gpt-3.5-turbo"

# optional settings:
settings.max_completion_tokens = "100" # optional, by default it's unlimited
settings.default_system_prompt="You are a funny assistant, always adding a short joke after your response." # optional, no automatic system prompt by default
settings.api_hostname = "api.openai.com" # optional, in case you're using a different OpenAI-compatible API
```

### How to use the HTTP endpoint

You can send requests to the endpoint and show the response message as follows:

```javascript
const response = await fetch('/chat', {
  method: 'POST',
  body: JSON.stringify({
    messages: [{
        role: 'user',
        content: 'Hello! Please say "ok" if this API call is working.',
    }],
  }),
});
const json = await response.json();
console.log(json.content);
```

## Development

### Building from Source
Prerequisites:
- [Rust](https://www.rust-lang.org/tools/install)

Build command:
```bash
edgee component build
```

Test command (with local HTTP emulator):
```bash
edgee component test
```

Test coverage command:
```bash
make test.coverage[.html]
```

### Contributing
Interested in contributing? Read our [contribution guidelines](./CONTRIBUTING.md)

### Security
Report security vulnerabilities to [security@edgee.cloud](mailto:security@edgee.cloud)
