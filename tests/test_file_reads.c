/* Re-protect guest pages after preparation, just before the host read (#345).
 * Windows: gcc -std=c11 -O2 -Wall -Wextra -Werror -pthread -Isrc
 * tests/test_file_reads.c src/compat_win.c -lbcrypt -o out/file-reads-test.exe */
#undef NDEBUG
#define _GNU_SOURCE
#include "../src/runtime.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <pthread.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <signal.h>
#include <sys/mman.h>
#endif

static unsigned char *destination;
static size_t page_size, region_size;
static int arm_protection, fail_allocation, read_error, error_after;
static size_t short_read;
static volatile int faults;

static void protect_page(int writable) {
#ifdef _WIN32
    DWORD old;
    assert(VirtualProtect(destination,page_size,writable ? PAGE_READWRITE : PAGE_READONLY,&old));
#else
    assert(!mprotect(destination,page_size,writable ? PROT_READ|PROT_WRITE : PROT_READ));
#endif
}
#ifdef _WIN32
static LONG CALLBACK fault_handler(EXCEPTION_POINTERS *exception) {
    EXCEPTION_RECORD *r=exception->ExceptionRecord;
    if (r->ExceptionCode!=EXCEPTION_ACCESS_VIOLATION || r->ExceptionInformation[0]!=1 ||
        r->ExceptionInformation[1]<(uintptr_t)destination ||
        r->ExceptionInformation[1]>=(uintptr_t)destination+page_size) return EXCEPTION_CONTINUE_SEARCH;
    protect_page(1); ++faults;
    return EXCEPTION_CONTINUE_EXECUTION;
}
#else
static void fault_handler(int signal,siginfo_t *info,void *context) {
    (void)signal; (void)context;
    if ((uintptr_t)info->si_addr<(uintptr_t)destination ||
        (uintptr_t)info->si_addr>=(uintptr_t)destination+page_size) _exit(2);
    protect_page(1); ++faults;
}
#endif
static void before_read(void) {
    if (arm_protection) { arm_protection=0; protect_page(0); }
    if (error_after && !--error_after) read_error=EIO;
}
static ssize_t tracked_read(int fd,void *buffer,size_t size) {
    before_read();
    if (read_error) { errno=read_error; read_error=0; return -1; }
    if (short_read && size>short_read) { size=short_read; short_read=0; }
#ifdef _WIN32
    return _read(fd,buffer,(unsigned)size);
#else
    return read(fd,buffer,size);
#endif
}
static ssize_t tracked_pread(int fd,void *buffer,size_t size,int64_t offset) {
    before_read();
    if (read_error) { errno=read_error; read_error=0; return -1; }
    if (short_read && size>short_read) { size=short_read; short_read=0; }
    return pread(fd,buffer,size,offset);
}
static void *tracked_malloc(size_t size) {
    if (fail_allocation) { fail_allocation=0; return NULL; }
    return malloc(size);
}
#ifdef _WIN32
#undef pread
#endif
#define read tracked_read
#define pread tracked_pread
#define malloc tracked_malloc
#include "../src/runtime_file.c"
#undef malloc
#undef pread
#undef read

static int32_t guest_errno;
int32_t *runtime_errno(void) { return &guest_errno; }
int32_t runtime_guest_errno(int e) { return e; }
uintptr_t runtime_lookup(const RuntimeExport *table,size_t n,const char *name) {
    for (size_t i=0;i<n;++i) if (!strcmp(table[i].name,name)) return (uintptr_t)table[i].function;
    return 0;
}

