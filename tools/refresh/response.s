.syntax unified
.arm
.section .text
.global _start
_start:
    mov r0, #0x04000000
    mov r1, #0x400
    orr r1, r1, #4
    strh r1, [r0]
    mov r1, #0x05000000
    mov r2, #0
    strh r2, [r1]
    mov r3, #0x06000000
    mov r5, #0x9600
0:
    strh r2, [r3], #2
    subs r5, r5, #2
    bne 0b
    mov r4, #0
1:
    ldrh r2, [r0, #6]
    cmp r2, #160
    bhs 1b
2:
    ldrh r2, [r0, #6]
    cmp r2, #160
    blo 2b
    cmp r4, #0
    moveq r2, #31
    movne r2, #0x3e0
    strh r2, [r1]
    ldr r3, =0x04000130
    ldrh r4, [r3]
    mvn r4, r4
    and r4, r4, #1
    b 1b
.ltorg
