.PHONY: pass analysis echo tg_ir audit audit-report \
	transfer div-before-mul shared-var-get-shared shared-var-get-invoke unsafe-math reentrancy round variable struct-member admin-func public-func tautology lock-callback non-callback-private non-private-callback incorrect-json-type complex-loop \
	clean clean_pass clean_demo clean_tg clean_tmp lint lint-diff lint-fix

# SHELL := /bin/bash # Use bash syntax

export
# Config Env
TOP = $(shell pwd)

ifeq ($(shell uname -s), Linux)
	LLVM_DIR = $(shell llvm-config-21 --obj-root)
else ifeq ($(shell uname -s), Darwin)
	LLVM_DIR = $(shell brew --prefix llvm)
else
	LLVM_DIR = $(shell llvm-config --obj-root)
endif

# Required by llvm-sys so cargo build finds the correct LLVM installation
export LLVM_SYS_211_PREFIX = $(LLVM_DIR)

# Python interpreter — uses uv venv if available, falls back to python3
PYTHON = $(shell test -f ${TOP}/.venv/bin/python && echo ${TOP}/.venv/bin/python || echo python3)

# Binaries
LLVM_CONFIG = ${LLVM_DIR}/bin/llvm-config
LLVM_CLANG  = ${LLVM_DIR}/bin/clang
LLVM_OPT    = ${LLVM_DIR}/bin/opt

# Flags
INCLUDE = -I${TOP}/detectors
CXXFLAGS = $(shell $(LLVM_CONFIG) --cxxflags) ${INCLUDE} -fno-rtti -fPIC
LDFLAGS = $(shell $(LLVM_CONFIG) --ldflags) -shared

OPTFLAGS = -enable-new-pm=0

ifeq ($(shell uname -s), Darwin)
	LDFLAGS += -undefined dynamic_lookup
endif


ifndef NEAR_TG_DIR
	NEAR_TG_DIR=${NEAR_SRC_DIR}
endif

ifndef TMP_DIR
	TMP_DIR=${TOP}/.tmp/
endif

export TG_MANIFESTS = $(shell find ${NEAR_SRC_DIR} -name "Cargo.toml" -not -path "**/node_modules/*")


echo:
	@echo "NEAR_SRC_DIR = ${NEAR_SRC_DIR}"
	@echo "NEAR_TG_DIR  = ${NEAR_TG_DIR}"
	@echo "TG_MANIFESTS = ${TG_MANIFESTS}"

pass:
	make -C detectors rust-pass

tg_ir:
	-@for i in ${TG_MANIFESTS} ; do \
		cargo rustc --target wasm32-unknown-unknown --manifest-path $$i -- -Awarnings --emit=llvm-ir,llvm-bc ; \
	done
	@mkdir -p ${TMP_DIR}
	@make -C ${TOP} get-packages-name
	@cat ${TMP_DIR}/.packages-name.tmp | xargs -I {} find ${NEAR_TG_DIR} -name '{}*.bc' > ${TMP_DIR}/.bitcodes.tmp


get-packages-name:
	@$(PYTHON) ./utils/getPackagesName.py

analysis: unsafe-math round reentrancy div-before-mul transfer timestamp promise-result upgrade-func self-transfer prepaid-gas unhandled-promise yocto-attach complex-loop \
	tautology unused-ret inconsistency lock-callback non-callback-private non-private-callback incorrect-json-type

callback: tg_ir
	@rm -f ${TMP_DIR}/.callback.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p callback -q
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/callback {}

ext-call-trait: tg_ir
	@rm -f ${TMP_DIR}/.ext-call-trait.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p ext_call_trait -q
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/ext_call_trait {}

ext-call: tg_ir ext-call-trait
	@rm -f ${TMP_DIR}/.ext-call.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p ext_call -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/ext_call {}

complex-loop: tg_ir
	rm -f ${TMP_DIR}/.$@.tmp
	cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p complex_loop -q
	if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/complex_loop {}

unsafe-math: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp ${TMP_DIR}/.$@-toml.tmp
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/unsafe-math-toml.py ${NEAR_SRC_DIR}
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p unsafe_math -q
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/unsafe_math {}

round: tg_ir
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p round -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/round {}

struct-member: tg_ir find-struct
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p struct_member -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/struct_member {}

reentrancy: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p reentrancy -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/reentrancy {}

div-before-mul: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p div_before_mul -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/div_before_mul {}

transfer: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p transfer -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/transfer {}

timestamp: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p timestamp -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/timestamp {}

promise-result: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p promise_result -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/promise_result {}

upgrade-func: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p upgrade_func -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/upgrade_func {}

self-transfer: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p self_transfer -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/self_transfer {}

prepaid-gas: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p prepaid_gas -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/prepaid_gas {}

all-call: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p all_call -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/all_call {}

unhandled-promise: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p unhandled_promise -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/unhandled_promise {}

yocto-attach: tg_ir callback
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p yocto_attach -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/yocto_attach {}

storage-gas: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p storage_gas -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/storage_gas {}

unregistered-receiver: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p unregistered_receiver -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/unregistered_receiver {}

unsaved-changes: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p unsaved_changes -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/unsaved_changes {}