static void trace_contract(const char *path,int enabled) {
#ifdef _WIN32
    assert(!_putenv_s("SPARTAN_FILE_TRACE",enabled ? "1" : "0"));
#else
    assert(!setenv("SPARTAN_FILE_TRACE",enabled ? "1" : "0",1));
#endif
    FILE *capture=tmpfile(); assert(capture);
    fflush(stdout);
    int saved=dup(fileno(stdout)); assert(saved>=0);
    assert(dup2(fileno(capture),fileno(stdout))>=0);
    assert(!runtime_file_mount("/app0/test.wad",path));
    int fd=(int)runtime_file_open("/app0/test.wad",0,0); assert(fd>=3);
    char data[32];
    assert(runtime_file_pread(fd,data,sizeof(data),7)==sizeof(data));
    assert(runtime_file_read(fd,data,sizeof(data))==sizeof(data));
    read_error=EIO;
    assert(runtime_file_pread(fd,data,sizeof(data),0)==-EIO);
    assert(runtime_file_pread(fd,data,sizeof(data),-1)==-EINVAL);
    assert(!runtime_file_close(fd));
    assert(runtime_file_open("/app0/missing.WAD",0,0)==-ENOENT);
    assert(!runtime_file_mount("/app0/ignored.bin",path));
    fd=(int)runtime_file_open("/app0/ignored.bin",0,0); assert(fd>=3);
    assert(runtime_file_read(fd,data,sizeof(data))==sizeof(data));
    assert(!runtime_file_close(fd));
    fflush(stdout);
    assert(dup2(saved,fileno(stdout))>=0 && !close(saved));
    assert(!fseek(capture,0,SEEK_SET));
    char log[16384]; size_t length=fread(log,1,sizeof(log)-1,capture); log[length]=0;
    assert(feof(capture) && !fclose(capture));
    if (!enabled) assert(!strstr(log,"File trace:") && !strstr(log,"WAD file tracing enabled"));
    else {
        assert(strstr(log,"WAD file tracing enabled (SPARTAN_FILE_TRACE=1)"));
        assert(strstr(log,"File trace:"));
        assert(strstr(log,"offset=7 requested=32"));
        assert(strstr(log,"offset=0 requested=32"));
        char expected_size[64]; snprintf(expected_size,sizeof(expected_size),"file_size=%zu",region_size);
        assert(strstr(log,expected_size));
        assert(strstr(log,"result=-5 errno=5"));
        assert(strstr(log,"result=-22 errno=22"));
        assert(strstr(log,"result=-2 errno=2"));
        assert(strstr(log,"path=/app0/missing.WAD"));
        assert(!strstr(log,"ignored.bin") && !strstr(log,path));
        unsigned begin[8]={0},end[8]={0};
        for (char *line=log;(line=strstr(line,"File trace:"));++line) {
            size_t id; char phase[16];
            assert(sscanf(line,"File trace: id=%zu phase=%15s",&id,phase)==2);
            assert(id>0 && id<8);
            if (!strcmp(phase,"begin")) ++begin[id];
            else { assert(!strcmp(phase,"end")); assert(begin[id]==1); ++end[id]; }
        }
        for (size_t id=1;id<8;++id) assert(begin[id]==1 && end[id]==1);
    }
    runtime_file_unmount("/app0/test.wad");
    runtime_file_unmount("/app0/ignored.bin");
    puts(enabled ? "WAD trace: paired operations, offsets, errors and filtering PASS" : "WAD trace disabled: no diagnostic output PASS");
}

