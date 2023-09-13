.POSIX:

PREFIX  = /usr/local
DPREFIX = ${DESTDIR}${PREFIX}
MANDIR  = ${DPREFIX}/share/man

target = target/release/mmv

mmv = $${MMV_NAME:-mmv}
mcp = $${MCP_NAME:-mcp}

all: ${target}
${target}: src/main.rs
	cargo build --release

install:
	mkdir -p ${DPREFIX}/bin ${DPREFIX}/share/man/man1
	cp ${target} ${DPREFIX}/bin/${mmv}
	cp mmv.1 ${MANDIR}/man1/${mmv}.1
	ln -srf ${DPREFIX}/bin/${mmv} ${DPREFIX}/bin/${mcp}
	ln -srf ${MANDIR}/man1/${mmv}.1 ${MANDIR}/man1/${mcp}.1

clean:
	rm -rf target
