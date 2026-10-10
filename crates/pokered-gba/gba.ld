/* SPDX-License-Identifier: MPL-2.0 */
/* Based on agb 0.25.0 src/gba.ld; see https://github.com/agbrs/agb.
 * Keep startup copy symbols compatible with agb when upgrading. */
OUTPUT_FORMAT("elf32-littlearm", "elf32-bigarm", "elf32-littlearm")
OUTPUT_ARCH(arm)

ENTRY(__start)
EXTERN(__RUST_INTERRUPT_HANDLER)

EXTERN(__agbabi_memset)
EXTERN(__agbabi_memcpy)

/* The bios reserves the final 256 bytes of iwram for its exclusive use, so we
 * need to avoid writing there */
__bios_reserved_iwram = 256;

MEMORY {
    ewram (w!x) : ORIGIN = 0x02000000, LENGTH = 256K
    iwram (w!x) : ORIGIN = 0x03000000, LENGTH = 32K - __bios_reserved_iwram
    rom (rx)    : ORIGIN = 0x08000000, LENGTH = 32M
}

SECTIONS {
    . = ORIGIN(rom);

    .entrypoint : {
        *(.entrypoint.regular .entrypoint.common);
        . = ALIGN(4);
    } > rom

    .iwram : {
        /* Hot framebuffer loops must not fetch instructions from slow carts.
         * Match symbol names independently of Rust crate hashes / LTO suffixes.
         * This is platform placement only; renderer implementations stay shared. */
        *(.text.*dotzuki_renderer*indexed_framebuffer*blit_gb_tile*);
        *(.text.*dotzuki_renderer*indexed_framebuffer*set_pixel*);
        *(.text.*dotzuki_renderer*indexed_framebuffer*fill_rect*);
        *(.text.*dotzuki_renderer*indexed_framebuffer*remap_shades*);
        *(.text.*dotzuki_renderer*embedded_font*draw_char*);
        *(.text.*dotzuki_renderer*embedded_font*draw_box_tile*);
        *(.text.*pokered_audio*gba_psg*write_register*);
        *(.text.*pokered_app*render*oak*draw_oak_speech*);
        *(.text.*specialized_div_rem*u32_div_rem*);
        *(.text.*pokered_gba*Mode4Presenter*copy_full_page*);
        *(.text.*pokered_gba*dma3_scroll_indices*);
        *(.iwram .iwram.*);
        . = ALIGN(4);

        *(.text_iwram .text_iwram.*);
        . = ALIGN(4);
    } > iwram AT>rom
    __iwram_data_start = ADDR(.iwram);
    __iwram_rom_start = LOADADDR(.iwram);
    __iwram_rom_length_halfwords = (SIZEOF(.iwram) + 1) / 2;
    __iwram_end = __iwram_data_start + SIZEOF(.iwram);

    /* Main game stack is in EWRAM; leave at least 8 KiB for startup/IRQ. */
    ASSERT(SIZEOF(.iwram) <= 24K - __bios_reserved_iwram,
           "GBA hot code exceeds IWRAM budget (8 KiB stack reserve)")

    .text : {
        *(.text .text*);
        . = ALIGN(4);
    } > rom

    .rodata : {
        *(.rodata .rodata.*);
        . = ALIGN(4);
    } > rom

    .ewram : {
        *(.ewram .ewram.*);
        . = ALIGN(4);

        *(.data .data.*);
        . = ALIGN(4);
    } > ewram AT>rom
    __ewram_data_start = ADDR(.ewram);
    __ewram_rom_start = LOADADDR(.ewram);
    __ewram_rom_length_halfwords = (SIZEOF(.ewram) + 1) / 2;

    .bss (NOLOAD) : {
        . = ALIGN(4);
        __bss_start = .;
        *(.bss .bss.*);
        . = ALIGN(4);
        __bss_end = .;
        __ewram_data_end = .;
    } > ewram

    .shstrtab : {
       *(.shstrtab)
    }

    /* debugging sections */
    /* Stabs */
    .stab            0 : { *(.stab) }
    .stabstr         0 : { *(.stabstr) }
    .stab.excl       0 : { *(.stab.excl) }
    .stab.exclstr    0 : { *(.stab.exclstr) }
    .stab.index      0 : { *(.stab.index) }
    .stab.indexstr   0 : { *(.stab.indexstr) }
    .comment         0 : { *(.comment) }
    /* DWARF 1 */
    .debug           0 : { *(.debug) }
    .line            0 : { *(.line) }
    /* GNU DWARF 1 extensions */
    .debug_srcinfo   0 : { *(.debug_srcinfo) }
    .debug_sfnames   0 : { *(.debug_sfnames) }
    /* DWARF 1.1 and DWARF 2 */
    .debug_aranges   0 : { *(.debug_aranges) }
    .debug_pubnames  0 : { *(.debug_pubnames) }
    /* DWARF 2 */
    .debug_info      0 : { *(.debug_info) }
    .debug_abbrev    0 : { *(.debug_abbrev) }
    .debug_line      0 : { *(.debug_line) }
    .debug_frame     0 : { *(.debug_frame) }
    .debug_str       0 : { *(.debug_str) }
    .debug_loc       0 : { *(.debug_loc) }
    .debug_macinfo   0 : { *(.debug_macinfo) }
    /* SGI/MIPS DWARF 2 extensions */
    .debug_weaknames 0 : { *(.debug_weaknames) }
    .debug_funcnames 0 : { *(.debug_funcnames) }
    .debug_typenames 0 : { *(.debug_typenames) }
    .debug_varnames  0 : { *(.debug_varnames) }

    .debug_ranges   0 : { *(.debug_ranges) }

    /* Symbol tables for debuggers */
    .symtab : { *(.symtab) }
    .strtab : { *(.strtab) }

    /* discard anything not already mentioned */
    /DISCARD/ : { *(*) }
}
