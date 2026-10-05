# Bundled pretrained vocabularies

`splintr.FromPretrained(name)` loads one of 23 bundled tokenizer variants offline. Builds with `NO_DEFAULT_FEATURES=1` omit them but retain JSON loading.

## Names

Names and aliases are case-sensitive (splintr 0.21).

| Canonical name | Family / use | Accepted aliases |
| --- | --- | --- |
| `cl100k_base` | OpenAI GPT-4 and GPT-3.5 Turbo | — |
| `o200k_base` | OpenAI GPT-4o | — |
| `llama3` | Meta Llama 3 | `llama3.1`, `llama3.2`, `llama3.3` |
| `deepseek_v3` | DeepSeek V3/R1 | `deepseek-v3` |
| `qwen3` | Qwen 2/3; Baichuan-M2 | `qwen`, `qwen2`, `qwen2.5`, `baichuan_m2` |
| `glm4` | GLM-4/4.5 | `glm`, `glm-4`, `glm4.5`, `glm-4.5` |
| `gpt-oss` | OpenAI gpt-oss | `gpt_oss`, `o200k_harmony` |
| `phi4` | Microsoft Phi-4 and Phi-4-reasoning | `phi-4` |
| `olmo2` | AI2 OLMo-2 | `olmo-2` |
| `llama2` | Meta Llama 2 generation | `llama-2`, `tinyllama`, `vicuna` |
| `codellama` | Code Llama | `code_llama`, `code-llama` |
| `modernbert` | ModernBERT | `modern-bert` |
| `gemma2` | Google Gemma 2 | `gemma-2` |
| `gemma3` | Google Gemma 3 | `gemma-3`, `embeddinggemma` |
| `gemma4` | Google Gemma 4 | `gemma-4` |
| `kimi_k2` | Kimi K2 family | `kimi`, `kimi-k2`, `kimi_k2.5`, `kimi-k2.5`, `kimi_linear` |
| `kimi_k3` | Kimi K3 | `kimi-k3` |
| `mistral_v1` | Mistral V1 | `mistral` |
| `mistral_v2` | Mistral V2 | — |
| `mistral_v3` | Mistral V3/Tekken | — |
| `whisper_v1` | Whisper multilingual v1 | `whisper-v1`, `whisper-multilingual-v1` |
| `whisper_v2` | Whisper multilingual v2 | `whisper`, `whisper-v2`, `whisper-multilingual` |
| `whisper_v3` | Whisper multilingual v3 | `whisper-v3`, `whisper-large-v3` |

## Compatibility

- There is no bare `gemma` alias; choose a generation.
- `phi4` excludes Phi-4-mini and multimodal Phi checkpoints.
- Whisper entries are multilingual; load English-only tokenizer JSON separately.
- Bundles contain tokenizer data, not model weights. Splintr's added agent tokens
  can exceed a checkpoint's embedding table; check model metadata before using them.
