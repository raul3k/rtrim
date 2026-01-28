# Configurações de Identidade
BINARY_NAME=rtrim
TARGET=target/release/$(BINARY_NAME)

# Configuração de Destino (Prioriza variável de ambiente, depois default)
PREFIX ?= /usr/local/bin

# Lógica de Detecção de Privilégio
# 1. Verifica se o diretório é gravável ou se o pai do diretório (caso não exista) é gravável.
# 2. Se não for gravável, define SUDO como 'sudo'.
SUDO := $(shell [ -w $(PREFIX) ] || [ -w $(shell dirname $(PREFIX) 2>/dev/null || echo ".") ] && echo "" || echo "sudo")

all: build strip

build:
	@echo "==> Compilando $(BINARY_NAME) em modo release..."
	@cargo build --release

strip: build
	@echo "==> Removendo símbolos de debug..."
	@strip $(TARGET)

install: all
	@echo "==> Instalando em $(PREFIX) (Usando: $(SUDO) install)..."
	@$(SUDO) mkdir -p $(PREFIX)
	@$(SUDO) install -m 755 $(TARGET) $(PREFIX)
	@echo "==> Pronto. Execute '$(BINARY_NAME) --help'"

uninstall:
	@echo "==> Removendo $(BINARY_NAME) de $(PREFIX)..."
	@$(SUDO) rm -f $(PREFIX)/$(BINARY_NAME)

clean:
	@cargo clean

.PHONY: all build strip install uninstall clean