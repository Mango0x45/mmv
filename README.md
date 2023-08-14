# mmv — mapped file moves

The `mmv` utility is a command-line tool to help you easily and safely perform
complex file moves and -renamings.  Unlike basically all file renaming tools I
have seen online, this utility does not limit you to using specific built-in
functions which offer only part of the functionality one might need.  This
utility also tries to be as safe as possible — you would be amazed at how many
file renaming tools lose your data when you try something as simple as swapping
two files.

## Installation

Installation is very easy.  This is written in Rust, so it’s assumed you have
`cargo` installed on your system.

```
$ make
$ sudo make install
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
