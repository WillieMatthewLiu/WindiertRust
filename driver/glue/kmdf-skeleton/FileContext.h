#ifndef WD_KMDF_SKELETON_FILE_CONTEXT_H
#define WD_KMDF_SKELETON_FILE_CONTEXT_H

#include <ntddk.h>
#include <wdf.h>

#include "..\wd_kmdf_bridge.h"

#ifndef WD_RUNTIME_QUEUE_CAPACITY
#define WD_RUNTIME_QUEUE_CAPACITY 64u
#endif

typedef struct _WD_FILE_CONTEXT {
    wd_runtime_glue_api_handle* runtime;
    size_t queue_capacity;
} WD_FILE_CONTEXT, *PWD_FILE_CONTEXT;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(WD_FILE_CONTEXT, GetWdFileContext)

static __forceinline VOID
WdInitializeFileContext(_Out_ PWD_FILE_CONTEXT file_ctx)
{
    file_ctx->runtime = NULL;
    file_ctx->queue_capacity = WD_RUNTIME_QUEUE_CAPACITY;
}

static __forceinline VOID
WdResetFileContext(_Inout_ PWD_FILE_CONTEXT file_ctx)
{
    file_ctx->runtime = NULL;
}

#endif /* WD_KMDF_SKELETON_FILE_CONTEXT_H */
