#include <mgba/flags.h>
#include <mgba/core/core.h>
#include <mgba/core/log.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
struct Audit { struct mCore* core; color_t pixels[240*160]; };
static FILE* logfile;
static void log_message(struct mLogger* l, int category, enum mLogLevel level, const char* fmt, va_list args) {
    (void)l;
    if (!logfile) return;
    if (!strcmp(mLogCategoryName(category), "GBA DMA")) return;
    fprintf(logfile,"[%s:%d] ", mLogCategoryName(category), level);
    vfprintf(logfile,fmt,args); fputc('\n',logfile); fflush(logfile);
}
static struct mLogger logger={log_message,NULL};
void* audit_open(const char* rom,const char* save,const char* logpath) {
    logfile=fopen(logpath,"w"); mLogSetDefaultLogger(&logger);
    struct Audit* a=calloc(1,sizeof(*a)); if(!a) return NULL;
    a->core=mCoreFind(rom);
    if(!a->core || !a->core->init(a->core)) return NULL;
    mCoreInitConfig(a->core,"open-pokered-audit");
    a->core->opts.useBios=false; a->core->opts.skipBios=true;
    a->core->opts.frameskip=0;
    if(!mCoreLoadFile(a->core,rom) || !mCoreLoadSaveFile(a->core,save,true)) return NULL;
    unsigned width=0,height=0; a->core->desiredVideoDimensions(a->core,&width,&height);
    if(width!=240 || height!=160 || sizeof(color_t)!=4) return NULL;
    a->core->setVideoBuffer(a->core,a->pixels,240);
    a->core->reset(a->core); return a;
}
void audit_step(void* ctx,unsigned frames,unsigned keys) {
    struct Audit* a=ctx; a->core->setKeys(a->core,keys);
    while(frames--) a->core->runFrame(a->core);
}
const void* audit_pixels(void* ctx) { return ((struct Audit*)ctx)->pixels; }
unsigned audit_frame(void* ctx) { struct Audit* a=ctx; return a->core->frameCounter(a->core); }
int audit_frequency(void* ctx) { struct Audit* a=ctx; return a->core->frequency(a->core); }
int audit_frame_cycles(void* ctx) { struct Audit* a=ctx; return a->core->frameCycles(a->core); }
void audit_close(void* ctx) {
    struct Audit* a=ctx; mCoreConfigDeinit(&a->core->config); a->core->deinit(a->core); free(a);
    mLogSetDefaultLogger(NULL); if(logfile) fclose(logfile); logfile=NULL;
}

/* Observe the cartridge save bytes without writing game memory or advancing a frame. */
size_t audit_export_sram(void* ctx, const char* path) {
    struct Audit* a=ctx; void* data=NULL;
    size_t size=a->core->savedataClone(a->core,&data);
    if(!size || !data) return 0;
    FILE* file=fopen(path,"wb");
    if(!file) { free(data); return 0; }
    size_t written=fwrite(data,1,size,file);
    fclose(file);free(data);return written;
}
