# NOTE FOR USERS THAT WANT TO TRY THIS OUT!

The current code on the master branch doesn’t work as documented, as a newer
more powerful interface is being developed.  For the doucmented interface, you
want to build the latest stable tag:

```console
$ git clone <clone url>
$ cd mmv
$ git checkout tags/v1.0.0 -b v1.0.0-master
$ cargo build -r
$ make install
```

# mmv — multi move

`mmv` is a command-line utility to move multiple files in a more convenient
manner than for-loops.  The best way to explain this is with an example.  Say
you had the following files in the current directory:

```console
$ ls
'The Fellowship of the Ring.mp4'  'The Two Towers.mp4'
'The Return of the King.mp4'
```

You may be excited because you’re about to watch Lord of the Rings, but the
filenames have spaces in them which for various reasons is [not a very good
idea][1].  You’re hacker though, so you execute the following code in your
shell:

```console
$ for old in *; do
>         new=`echo "$old" | tr ' ' '-'`
>         mv "$old" "$new"
> done
```

Cool, that worked.  It is quite a lot to write directly into your shell though,
and if the shell you’re using has a CLI that isn’t great with multi-line input
you may find yourself with a negative experience.

But wait!  Now you decided that actually you would like your filenames to all be
in lowercase too, because it’s more consistent with the rest of your filesystem.
Well cool, now you can use your shells history to navigate to the previous loop,
navigate to the call to `tr`, and edit its parameters to `'A-Z' 'a-z'`.  But
again, if you have a shell that doesn’t take too kindly to this longer form of
script editing then you won’t have a great time (and trust me, _very few_ shells
offer a nice experience in this regard).

Let’s try to accomplish this same task using `mmv`:

```console
$ mmv *
```

After running the above command, an instance of your editor specified by the
`$EDITOR` variable gets launched with a list of all your files.  It looks like
so:

```
The Fellowship of the Ring.mp4
The Return of the King.mp4
The Two Towers.mp4
```

Now like mentioned previously, you are a hacker.  This means that you use vim
(or if you’re a _real_ hacker, emacs).  You’re not that cool though, so you use
vim.  Well thankfully vim makes renaming these files very easy:

```viml
VGu        " Go into visual-line mode, select the whole document, and lowercase
           " everything
:%s/ /-/g  " Swap all spaces for hyphens
ZZ         " Save and quit
```

…and just like that, we have turned all those pesky spaces into hyphens and
lowercased our filenames, and all it took was 15 keypresses (that includes the
enter key, and saving the file)!

## Safety

Another advantage of `mmv` is that it’s a lot safer than your typical for-loop,
or embarassingly enough even most file renaming tools I find online (lol).  What
do I mean by “safer”?  Well consider that we want to make the following very
simple file renamings:

```
foo → bar
bar → foo
```

Most tools I’ve come across for renaming files will rename `foo` to `bar`, and
then _oops!_, the original `bar` file no longer exists and we just lost our
data!  Even if you were to do this manually (imagine a lot more files) you would
need to go and move everything to a temporary directory, and move them back, and
it’s kind of just a pain to do manually.  Luckily `mmv` handles this for you!

[1]: https://superuser.com/q/29111
