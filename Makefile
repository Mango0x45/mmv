.POSIX:

PREFIX  = /usr
DPREFIX = ${DESTDIR}${PREFIX}
BINDIR  = ${DPREFIX}/bin
MANDIR  = ${DPREFIX}/share/man/man1

target = target/release/mmv

all:
	@echo "Run “cargo build [-r]” to build"

install:
	mkdir -p ${BINDIR} ${MANDIR}
	cp ${target} ${BINDIR}
	cp man/mmv.1 ${MANDIR}