nep%-interface: tg_ir
	@rm -f ${TMP_DIR}/.nep$*-interface.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p nep_interface -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@echo "\e[33m[*] Checking interfaces of NEP-$*\e[0m"  #]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/nep_interface --nep-id $* {}

unclaimed-storage-fee: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p unclaimed_storage_fee -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/unclaimed_storage_fee {}

nft-approval-check: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p nft_approval_check -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/nft_approval_check {}

nft-owner-check: tg_ir
	@rm -f ${TMP_DIR}/.$@.tmp
	@cargo build --release --manifest-path ${TOP}/detectors/Cargo.toml -p nft_owner_check -q
	@if test $(shell cat ${TMP_DIR}/.bitcodes.tmp | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@cat ${TMP_DIR}/.bitcodes.tmp | xargs -I {} ${TOP}/detectors/target/release/nft_owner_check {}

tautology:
	@rm -f ${TMP_DIR}/.$@.tmp
	@if test $(shell find ${NEAR_SRC_DIR}// -name '*.rs' | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/tautology.py ${NEAR_SRC_DIR}

unused-ret: all-call
	@rm -f ${TMP_DIR}/.$@.tmp
	@if test $(shell find ${NEAR_SRC_DIR}// -name '*.rs' | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/unused-ret.py ${NEAR_SRC_DIR}

inconsistency:
	@rm -f ${TMP_DIR}/.$@.tmp
	@if test $(shell find ${NEAR_SRC_DIR}// -name '*.rs' | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/inconsistency.py ${NEAR_SRC_DIR}

lock-callback: callback
	@rm -f ${TMP_DIR}/.$@.tmp
	@if test $(shell find ${NEAR_SRC_DIR}// -name '*.rs' | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/lock-callback.py ${NEAR_SRC_DIR}

non-callback-private: callback
	@rm -f ${TMP_DIR}/.$@.tmp
	@if test $(shell find ${NEAR_SRC_DIR}// -name '*.rs' | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/non-callback-private.py ${NEAR_SRC_DIR}

non-private-callback: callback
	@rm -f ${TMP_DIR}/.$@.tmp
	@if test $(shell find ${NEAR_SRC_DIR}// -name '*.rs' | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/non-private-callback.py ${NEAR_SRC_DIR}

incorrect-json-type: find-struct
	@rm -f ${TMP_DIR}/.$@.tmp
	@if test $(shell find ${NEAR_SRC_DIR}// -name '*.rs' | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/incorrect-json-type.py ${NEAR_SRC_DIR}

public-interface:
	@rm -f ${TMP_DIR}/.$@.tmp
	@if test $(shell find ${NEAR_SRC_DIR}// -name '*.rs' | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/public-interface.py ${NEAR_SRC_DIR}

dup-collection-id:
	@rm -f ${TMP_DIR}/.$@.tmp
	@if test $(shell find ${NEAR_SRC_DIR}// -name '*.rs' | wc -c) -gt 0 ; then \
		command -v figlet >/dev/null 2>&1 && figlet $@ || echo "=== $@ ==="; \
	else \
		echo -e "\e[31m[!] Source not found\e[0m" ; \
	fi  # ]]
	@$(PYTHON) ./detectors/dup-collection-id.py ${NEAR_SRC_DIR}

find-struct:  # provide .struct.tmp and .struct-member.tmp
	@$(PYTHON) ./utils/findStruct.py ${NEAR_SRC_DIR}

audit: promise-result reentrancy transfer timestamp div-before-mul unsafe-math round find-struct upgrade-func self-transfer prepaid-gas unhandled-promise yocto-attach complex-loop \
	tautology unused-ret inconsistency lock-callback non-callback-private non-private-callback incorrect-json-type
	@$(PYTHON) ./utils/audit.py ${NEAR_SRC_DIR}

audit-report:
	@$(PYTHON) ./utils/audit.py ${NEAR_SRC_DIR}

clean: clean_pass clean_example clean_tg
clean_pass:
	make -C detectors clean
	make -C detectors rust-clean
clean_example:
	find examples -name "Cargo.toml" | xargs -I {} cargo clean --manifest-path={}
clean_tg:
	@for i in ${TG_MANIFESTS} ; do \
		cargo clean --manifest-path=$$i ; \
	done

clean_tmp:
	rm -rf ${TMP_DIR}

compile_commands.json: clean_pass
	if [[ $(shell bear --version | cut -d' ' -f2) = 2.4.* ]] ; then \
		bear make -C detectors pass ; \
	else \
		bear -- make -C detectors pass ; \
	fi

compile_flags.txt: Makefile
	echo ${LLVM_CLANG} ${CXXFLAGS} ${LDFLAGS} | sed 's/ /\n/g' > compile_flags.txt

lint:
	rm -f clang-tidy-fixes.yaml
	${LLVM_DIR}/bin/clang-tidy --quiet --export-fixes=clang-tidy-fixes.yaml detectors/*.cpp -- ${CXXFLAGS}

lint-fix:
	${LLVM_DIR}/bin/clang-apply-replacements --style=file ${TOP}

format:
	${LLVM_DIR}/bin/clang-format -i detectors/*.cpp detectors/*.h
	 find . -name "Cargo.toml" | xargs -I {} cargo fmt --manifest-path={}
