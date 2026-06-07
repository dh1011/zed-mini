# Zed Mini

Zed Mini is a minimal fork of [Zed](https://github.com/zed-industries/zed), focused on local text editing.

I like Zed a lot. The design is beautiful and it is fast. But for my own use, I want a focused editor rather than a full IDE, so this fork strips out features I do not use.

Zed Mini removes:

- AI features
- Terminal integration
- Plugin runtime
- Collaboration
- Cloud sign-in
- Telemetry and feedback flows
- Auto-update
- GitHub integration
- Local git/libgit2 integration
- Project diagnostics
- App database persistence

It keeps the core editing experience and LSP support for language-aware editing.

The executable is down from roughly `400 MB` to around `100 MB`.

Currently macOS Apple Silicon only.

---
Zed Mini is an unofficial modified fork of [Zed](https://github.com/zed-industries/zed) and is not affiliated with or endorsed by Zed Industries. See [LICENSE-GPL](LICENSE-GPL) and [LICENSE-APACHE](LICENSE-APACHE).
