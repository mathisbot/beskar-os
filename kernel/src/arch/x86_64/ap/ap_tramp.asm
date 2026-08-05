; WARNING:
;
; This file is not automatically built using `cargo build`.
; You should run `nasm -f bin ap_tramp.asm -o ap_tramp` each time you change this file.
;
; Also, try to keep its compiled size under 4096 bytes for convenience.

ORG 0x8000
[BITS 16]
section .text

__startup:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax

    mov edi, [trampoline.page_table]
    mov cr3, edi

    ; Minimal CR4 bit setup, will be overwritten at startup
    ; with BSP's CR4 value
    mov eax, cr4
    or eax, 1 << 5 ; Enable PAE
    mov cr4, eax

    lgdt [gdtr]

    ; Minimal EFER bit setup, will be overwritten at startup
    ; with BSP's EFER value
    mov ecx, 0xC0000080 ; Read from EFER MSR
    rdmsr
    or eax,  1 << 8 | 1 << 11 ; Set Long-Mode-Enable and NXE
    wrmsr

    ; Minimal CR0 bit setup, will be overwritten at startup
    ; with BSP's CR0 value
    mov ebx, cr0
    or ebx, 1 << 31  | 1 ; Set paging and write protection
    mov cr0, ebx

    ; Far jump to long mode
    jmp gdt.kernel_code:long_mode_ap

[BITS 64]
DEFAULT ABS

long_mode_ap:
    xor rsp, rsp

    ; Load stack end
    mov rdi, [trampoline.stack_end_ptr]

    jmp stack_lookup

; Spin until stack is owned
wait_for_stack:
    pause
stack_lookup:
    ; Try to atomically acquire the stack
    xchg [rdi], rsp
    test rsp, rsp
    je wait_for_stack

    ; Past this point, the stack is ours

    jmp [trampoline.ap_entry]

section .data

; Minimal GDT (overwritten later on)
gdt:
.null equ $ - gdt
    dq 0
.kernel_code equ $ - gdt
    ; 53: Long mode
    ; 47: Present
    ; 44: Code/data segment
    ; 43: Executable
    ; 41: Readable
    dq 0x00209A0000000000
.kernel_data equ $ - gdt
    ; 47: Present
    ; 44: Code/data segment
    ; 41: Writable
    dq 0x0000920000000000
.end equ $ - gdt
ALIGN 4, db 0
gdtr:
    dw gdt.end - 1
    dq gdt

ALIGN 8, nop
; These are placeholders and will be overwritten at runtime by the BSP
trampoline:
    .page_table: dq 0xFFFFFFFFFFFFFFFF
    .stack_end_ptr: dq 0xFFFFFFFFFFFFFFFF
    .ap_entry: dq 0xFFFFFFFFFFFFFFFF
