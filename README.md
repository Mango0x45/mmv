# mmv, mcp — mapped file moves and -copies

The `mmv` and `mcp` utilities is a command-line tool to help you easily and
safely perform complex file copies, -moves, and -renamings.  Unlike almost all
file renaming tools I have seen online, these utilities do not limit you to
using specific built-in functions which offer only part of the functionality one
might need.  These utilities also try to be as safe as possible — you would be
amazed at how many file renaming tools lose your data when you try something as
simple as swapping two files.

## Installation

Installation is very easy.  This is written in Rust, so it’s assumed you have
`cargo` installed on your system.

```
$ make
$ sudo make install
```

The following environment variables can also be set at compile-time to modify
the names of the generated binaries:

- `$MMV_NAME`
    + The name of the file-moving binary (default is `mmv`).  This is also used
      to name the backups folder in `$XDG_CACHE_HOME`.
- `$MCP_NAME`
    + The name of the file-copying binary (default is `mcp`).

If you are compiling with a custom binary name, you want to make sure that the
environment variables actually get used when performing a `make install`.  If
you’re using `sudo`, you want to do this with the `-E` flag.

```
$ MMV_NAME=mmv-rs MCP_NAME=mcp-rs make
$ MMV_NAME=mmv-rs MCP_NAME=mcp-rs sudo -E make install
```

## Examples and Documentation

To avoid repeating myself everywhere, if you would like to see usage examples
and documentation I highly suggest reading the [included manual page][1] or
reading the extended documentation [on my website][2].

## Contributions

Contributions, feedback, and suggestions are always welcome!  You are free to
open GitHub issues, email me, or do whatever else you want.  I do ask however
that contributions be made either on [sourcehut][3] or via [`git send-email`][4]
to [~mango/public-inbox@lists.sr.ht][5].  The git repositories on GitHub and my
own personal site exist purely as read-only mirrors.

[1]: mmv.1
[2]: https://thomasvoss.com/prj/mmv
[3]: https://sr.ht/~mango/mmv 
[4]: https://git-send-email.io/
[5]: mailto:~mango/public-inbox@lists.sr.ht
