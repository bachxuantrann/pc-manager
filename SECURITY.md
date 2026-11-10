# Security policy

## Supported versions

Only the [latest release](https://github.com/bachxuantrann/pc-manager/releases/latest) receives
security fixes.

## Reporting a vulnerability

PC Manager can run with administrator rights and deletes files, so security reports are taken
seriously.

**Please do not open a public issue for security problems.** Instead, report them privately
through GitHub: open the repository's **Security** tab and choose
[**Report a vulnerability**](https://github.com/bachxuantrann/pc-manager/security/advisories/new).

Please include:

- the affected version;
- steps to reproduce, or a proof of concept;
- the impact (for example: deleting files outside the cleaning rules, gaining administrator rights,
  ending protected processes).

You can expect an acknowledgement within a few days. Once a fix is released, the advisory will be
published with credit to the reporter unless you prefer to remain anonymous.

## Scope

Of particular interest:

- any way to make the app delete or move files outside its rules, protected folders or the
  Recycle Bin paths described in [docs/SAFETY.md](docs/SAFETY.md);
- path traversal, symlink or junction tricks that bypass those checks;
- privilege escalation through the start-with-Windows task or the elevation flow;
- ending or tampering with protected system processes.
