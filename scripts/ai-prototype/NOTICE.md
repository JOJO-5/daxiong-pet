# Third-party materials

`lfm-history-template.jinja` is copied without content changes from `tokenizer.chat_template` in LiquidAI's `LFM2.5-350M-QAD-Q4_0.gguf`, repository `LiquidAI/LFM2.5-350M-GGUF`, revision `657e078c94084481950a2d555a941481f715536b`. Origin: Liquid AI, Inc. Licensed under LFM Open License v1.0; see `LFM-LICENSE.txt`. License text line endings/trailing whitespace were normalized; terms are unchanged. The template is separate from the project's own prototype scripts and does not change their license.

Template SHA-256: `70278c3c69a31e89c2383bb2c4cb5f22ec8456069bcd194d553c30a00dbe1b05`.

The research evidence contains model-provided chat templates from the same LiquidAI artifacts and `lmstudio-community/Qwen3-0.6B-GGUF`, revision `3334d820ab76652cf6e242d7c6302b10f0951f23` (Qwen3, Alibaba Cloud, Apache-2.0). See `QWEN-LICENSE.txt` for the Qwen license. Model weights and runtime binaries are not distributed here.

Fourth-round evidence also contains model-provided templates and configuration from `lmstudio-community/Qwen3.5-0.8B-GGUF`, revision `26bab2c9369648924251c0ebb3dae012f5147707`, derived from Alibaba Cloud Qwen3.5-0.8B. Apache-2.0; original model LICENSE at revision `2fc06364715b967f1860aea9cf38778875588b17` is preserved in `QWEN35-LICENSE.txt` (line endings/trailing whitespace normalized; Copyright 2026 Alibaba Cloud retained). The 527,502,816-byte language GGUF and 207,345,952-byte BF16 projector are pinned in `docs/research/chat-model-candidates-2026-10-10.json`; neither weights nor binaries are distributed here.

Qwen3.5-2B text trial templates/configuration come from `lmstudio-community/Qwen3.5-2B-GGUF`, revision `bb84e11355a036e28f080c7793fa6d22b7c4e344`, derived from `Qwen/Qwen3.5-2B`, revision `15852e8c16360a2fea060d615a32b45270f8a8fc`. Its original Apache-2.0 LICENSE matches `QWEN35-LICENSE.txt` after the same whitespace normalization. Its 671,372,416-byte vision projector is metadata only and was not downloaded/tested.

The pure-text quality comparison uses official `Qwen/Qwen2.5-1.5B-Instruct-GGUF`, revision `91cad51170dc346986eccefdc2dd33a9da36ead9`, based on `Qwen/Qwen2.5-1.5B-Instruct`, revision `989aa7980e4cf806f80c7fef2b1adb7bc71aa306`. Its Apache-2.0 LICENSE matches `QWEN-LICENSE.txt` after whitespace normalization (Copyright 2024 Alibaba Cloud). Trial props/templates are third-party materials under those terms, not project-authored templates.
