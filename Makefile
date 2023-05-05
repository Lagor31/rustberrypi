## SPDX-License-Identifier: MIT OR Apache-2.0
##
## Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

DOCKER_IMAGE := rustembedded/osdev-utils:2021.12
## Operating system
ifeq ($(shell uname -s),Linux)
    DU_ARGUMENTS = --block-size=1024 --apparent-size
else ifeq ($(shell uname -s),Darwin)
    DU_ARGUMENTS = -k -A
endif

define disk_usage_KiB
    @printf '%s KiB\n' `du $(DU_ARGUMENTS) $(1) | cut -f1`
endef

## Format
define color_header
    @tput setaf 6 2> /dev/null || true
    @printf '\n%s\n' $(1)
    @tput sgr0 2> /dev/null || true
endef

define color_progress_prefix
    @tput setaf 2 2> /dev/null || true
    @tput bold 2 2> /dev/null || true
    @printf '%12s ' $(1)
    @tput sgr0 2> /dev/null || true
endef


##--------------------------------------------------------------------------------------------------
## Optional, user-provided configuration values
##--------------------------------------------------------------------------------------------------

# Default to the RPi3.
BSP ?= rpi4
MODE ?= release


# Default to a serial device name that is common in Linux.
DEV_SERIAL ?= /dev/ttyUSB0

# Optional debug prints.
ifdef DEBUG_PRINTS
    FEATURES = --features debug_prints
endif

# Optional integration test name.
ifdef TEST
    TEST_ARG = --test $(TEST)
else
    TEST_ARG = --test '*'
endif



##--------------------------------------------------------------------------------------------------
## BSP-specific configuration values
##--------------------------------------------------------------------------------------------------

TARGET            = aarch64-unknown-none-softfloat
KERNEL_BIN        = kernel8.img
OBJDUMP_BINARY    = aarch64-linux-gnu-objdump
NM_BINARY         = aarch64-linux-gnu-nm
READELF_BINARY    = aarch64-linux-gnu-readelf
OPENOCD_ARG       = -f /openocd/tcl/interface/ftdi/olimex-arm-usb-tiny-h.cfg -f /openocd/rpi4.cfg
JTAG_BOOT_IMAGE   = ../X1_JTAG_boot/jtag_boot_rpi4.img
LD_SCRIPT_PATH    = $(shell pwd)/kernel/src/
RUSTC_MISC_ARGS   = -C target-cpu=cortex-a72 -C force-frame-pointers

# Export for build.rs.
export LD_SCRIPT_PATH



##--------------------------------------------------------------------------------------------------
## Targets and Prerequisites
##--------------------------------------------------------------------------------------------------
KERNEL_MANIFEST      = kernel/Cargo.toml
KERNEL_LINKER_SCRIPT = kernel.ld
LAST_BUILD_CONFIG    = target/$(BSP)_$(DEBUG_PRINTS).build_config

KERNEL_ELF_RAW      = target/$(TARGET)/$(MODE)/kernel
# This parses cargo's dep-info file.
# https://doc.rust-lang.org/cargo/guide/build-cache.html#dep-info-files
KERNEL_ELF_RAW_DEPS = $(filter-out %: ,$(file < $(KERNEL_ELF_RAW).d)) $(KERNEL_MANIFEST) $(LAST_BUILD_CONFIG)

##------------------------------------------------------------------------------
## Translation tables
##------------------------------------------------------------------------------
TT_TOOL_PATH = tools/translation_table_tool