int main(int argc,char **argv) {
    (void)tracked_malloc;
    char path[512];
#ifdef _WIN32
    SYSTEM_INFO system; GetSystemInfo(&system); page_size=system.dwPageSize;
    char temp[MAX_PATH]; assert(GetTempPathA(sizeof(temp),temp));
    assert(GetTempFileNameA(temp,"spr",0,path));
    void *handler=AddVectoredExceptionHandler(1,fault_handler); assert(handler);
#else
    page_size=(size_t)sysconf(_SC_PAGESIZE);
    strcpy(path,"/tmp/spartan-file-reads-XXXXXX");
    int temp=mkstemp(path); assert(temp>=0 && !close(temp));
    struct sigaction sa={0},old;
    sa.sa_sigaction=fault_handler; sa.sa_flags=SA_SIGINFO;
    assert(!sigaction(SIGSEGV,&sa,&old));
#endif
    region_size=(2u<<20)+page_size;
    unsigned char *data=malloc(region_size); assert(data);
    for (size_t i=0;i<region_size;++i) data[i]=(unsigned char)(i*31+i/251);
    FILE *file=fopen(path,"wb"); assert(file);
    assert(fwrite(data,1,region_size,file)==region_size && !fclose(file));
    if (argc>1) {
        trace_contract(path,!strcmp(argv[1],"--trace-on"));
        assert(!remove(path)); free(data);
#ifdef _WIN32
        assert(RemoveVectoredExceptionHandler(handler));
#else
        assert(!sigaction(SIGSEGV,&old,NULL));
#endif
        return 0;
    }
#ifdef _WIN32
    destination=VirtualAlloc(NULL,region_size,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    assert(destination);
#else
    destination=mmap(NULL,region_size,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);
    assert(destination!=MAP_FAILED);
#endif
    assert(!runtime_file_mount("/app0/test.wad",path));
    int fd=(int)runtime_file_open("/app0/test.wad",0,0); assert(fd>=3);
    assert(runtime_file_lseek(fd,17,SEEK_SET)==17);
    arm_protection=1;
    int64_t result=runtime_file_pread(fd,destination,region_size,0);
    if (result!=(int64_t)region_size) fprintf(stderr,"protected pread: expected %zu, got %lld\n",region_size,(long long)result);
    assert(result==(int64_t)region_size && !memcmp(destination,data,region_size) && faults>0);
    assert(runtime_file_lseek(fd,0,SEEK_CUR)==17);
    puts("Protected pread through runtime API PASS");

    int previous=faults; arm_protection=1;
    assert(runtime_file_read(fd,destination,region_size-17)==(int64_t)region_size-17);
    assert(!memcmp(destination,data+17,region_size-17) && faults>previous);
    assert(runtime_file_lseek(fd,0,SEEK_CUR)==(int64_t)region_size);
    puts("Protected sequential read through runtime API PASS");

    assert(runtime_file_pread(fd,destination+3,12345,777)==12345);
    assert(!memcmp(destination+3,data+777,12345));
    assert(runtime_file_pread(fd,destination,4096,(int64_t)region_size-100)==100);
    assert(!memcmp(destination,data+region_size-100,100));
    destination[0]=0xa5;
    assert(runtime_file_pread(fd,destination,10,(int64_t)region_size)==0 && destination[0]==0xa5);
    assert(runtime_file_pread(fd,NULL,0,0)==0 && runtime_file_read(fd,NULL,0)==0);
    assert(runtime_file_pread(fd,destination,10,-1)==-EINVAL);
    assert(runtime_file_pread(-1,destination,10,0)==-EBADF);
    assert(runtime_file_read(-1,destination,10)==-EBADF);
    assert(runtime_file_pread(fd,NULL,10,0)==-EFAULT);
    assert(runtime_file_pread(fd,destination,UINT64_MAX,0)==-EINVAL);
    read_error=EIO;
    assert(runtime_file_pread(fd,destination,10,0)==-EIO);
    short_read=7;
    assert(runtime_file_pread(fd,destination,100,0)==100 && !memcmp(destination,data,100));
    short_read=7; fail_allocation=1;
    assert(runtime_file_pread(fd,destination,100,0)==7 && !memcmp(destination,data,7));
    assert(!fail_allocation);
    fail_allocation=1;
    arm_protection=1;
    assert(runtime_file_pread(fd,destination,100,0)==-ENOMEM);
    assert(!fail_allocation);
    assert(runtime_file_lseek(fd,0,SEEK_CUR)==(int64_t)region_size);
    assert(runtime_file_lseek(fd,17,SEEK_SET)==17);
    fail_allocation=1;
    assert(runtime_file_read(fd,destination,100)==-ENOMEM);
    assert(!fail_allocation && runtime_file_lseek(fd,0,SEEK_CUR)==17);
    read_error=EIO;
    assert(runtime_file_read(fd,destination,100)==-EIO);
    assert(runtime_file_lseek(fd,0,SEEK_CUR)==17);
    error_after=3; arm_protection=1;
    result=runtime_file_pread(fd,destination,region_size,0);
    assert(result>0 && result<(int64_t)region_size);
    assert(!memcmp(destination,data,(size_t)result) && !read_error && !error_after);
    assert(runtime_file_lseek(fd,0,SEEK_CUR)==17);
    error_after=2;
    result=runtime_file_read(fd,destination,region_size-17);
    assert(result>0 && result<(int64_t)region_size-17);
    assert(!memcmp(destination,data+17,(size_t)result));
    assert(runtime_file_lseek(fd,0,SEEK_CUR)==17+result);
    assert(runtime_file_lseek(fd,17,SEEK_SET)==17);
    short_read=7;
    assert(runtime_file_read(fd,destination,100)==7 && !memcmp(destination,data+17,7));
    assert(runtime_file_lseek(fd,0,SEEK_CUR)==24);
    typedef int64_t (ABI *Pread)(int,void *,uint64_t,int64_t);
    Pread posix_api=(Pread)runtime_file_resolve("pread");
    Pread kernel_api=(Pread)runtime_file_resolve("sceKernelPread");
    assert(posix_api && kernel_api);
    read_error=EIO;
    assert(posix_api(fd,destination,10,0)==-1 && guest_errno==EIO);
    read_error=EIO;
    assert(kernel_api(fd,destination,10,0)==(int32_t)UINT32_C(0x80020005));
    puts("Offsets, EOF, short reads, errors and allocation failure PASS");

    assert(!runtime_file_close(fd));
    runtime_file_unmount("/app0/test.wad");
    assert(!remove(path)); free(data);
#ifdef _WIN32
    assert(VirtualFree(destination,0,MEM_RELEASE));
    assert(RemoveVectoredExceptionHandler(handler));
#else
    assert(!munmap(destination,region_size));
    assert(!sigaction(SIGSEGV,&old,NULL));
#endif
    return 0;
}
