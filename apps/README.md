# Apps (future)

This directory is deliberately empty. Pippin's application layer arrives in
this order:

- **Milestone 4–5 — desktop shell:** C++ and Rust implement the shell and
  Toolbox-facing desktop services.
- **Milestone 5 — native apps:** C++ and Rust GUI applications use the Toolbox
  API (Window/Menu/Control managers, event loop).
- **Milestone 6 — optional C# apps (x86-64 only):** a managed runtime and
  bindings let C# applications use the same Toolbox API.

C# is an optional application language, not a dependency of the desktop shell
or kernel. It is out of scope for the current kernel milestone and the 68k port.
Vanta is another candidate for native apps and utilities once its compiler can
target Pippin and use the Toolbox ABI; it has no scheduled milestone yet.
See docs/gui.md and docs/architecture.md (§ Application layer).
