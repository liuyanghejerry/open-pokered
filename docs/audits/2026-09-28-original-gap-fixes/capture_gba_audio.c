#include <mgba/flags.h>
#include <mgba/core/core.h>
#include <mgba-util/vfs.h>
#include <mgba/core/log.h>
static bool complete=false, failed=false;
static void report_log(struct mLogger *l, int cat, enum mLogLevel level, const char *fmt, va_list args) {
 if (strcmp(mLogCategoryName(cat), "GBA Debug") == 0 || (level & (mLOG_ERROR | mLOG_FATAL))) { char line[1024]; vsnprintf(line,sizeof(line),fmt,args); puts(line); fflush(stdout); if(strstr(line,"memory: ALL PASS")) complete=true; if(strstr(line,"panicked") || strstr(line,"memory allocation")) failed=true; }
}
static struct mLogger logger={.log=report_log};
#include <mgba/core/blip_buf.h>
int main(int argc, char **argv) {
    if (argc < 4) { fprintf(stderr, "usage: %s ROM AUDIO.raw SCREEN.rgba [frames]\n", argv[0]); return 1; }
    mLogSetDefaultLogger(&logger);
    struct mCore *c = mCoreFind(argv[1]);
    if (!c || !c->init(c)) return 1;
    mCoreInitConfig(c, NULL);
    color_t *pixels = calloc(240*160, sizeof(color_t));
    c->setVideoBuffer(c,pixels,240);
    c->setAudioBufferSize(c, 2048);
    if (!mCoreLoadFile(c, argv[1])) return 2;
    c->reset(c);
    blip_t *left=c->getAudioChannel(c,0), *right=c->getAudioChannel(c,1);
    blip_set_rates(left,c->frequency(c),44100);
    blip_set_rates(right,c->frequency(c),44100);
    FILE *out=fopen(argv[2],"wb");
    long nonzero=0, samples=0; int active=0;
    for(int frame=0;frame<(argc > 4 ? atoi(argv[4]) : 1800) && !complete && !failed;frame++) {
        c->setKeys(c, frame >= 600 && frame < 900 && frame % 8 < 2 ? 1 : 0);
        if(frame==605) printf("keys=%04x\n", c->busRead16(c,0x04000130));
        c->runFrame(c);
        int16_t pcm[4096]={0};
        int n=blip_read_samples(left, pcm, 2048, 1);
        blip_read_samples(right, pcm+1, 2048, 1);
        fwrite(pcm, sizeof(int16_t), n*2, out);
        samples+=n*2;
        for(int i=0;i<n*2;i++) nonzero+=pcm[i]!=0;
        unsigned status=c->busRead16(c,0x04000084);
        active |= status & 15;
        if(frame%300==0) printf("frame=%d status=%04x volume=%04x\n",frame,status,c->busRead16(c,0x04000080));
    }
    fclose(out);
    printf("samples=%ld nonzero=%ld active_channels=%x\n",samples,nonzero,active);
    FILE *pic=fopen(argv[3],"wb");fwrite(pixels,sizeof(color_t),240*160,pic);fclose(pic);
    c->deinit(c);free(pixels);
    return failed ? 4 : nonzero ? 0 : 3;
}
