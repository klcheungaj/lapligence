/*
 * Narrow C ABI for the vendored Slang frontend.
 *
 * The caller owns all input memory for the duration of llg_slang_compile().
 * A successful snapshot owns all exported memory; views borrow from the snapshot
 * and remain valid until llg_slang_snapshot_destroy(). Destruction accepts NULL.
 */
#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LLG_SLANG_ABI_VERSION 1u
#define LLG_SLANG_INVALID_ID UINT64_MAX

typedef struct LlgSlangSnapshot LlgSlangSnapshot;
typedef struct LlgSlangError LlgSlangError;

typedef struct {
  const uint8_t* data;
  uint64_t len;
} LlgSlangString;

typedef struct {
  LlgSlangString name;
  LlgSlangString text;
  uint32_t flags;
  uint32_t reserved;
} LlgSlangSource;

enum {
  /* Parse and add this buffer to the compilation. Buffers without this flag
   * are admitted include files available only through cache-only lookup. */
  LLG_SLANG_SOURCE_COMPILATION_UNIT = 1u << 0
};

typedef struct {
  LlgSlangString name;
  LlgSlangString value;
  uint32_t has_value;
  uint32_t reserved;
} LlgSlangDefine;

typedef struct {
  uint64_t max_sources;
  uint64_t max_source_bytes;
  uint64_t max_diagnostics;
  uint64_t max_instances;
  uint64_t max_parameters;
  uint64_t max_types;
  uint64_t max_value_bits;
  uint64_t max_related_diagnostics;
  uint64_t max_output_bytes;
} LlgSlangLimits;

typedef struct {
  uint32_t abi_version;
  uint32_t flags;
  const LlgSlangSource* sources;
  uint64_t source_count;
  const LlgSlangDefine* defines;
  uint64_t define_count;
  const LlgSlangString* top_modules;
  uint64_t top_module_count;
  const LlgSlangString* include_dirs;
  uint64_t include_dir_count;
  const LlgSlangDefine* parameter_overrides;
  uint64_t parameter_override_count;
  LlgSlangLimits limits;
} LlgSlangCompileRequest;

enum {
  LLG_SLANG_STATUS_OK = 0,
  LLG_SLANG_STATUS_INVALID_ARGUMENT = 1,
  LLG_SLANG_STATUS_LIMIT_EXCEEDED = 2,
  LLG_SLANG_STATUS_FRONTEND_ERROR = 3,
  LLG_SLANG_STATUS_INTERNAL_ERROR = 4
};

enum {
  LLG_SLANG_SNAPSHOT_HAS_ERRORS = 1u << 0,
  LLG_SLANG_SNAPSHOT_ANALYSIS_RAN = 1u << 1
};

enum {
  LLG_SLANG_DIAG_COMPILATION = 1,
  LLG_SLANG_DIAG_ANALYSIS = 2
};

enum {
  LLG_SLANG_SEVERITY_IGNORED = 0,
  LLG_SLANG_SEVERITY_NOTE = 1,
  LLG_SLANG_SEVERITY_WARNING = 2,
  LLG_SLANG_SEVERITY_ERROR = 3,
  LLG_SLANG_SEVERITY_FATAL = 4
};

/* Byte ranges are zero-based, half-open, and refer to an entry in files.
 * Unknown locations use file_id == LLG_SLANG_INVALID_ID and zero offsets. */
typedef struct {
  uint64_t file_id;
  uint64_t start;
  uint64_t end;
} LlgSlangSourceRange;

typedef struct {
  uint64_t id;
  LlgSlangString name;
  uint64_t byte_len;
} LlgSlangFile;

typedef struct {
  uint32_t provider;
  uint32_t severity;
  uint32_t subsystem;
  uint32_t code;
  LlgSlangString name;
  LlgSlangString option_name;
  LlgSlangString message;
  LlgSlangSourceRange primary;
  uint64_t related_start;
  uint64_t related_count;
} LlgSlangDiagnostic;

typedef struct {
  LlgSlangSourceRange range;
  LlgSlangString message;
} LlgSlangRelatedDiagnostic;

enum {
  LLG_SLANG_INSTANCE_MODULE = 1,
  LLG_SLANG_INSTANCE_INTERFACE = 2,
  LLG_SLANG_INSTANCE_PROGRAM = 3,
  LLG_SLANG_INSTANCE_UNKNOWN = 255
};

