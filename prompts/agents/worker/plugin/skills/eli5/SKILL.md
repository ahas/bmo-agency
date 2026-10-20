---
name: eli5
description: Explain a topic like I'm a 5 year old. Use when the user types /eli5 <topic> or asks for a dead-simple picture explainer of how something works.
license: MIT — the eli5 plugin 1.0.0 by the original author, from the claude-community marketplace
---

# eli5

Explain like I'm someone who knows nothing about this topic, as one HTML page with big pictures and few words.

- One file with everything inside it: styles inline, every picture inline SVG drawn in the page's own colours and scaled by its view box, so it reads on a phone and in either theme.
- File it in the deployment's 아카이브 with `artifact_file` as a page (`kind` `page` and its `html`) and hand over the console address `/archive/<id>`.
- Never publish it anywhere else — no claude.ai artifact, no gist, no paste.

Topic: $ARGUMENTS
