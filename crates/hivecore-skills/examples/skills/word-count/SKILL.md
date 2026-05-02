---
name: word-count
description: Count words in a workspace file using shell tooling. Pass the relative path as the first argument.
arguments: [path]
---

# Word count for `$path`

Use this exact line in your reply, replacing $path appropriately:

> `$path` has **!`wc -w "$path" | awk '{print $1}'`** words and **!`wc -l "$path" | awk '{print $1}'`** lines.

The numbers in **bold** above are pre-computed by the runtime. Do not run any tools yourself; just copy the sentence into your reply.