KERNEL_ELF_TTABLES      = target/$(TARGET)/${MODE}/kernel+ttables
KERNEL_ELF_TTABLES_DEPS = $(KERNEL_ELF_RAW) $(wildcard $(TT_TOOL_PATH)/*)

##------------------------------------------------------------------------------
## Kernel symbols
##------------------------------------------------------------------------------
export KERNEL_SYMBOLS_TOOL_PATH = tools/kernel_symbols_tool

KERNEL_ELF_TTABLES_SYMS = target/$(TARGET)/$(MODE)/kernel+ttables+symbols

# Unlike with KERNEL_ELF_RAW, we are not relying on dep-info here. One of the reasons being that the
# name of the generated symbols file varies between runs, which can cause confusion.
KERNEL_ELF_TTABLES_SYMS_DEPS = $(KERNEL_ELF_TTABLES) \
    $(wildcard kernel_symbols/*)                     \
    $(wildcard $(KERNEL_SYMBOLS_TOOL_PATH)/*)

export TARGET
export KERNEL_SYMBOLS_INPUT_ELF  = $(KERNEL_ELF_TTABLES)
export KERNEL_SYMBOLS_OUTPUT_ELF = $(KERNEL_ELF_TTABLES_SYMS)

KERNEL_ELF = $(KERNEL_ELF_TTABLES_SYMS)



##--------------------------------------------------------------------------------------------------
## Command building blocks
##--------------------------------------------------------------------------------------------------
RUSTFLAGS = $(RUSTC_MISC_ARGS)                   \
    -C link-arg=--library-path=$(LD_SCRIPT_PATH) \
    -C link-arg=--script=$(KERNEL_LINKER_SCRIPT)

RUSTFLAGS_PEDANTIC = $(RUSTFLAGS) \
    -D warnings                   \
#    -D missing_docs

FEATURES     += --features bsp_$(BSP)


ifeq ($(MODE), release) # Release mode
COMPILER_ARGS = --target=$(TARGET) \
    $(FEATURES)                    \
    --release
else
COMPILER_ARGS = --target=$(TARGET) \
    $(FEATURES)
endif

# build-std can be skipped for helper commands that do not rely on correct stack frames and other
# custom compiler options. This results in a huge speedup.
RUSTC_CMD   = cargo rustc $(COMPILER_ARGS) -Z build-std=core,alloc --manifest-path $(KERNEL_MANIFEST)
DOC_CMD     = cargo doc $(COMPILER_ARGS)
CLIPPY_CMD  = cargo clippy $(COMPILER_ARGS)
OBJCOPY_CMD = rust-objcopy \
    --strip-all            \
    -O binary

EXEC_TT_TOOL       = ruby $(TT_TOOL_PATH)/main.rb


##------------------------------------------------------------------------------
## Dockerization
##------------------------------------------------------------------------------
DOCKER_CMD            = docker run -t --rm -v $(shell pwd):/work/tutorial -w /work/tutorial
DOCKER_CMD_INTERACT   = $(DOCKER_CMD) -i
DOCKER_ARG_DIR_COMMON = -v $(shell pwd)/../common:/work/common
DOCKER_ARG_DIR_JTAG   = -v $(shell pwd)/../X1_JTAG_boot:/work/X1_JTAG_boot
DOCKER_ARG_DEV        = --privileged -v /dev:/dev
DOCKER_ARG_NET        = --network host

# DOCKER_IMAGE defined in include file (see top of this file).
DOCKER_TOOLS = $(DOCKER_CMD) $(DOCKER_IMAGE)
DOCKER_TEST  = $(DOCKER_CMD) $(DOCKER_ARG_DIR_COMMON) $(DOCKER_IMAGE)
DOCKER_GDB   = $(DOCKER_CMD_INTERACT) $(DOCKER_ARG_NET) $(DOCKER_IMAGE)

# Dockerize commands, which require USB device passthrough, only on Linux.
ifeq ($(shell uname -s),Linux)
    DOCKER_CMD_DEV = $(DOCKER_CMD_INTERACT) $(DOCKER_ARG_DEV)

    DOCKER_CHAINBOOT = $(DOCKER_CMD_DEV) $(DOCKER_ARG_DIR_COMMON) $(DOCKER_IMAGE)
    DOCKER_JTAGBOOT  = $(DOCKER_CMD_DEV) $(DOCKER_ARG_DIR_COMMON) $(DOCKER_ARG_DIR_JTAG) $(DOCKER_IMAGE)
    DOCKER_OPENOCD   = $(DOCKER_CMD_DEV) $(DOCKER_ARG_NET) $(DOCKER_IMAGE)
else
    DOCKER_OPENOCD   = echo "Not yet supported on non-Linux systems."; \#
endif



##--------------------------------------------------------------------------------------------------
## Targets
##--------------------------------------------------------------------------------------------------
.PHONY: all doc qemu chainboot clippy clean readelf objdump nm check

all: $(KERNEL_BIN)

##------------------------------------------------------------------------------
## Save the configuration as a file, so make understands if it changed.
##------------------------------------------------------------------------------
$(LAST_BUILD_CONFIG):
	@rm -f target/*.build_config
	@mkdir -p target
	@touch $(LAST_BUILD_CONFIG)

##------------------------------------------------------------------------------
## Compile the kernel ELF
##------------------------------------------------------------------------------
$(KERNEL_ELF_RAW): $(KERNEL_ELF_RAW_DEPS)
	$(call color_header, "Compiling kernel ELF - $(BSP)")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(RUSTC_CMD)

##------------------------------------------------------------------------------
## Precompute the kernel translation tables and patch them into the kernel ELF
##------------------------------------------------------------------------------
$(KERNEL_ELF_TTABLES): $(KERNEL_ELF_TTABLES_DEPS)
	$(call color_header, "Precomputing kernel translation tables and patching kernel ELF")
	@cp $(KERNEL_ELF_RAW) $(KERNEL_ELF_TTABLES)
	@$(EXEC_TT_TOOL) $(BSP) $(KERNEL_ELF_TTABLES)

##------------------------------------------------------------------------------
## Generate kernel symbols and patch them into the kernel ELF
##------------------------------------------------------------------------------
$(KERNEL_ELF_TTABLES_SYMS): $(KERNEL_ELF_TTABLES_SYMS_DEPS)
	$(call color_header, "Generating kernel symbols and patching kernel ELF")
	@$(MAKE) MODE=$(MODE) --no-print-directory -f kernel_symbols.mk

##------------------------------------------------------------------------------
## Generate the stripped kernel binary
##------------------------------------------------------------------------------
$(KERNEL_BIN): $(KERNEL_ELF_TTABLES_SYMS)
	$(call color_header, "Generating stripped binary")
	@$(OBJCOPY_CMD) $(KERNEL_ELF_TTABLES_SYMS) $(KERNEL_BIN)
	$(call color_progress_prefix, "Name")
	@echo $(KERNEL_BIN)
	$(call color_progress_prefix, "Size")
	$(call disk_usage_KiB, $(KERNEL_BIN))

##------------------------------------------------------------------------------
## Generate the documentation
##------------------------------------------------------------------------------
doc: clean
	$(call color_header, "Generating docs")
	@$(DOC_CMD) --document-private-items --open

##------------------------------------------------------------------------------
## Run clippy
##------------------------------------------------------------------------------
clippy:
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(CLIPPY_CMD)
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(CLIPPY_CMD) --features test_build --tests \
                --manifest-path $(KERNEL_MANIFEST)

##------------------------------------------------------------------------------
## Clean
##------------------------------------------------------------------------------
clean:
	rm -rf target $(KERNEL_BIN)

##------------------------------------------------------------------------------
## Run readelf
##------------------------------------------------------------------------------
readelf: $(KERNEL_ELF)
	$(call color_header, "Launching readelf")
	@$(READELF_BINARY) --headers $(KERNEL_ELF)

##------------------------------------------------------------------------------
## Run objdump
##------------------------------------------------------------------------------
objdump: $(KERNEL_ELF)
	$(call color_header, "Launching objdump")
	@$(OBJDUMP_BINARY) --disassemble --demangle \
                --section .text   \
                --section .rodata \
                $(KERNEL_ELF) | rustfilt

##------------------------------------------------------------------------------
## Run nm
##------------------------------------------------------------------------------
nm: $(KERNEL_ELF)
	$(call color_header, "Launching nm")
	@$(NM_BINARY) --demangle --print-size $(KERNEL_ELF) | sort | rustfilt



##--------------------------------------------------------------------------------------------------
## Debugging targets
##--------------------------------------------------------------------------------------------------
.PHONY: jtagboot openocd gdb gdb-opt0

##------------------------------------------------------------------------------
## Push the JTAG boot image to the real HW target
##------------------------------------------------------------------------------
jtagboot:
	@$(DOCKER_JTAGBOOT) $(EXEC_MINIPUSH) $(DEV_SERIAL) $(JTAG_BOOT_IMAGE)

##------------------------------------------------------------------------------
## Start OpenOCD session
##------------------------------------------------------------------------------
openocd:
	$(call color_header, "Launching OpenOCD")
	@$(DOCKER_OPENOCD) openocd $(OPENOCD_ARG)

##------------------------------------------------------------------------------
## Start GDB session
##------------------------------------------------------------------------------
gdb-opt0: RUSTC_MISC_ARGS += -C opt-level=0
gdb gdb-opt0: $(KERNEL_ELF)
	$(call color_header, "Launching GDB")
	@gdb-multiarch -q $(KERNEL_ELF)
