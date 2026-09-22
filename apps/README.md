# Apps (future)

This directory is deliberately empty. Pippin's application layer arrives in
this order:

- **Milestone 5 — C++ apps:** native GUI applications written in C++ against
  the Toolbox API (Window/Menu/Control managers, event loop).
- **Milestone 7 — C# shell (x86-64 desktop only):** a managed (AOT) C#
  runtime hosts the desktop shell and higher-level applications.

C# is explicitly out of scope for the current x86-64 kernel milestone and
for the future 68k port; it is tied to the x86-64 desktop plan only.
See docs/gui.md and docs/architecture.md (§ Application layer).