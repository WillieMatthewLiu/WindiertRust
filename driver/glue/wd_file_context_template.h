#ifndef WD_FILE_CONTEXT_TEMPLATE_H
#define WD_FILE_CONTEXT_TEMPLATE_H

#include <wdf.h>

#include "wd_kmdf_bridge.h"

#ifndef WD_RUNTIME_QUEUE_CAPACITY
#define WD_RUNTIME_QUEUE_CAPACITY 64u
#endif

/*
 * Per-file runtime state template.
 *
 * The recommended ownership model is one Rust runtime handle per file object.
 * That keeps open/recv/send queue state isolated across callers.
 */
typedef struct _WD_FILE_CONTEXT {
    wd_runtime_glue_api_handle* runtime;
    size_t queue_capacity;
} WD_FILE_CONTEXT, *PWD_FILE_CONTEXT;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(WD_FILE_CONTEXT, GetWdFileContext)

static inline VOID
WdInitializeFileContext(_Out_ PWD_FILE_CONTEXT file_ctx)
{
    file_ctx->runtime = NULL;
    file_ctx->queue_capacity = WD_RUNTIME_QUEUE_CAPACITY;
}

static inline VOID
WdResetFileContext(_Inout_ PWD_FILE_CONTEXT file_ctx)
{
    file_ctx->runtime = NULL;
}

#endif /* WD_FILE_CONTEXT_TEMPLATE_H */
