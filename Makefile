.POSIX:

PREFIX  = /usr/local
DPREFIX = ${DESTDIR}${PREFIX}

target = target/release/mmv

all: ${target}
${target}: src/main.rs
	cargo build --release

install:
	mkdir -p ${DPREFIX}/bin ${DPREFIX}/share/man/man1
	cp ${target} ${DPREFIX}/bin
	cp mmv.1 ${DPREFIX}/share/man/man1
