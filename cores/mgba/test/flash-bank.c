#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <mgba/internal/gba/gba.h>
#include <mgba/internal/gba/savedata.h>
#include <mgba-util/vfs.h>

#define LOAD(name) __typeof__(&name) call_##name = dlsym(core, #name); \
    if (!call_##name) { fprintf(stderr, "missing %s\n", #name); return 2; }

static void check(struct GBASavedata *save, const char *when) {
    if (save->currentBank - save->data != 0x10000) {
        fprintf(stderr, "%s: selected flash bank is %td, expected 65536\n",
                when, save->currentBank - save->data);
        exit(1);
    }
}

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    void *core = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (!core) { fprintf(stderr, "%s\n", dlerror()); return 2; }
    LOAD(VFileMemChunk);
    LOAD(GBASavedataInit);
    LOAD(GBASavedataForceType);
    LOAD(GBASavedataWriteFlash);
    LOAD(GBASavedataReadFlash);
    LOAD(GBASavedataMask);
    LOAD(GBASavedataUnmask);
    LOAD(GBASavedataDeinit);

    struct GBA board = {0};
    struct GBASavedata save = {0};
    struct VFile *real = call_VFileMemChunk(NULL, 0x20000);
    if (!real) return 2;
    save.p = &board;
    call_GBASavedataInit(&save, real);
    call_GBASavedataForceType(&save, GBA_SAVEDATA_FLASH1M);
    memset(save.data, 0x11, 0x10000);
    memset(save.data + 0x10000, 0x22, 0x10000);
    call_GBASavedataWriteFlash(&save, 0x5555, 0xAA);
    call_GBASavedataWriteFlash(&save, 0x2AAA, 0x55);
    call_GBASavedataWriteFlash(&save, 0x5555, 0xB0);
    call_GBASavedataWriteFlash(&save, 0, 1);
    check(&save, "before mask");
    for (int i = 0; i < 8; ++i) {
        struct VFile *mask = call_VFileMemChunk(save.data, 0x20000);
        if (!mask) return 2;
        call_GBASavedataMask(&save, mask, false);
        check(&save, "after mask");
        if (call_GBASavedataReadFlash(&save, 0) != 0x22) return 1;
        call_GBASavedataUnmask(&save);
        check(&save, "after unmask");
        if (call_GBASavedataReadFlash(&save, 0) != 0x22 || save.data[0] != 0x11) return 1;
    }
    call_GBASavedataDeinit(&save);
    real->close(real);
    dlclose(core);
    puts("flash bank and both save banks survive eight mask/unmask cycles");
    return 0;
}
