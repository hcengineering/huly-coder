# Changelog

## [0.3.1](https://github.com/hcengineering/huly-coder/compare/huly-coder-v0.3.0...huly-coder-v0.3.1) (2025-06-16)


### Bug Fixes

* Fix npm package ([dc94aa0](https://github.com/hcengineering/huly-coder/commit/dc94aa0d0ea3ffe7b20b1e9ef19c6535ab5f999e))
* Message color fix in TUI ([7c806c9](https://github.com/hcengineering/huly-coder/commit/7c806c99843d6625faac199b5a055013e6999a57))

## [0.3.0](https://github.com/hcengineering/huly-coder/compare/huly-coder-v0.2.1...huly-coder-v0.3.0) (2025-06-15)


### Features

* rewrite OpenRouter provider to support image data response from MCP tools, add example for puppeteer MCP server ([8eee6d0](https://github.com/hcengineering/huly-coder/commit/8eee6d044873bb402f1d4cdfe1332e7a05077606))


### Bug Fixes

* Fix possible crash in messages view, Update MCP declaration example in config, adjust theme colors ([0ce34fd](https://github.com/hcengineering/huly-coder/commit/0ce34fd1eb94349b946c48987a4bd318c2829616))

## [0.2.1](https://github.com/hcengineering/huly-coder/compare/huly-coder-v0.2.0...huly-coder-v0.2.1) (2025-06-03)


### Bug Fixes

* Split long messages for correct scrolling ([61b886f](https://github.com/hcengineering/huly-coder/commit/61b886f39d2cdf9851ad76fe08a3adcc31065163))

## [0.2.0](https://github.com/hcengineering/huly-coder/compare/huly-coder-v0.1.1...huly-coder-v0.2.0) (2025-06-03)


### Features

* Permission mode for tool execution ([0c990b6](https://github.com/hcengineering/huly-coder/commit/0c990b6206f4a962c5b6801fa5992ec1bfb20518))

## [0.1.1](https://github.com/hcengineering/huly-coder/compare/huly-coder-v0.1.0...huly-coder-v0.1.1) (2025-05-30)


### Bug Fixes

* change quit shortcut to Ctrl+w ([df8de85](https://github.com/hcengineering/huly-coder/commit/df8de850c32f5e5d0a2047e567636ac5c326be0f))
* Correct handle not result from tools ([6bc3bff](https://github.com/hcengineering/huly-coder/commit/6bc3bfff39bfa2b738ab42e9016c01bbb137e379))
* improve error handling for MCP client initialization and opening with context messages ([d977735](https://github.com/hcengineering/huly-coder/commit/d977735044ea324e2a106bcdc604022272ff221c))

## 0.1.0 (2025-05-29)


### Features

* Add Anthropic provider, fix in tools ([6683a60](https://github.com/hcengineering/huly-coder/commit/6683a60baca9e0e6b09c765a556de5db97918ec7))
* Add current agent status indication ([40a92b2](https://github.com/hcengineering/huly-coder/commit/40a92b22067965e2c9e27d24b3e6d5ce888b9a99))
* Add MCP servers support ([cbfeaf3](https://github.com/hcengineering/huly-coder/commit/cbfeaf3ea141134a204ade7121104f3e70750c65))
* Add memory support and tools for agent ([3571da2](https://github.com/hcengineering/huly-coder/commit/3571da28622a01da92c50d1daa61ab8dc8910575))
* add model info with context len and usage cost ([1cc1df8](https://github.com/hcengineering/huly-coder/commit/1cc1df86360eb9838c6220d52780a82da6e5b589))
* Add new task functionality and hotkey ([e247f3d](https://github.com/hcengineering/huly-coder/commit/e247f3d2833e9958b2bfaab39a1c11d5f6563ae2))
* add possibility to input in terminal windows ([a3bdf83](https://github.com/hcengineering/huly-coder/commit/a3bdf83458afad40110b5f164044b2a83c7c210f))
* Add support for Anthropic Claude 4 models ([73c9263](https://github.com/hcengineering/huly-coder/commit/73c926336489ccf27434045be460428897831a33))
* Add web fetch tool(support direct and chrome based fetch) ([70b9f8a](https://github.com/hcengineering/huly-coder/commit/70b9f8a9296dab7e14957ee2bc1e098a76129377))
* Improve memory management instructions ([12c1c7e](https://github.com/hcengineering/huly-coder/commit/12c1c7ece9107b905e33dfe3496e0bd202c4f002))
* Merge branch 'memory' ([2b6c6b8](https://github.com/hcengineering/huly-coder/commit/2b6c6b8d120f6c17db4722e246214aae925025d2))
* Rework terminal command tools, add support for long-running command ([7c8c31f](https://github.com/hcengineering/huly-coder/commit/7c8c31f596a41806590c61d1e300c85d1c9e8839))
* soft world wrap in promt field, layou fixes ([1d2fbee](https://github.com/hcengineering/huly-coder/commit/1d2fbeebd53e17e520d9b9bddceadf0e97c99f8b))
* update file system work to support huge workspaces ([f14a992](https://github.com/hcengineering/huly-coder/commit/f14a992451c67022282d7c00e581a6f76ba55fcf))
* Web Search Tool (SearX, Brave) ([f2b096c](https://github.com/hcengineering/huly-coder/commit/f2b096ce1436330a1ca5f310761281c1b846f7a8))


### Bug Fixes

* attemp complete incorrect state resolve ([d45fd38](https://github.com/hcengineering/huly-coder/commit/d45fd384c1d51c5ebfe7516205f3ae75d3ee5a3a))
* Error handling fixes ([73c9263](https://github.com/hcengineering/huly-coder/commit/73c926336489ccf27434045be460428897831a33))
* error message dont hide on unpause, incorrect formatting asisstant messages ([4df2ce9](https://github.com/hcengineering/huly-coder/commit/4df2ce984593a84acd63b8fbae236474b3081a23))
* Fix memory embedding initialization ([d254a0b](https://github.com/hcengineering/huly-coder/commit/d254a0baf50ec6af392bcafe54d79be0551c75ca))
* Fix memory tool_info, improve shortcuts and key management ([18103ff](https://github.com/hcengineering/huly-coder/commit/18103ff13b85c73a8c55f89e32dceb4b16c8267b))
* Fix workflow on ask question tool ([29cf32c](https://github.com/hcengineering/huly-coder/commit/29cf32c027386937f76ac4edbbbd11e53b040782))
* generalize Error for tools ([59cf099](https://github.com/hcengineering/huly-coder/commit/59cf099adaade3bf5c66881f630e4e592235b008))
* generalize Error for tools ([428fcc6](https://github.com/hcengineering/huly-coder/commit/428fcc622281b4be343e8335ee053b374d67f0b7))
* incorrect read_file tool result for curly braces ([3571da2](https://github.com/hcengineering/huly-coder/commit/3571da28622a01da92c50d1daa61ab8dc8910575))
* OpenRouter messages handling, add App title ([1cc1df8](https://github.com/hcengineering/huly-coder/commit/1cc1df86360eb9838c6220d52780a82da6e5b589))
* remove list_code_definition_names from system prompt ([a5d64b1](https://github.com/hcengineering/huly-coder/commit/a5d64b14982be5571d068ab5a0c4cab8012ddf07))
* Rework history and message view ([e459fde](https://github.com/hcengineering/huly-coder/commit/e459fde5fc86d438afd464a4aadad8f577b0eb7f))
* Update RIG dependecies with fixes ([de7afdf](https://github.com/hcengineering/huly-coder/commit/de7afdfa9f233942f46ced485f2602aaa02ffdb0))