typedef struct {
  uint64_t id;
  uint64_t parent_id;
  uint32_t kind;
  uint32_t reserved;
  LlgSlangString name;
  LlgSlangString definition_name;
  LlgSlangSourceRange declaration;
  uint64_t parameter_start;
  uint64_t parameter_count;
} LlgSlangInstance;

enum {
  LLG_SLANG_TYPE_INTEGRAL = 1,
  LLG_SLANG_TYPE_FLOATING = 2,
  LLG_SLANG_TYPE_STRING = 3,
  LLG_SLANG_TYPE_AGGREGATE = 4,
  LLG_SLANG_TYPE_OTHER = 255
};

enum {
  LLG_SLANG_TYPE_SIGNED = 1u << 0,
  LLG_SLANG_TYPE_FOUR_STATE = 1u << 1,
  LLG_SLANG_TYPE_FIXED_SIZE = 1u << 2
};

typedef struct {
  uint64_t id;
  uint32_t kind;
  uint32_t flags;
  uint64_t bit_width;
  LlgSlangString display_name;
} LlgSlangType;

enum {
  LLG_SLANG_VALUE_NONE = 0,
  LLG_SLANG_VALUE_INTEGER = 1,
  LLG_SLANG_VALUE_REAL = 2,
  LLG_SLANG_VALUE_SHORT_REAL = 3,
  LLG_SLANG_VALUE_STRING = 4,
  LLG_SLANG_VALUE_OTHER = 255
};

/* Integer words are little endian by significance. For each bit, the pair
 * (unknown, value) encodes 0=(0,0), 1=(0,1), X=(1,0), Z=(1,1).
 * Bits beyond bit_width in the last word are always zero. */
typedef struct {
  uint32_t kind;
  uint32_t is_signed;
  uint64_t bit_width;
  uint64_t value_word_start;
  uint64_t unknown_word_start;
  uint64_t word_count;
  uint64_t real_bits; /* IEEE-754 bits; low 32 bits for SHORT_REAL. */
  LlgSlangString text;
} LlgSlangConstant;

enum {
  LLG_SLANG_PARAMETER_VALUE = 1,
  LLG_SLANG_PARAMETER_TYPE = 2
};

enum {
  LLG_SLANG_PARAMETER_LOCAL = 1u << 0,
  LLG_SLANG_PARAMETER_PORT = 1u << 1
};

typedef struct {
  uint64_t owner_instance_id;
  uint32_t kind;
  uint32_t flags;
  LlgSlangString name;
  LlgSlangSourceRange declaration;
  uint64_t type_id;
  uint64_t constant_id;
} LlgSlangParameter;

typedef struct {
  uint32_t abi_version;
  uint32_t flags;
  const LlgSlangFile* files;
  uint64_t file_count;
  const LlgSlangDiagnostic* diagnostics;
  uint64_t diagnostic_count;
  const LlgSlangRelatedDiagnostic* related_diagnostics;
  uint64_t related_diagnostic_count;
  const LlgSlangInstance* instances;
  uint64_t instance_count;
  const LlgSlangParameter* parameters;
  uint64_t parameter_count;
  const LlgSlangType* types;
  uint64_t type_count;
  const LlgSlangConstant* constants;
  uint64_t constant_count;
  const uint64_t* value_words;
  uint64_t value_word_count;
} LlgSlangSnapshotView;

typedef struct {
  uint32_t status;
  uint32_t reserved;
  LlgSlangString message;
} LlgSlangErrorView;

/* Semantic errors are represented in a successful snapshot. A non-OK return
 * means argument, resource, setup, or bridge failure and sets out_error. */
uint32_t llg_slang_compile(const LlgSlangCompileRequest* request,
                           LlgSlangSnapshot** out_snapshot,
                           LlgSlangError** out_error);
uint32_t llg_slang_snapshot_view(const LlgSlangSnapshot* snapshot,
                                 LlgSlangSnapshotView* out_view,
                                 LlgSlangError** out_error);
uint32_t llg_slang_error_view(const LlgSlangError* error,
                              LlgSlangErrorView* out_view);
void llg_slang_snapshot_destroy(LlgSlangSnapshot* snapshot);
void llg_slang_error_destroy(LlgSlangError* error);

#ifdef __cplusplus
}
#endif
