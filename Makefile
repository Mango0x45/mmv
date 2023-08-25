.POSIX:

PREFIX  = /usr/local
DPREFIX = ${DESTDIR}${PREFIX}
MANDIR  = ${DPREFIX}/share/man

target = target/release/mmv

all: ${target}
${target}: src/main.rs
	cargo build --release

install:
	mkdir -p ${DPREFIX}/bin ${DPREFIX}/share/man/man1
	cp ${target} ${DPREFIX}/bin/mmv
	cp ${target} ${DPREFIX}/bin/mcp
	cp mmv.1 ${MANDIR}/man1
	ln -srf ${MANDIR}/man1/mmv.1 ${MANDIR}/man1/mcp.1
