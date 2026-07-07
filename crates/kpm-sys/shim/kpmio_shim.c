/*
 * KPMIO trampoline shim.
 *
 * libkpm's `struct KPMIO` callbacks (`KPMLog`, `KPMLogProgress`, `KPMGetInput`)
 * are variadic C functions, which cannot be defined in stable Rust. This shim
 * defines the variadic trampolines in C: each one formats the message with
 * vsnprintf and forwards the finished string to a non-variadic handler
 * registered from Rust.
 *
 * KPMIO carries no user-data pointer, so the handlers are process-global.
 * Kinstaller drives a single KPM session from a single worker thread, so this
 * is safe in practice; the setter must be called before any KPM operation.
 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>

#define SHIM_BUF_SIZE 8192

typedef void (*kinstaller_log_handler)(int verbosity, const char *message);
typedef void (*kinstaller_stream_handler)(char c);
typedef void (*kinstaller_progress_handler)(unsigned int progress, const char *message);
typedef bool (*kinstaller_input_handler)(const char *prompt);

static kinstaller_log_handler g_log = 0;
static kinstaller_stream_handler g_stream = 0;
static kinstaller_progress_handler g_progress = 0;
static kinstaller_input_handler g_input = 0;

void kinstaller_kpmio_set_handlers(kinstaller_log_handler log,
                                   kinstaller_stream_handler stream,
                                   kinstaller_progress_handler progress,
                                   kinstaller_input_handler input)
{
    g_log = log;
    g_stream = stream;
    g_progress = progress;
    g_input = input;
}

/* Matches: typedef void KPMLog(enum KPMVerbosity, const char* format, ...) */
static void shim_log(int verbosity, const char *format, ...)
{
    char buf[SHIM_BUF_SIZE];
    va_list ap;
    va_start(ap, format);
    vsnprintf(buf, sizeof buf, format, ap);
    va_end(ap);
    if (g_log)
        g_log(verbosity, buf);
}

/* Matches: typedef void KPMStream(char c) */
static void shim_stream(char c)
{
    if (g_stream)
        g_stream(c);
}

/* Matches: typedef void KPMLogProgress(unsigned int progress, const char* format, ...) */
static void shim_progress(unsigned int progress, const char *format, ...)
{
    char buf[SHIM_BUF_SIZE];
    va_list ap;
    va_start(ap, format);
    vsnprintf(buf, sizeof buf, format, ap);
    va_end(ap);
    if (g_progress)
        g_progress(progress, buf);
}

/* Matches: typedef bool KPMGetInput(const char* format, ...) */
static bool shim_input(const char *format, ...)
{
    char buf[SHIM_BUF_SIZE];
    va_list ap;
    va_start(ap, format);
    vsnprintf(buf, sizeof buf, format, ap);
    va_end(ap);
    /* No handler registered: decline, mirroring a cautious default. */
    return g_input ? g_input(buf) : false;
}

/* Accessors returning the trampolines for Rust to place into struct KPMIO. */
void *kinstaller_kpmio_log_fn(void) { return (void *)shim_log; }
void *kinstaller_kpmio_stream_fn(void) { return (void *)shim_stream; }
void *kinstaller_kpmio_progress_fn(void) { return (void *)shim_progress; }
void *kinstaller_kpmio_input_fn(void) { return (void *)shim_input; }
