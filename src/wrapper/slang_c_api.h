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

#define LLG_SLANG_ABI_VERSION 2u
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
  uint64_t max_semantic_nodes;
  uint64_t max_semantic_edges;
  uint64_t max_lexical_tokens;
  uint64_t max_type_ranges;
  uint64_t max_type_members;
  uint64_t max_constants;
} LlgSlangLimits;

enum {
  /* Parse all compilation units as library units. Definitions are checked as
   * uninstantiated scopes instead of being inferred as recursive design tops. */
  LLG_SLANG_COMPILE_LIBRARY_UNITS = 1u << 0,
  /* Select the complete compilation language policy. If neither edition bit
   * is set, the bridge uses SystemVerilog-2009 for compatibility. These bits
   * affect the global policy only; `begin_keywords` remains lexical. */
  LLG_SLANG_COMPILE_EDITION_VERILOG_2001 = 1u << 1,
  LLG_SLANG_COMPILE_EDITION_SYSTEMVERILOG_2009 = 1u << 2,
  /* Parse all admitted compilation-unit buffers in source order as one unit.
   * When absent, each compilation-unit buffer is parsed separately. */
  LLG_SLANG_COMPILE_MERGED_COMPILATION_UNITS = 1u << 3
};

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
  LLG_SLANG_SNAPSHOT_ANALYSIS_RAN = 1u << 1,
  LLG_SLANG_SNAPSHOT_EDITION_VERILOG_2001 = 1u << 8,
  LLG_SLANG_SNAPSHOT_EDITION_SYSTEMVERILOG_2009 = 1u << 9,
  LLG_SLANG_SNAPSHOT_MERGED_COMPILATION_UNITS = 1u << 10
};

enum {
  LLG_SLANG_DIAG_COMPILATION = 1,
  LLG_SLANG_DIAG_ANALYSIS = 2
};

enum {
  LLG_SLANG_DIAG_SUBSYSTEM_INVALID = 0,
  LLG_SLANG_DIAG_SUBSYSTEM_GENERAL = 1,
  LLG_SLANG_DIAG_SUBSYSTEM_LEXER = 2,
  LLG_SLANG_DIAG_SUBSYSTEM_NUMERIC = 3,
  LLG_SLANG_DIAG_SUBSYSTEM_PREPROCESSOR = 4,
  LLG_SLANG_DIAG_SUBSYSTEM_PARSER = 5,
  LLG_SLANG_DIAG_SUBSYSTEM_DECLARATIONS = 6,
  LLG_SLANG_DIAG_SUBSYSTEM_EXPRESSIONS = 7,
  LLG_SLANG_DIAG_SUBSYSTEM_STATEMENTS = 8,
  LLG_SLANG_DIAG_SUBSYSTEM_TYPES = 9,
  LLG_SLANG_DIAG_SUBSYSTEM_LOOKUP = 10,
  LLG_SLANG_DIAG_SUBSYSTEM_SYSFUNCS = 11,
  LLG_SLANG_DIAG_SUBSYSTEM_CONST_EVAL = 12,
  LLG_SLANG_DIAG_SUBSYSTEM_COMPILATION = 13,
  LLG_SLANG_DIAG_SUBSYSTEM_ANALYSIS = 14,
  LLG_SLANG_DIAG_SUBSYSTEM_META = 15,
  LLG_SLANG_DIAG_SUBSYSTEM_DRIVER = 16,
  LLG_SLANG_DIAG_SUBSYSTEM_TIDY = 17,
  LLG_SLANG_DIAG_SUBSYSTEM_NETLIST = 18
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
  LLG_SLANG_TYPE_ENUM = 5,
  LLG_SLANG_TYPE_PACKED_ARRAY = 6,
  LLG_SLANG_TYPE_FIXED_UNPACKED_ARRAY = 7,
  LLG_SLANG_TYPE_DYNAMIC_ARRAY = 8,
  LLG_SLANG_TYPE_ASSOCIATIVE_ARRAY = 9,
  LLG_SLANG_TYPE_QUEUE = 10,
  LLG_SLANG_TYPE_PACKED_STRUCT = 11,
  LLG_SLANG_TYPE_PACKED_UNION = 12,
  LLG_SLANG_TYPE_UNPACKED_STRUCT = 13,
  LLG_SLANG_TYPE_UNPACKED_UNION = 14,
  LLG_SLANG_TYPE_CLASS = 15,
  LLG_SLANG_TYPE_CHANDLE = 16,
  LLG_SLANG_TYPE_EVENT = 17,
  LLG_SLANG_TYPE_VOID = 18,
  LLG_SLANG_TYPE_VIRTUAL_INTERFACE = 19,
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
  uint64_t element_type_id;
  uint64_t index_type_id;
  uint64_t range_start;
  uint64_t range_count;
  uint64_t member_start;
  uint64_t member_count;
} LlgSlangType;

enum {
  LLG_SLANG_TYPE_RANGE_PACKED = 1,
  LLG_SLANG_TYPE_RANGE_UNPACKED = 2,
  LLG_SLANG_TYPE_RANGE_QUEUE_BOUND = 3
};

typedef struct {
  int64_t left;
  int64_t right;
  uint32_t kind;
  uint32_t reserved;
} LlgSlangTypeRange;

typedef struct {
  LlgSlangString name;
  uint64_t type_id;
  uint64_t bit_offset;
  uint64_t bit_width;
} LlgSlangTypeMember;

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

/* Repository-owned semantic taxonomy. Slang enum values never cross the ABI;
 * constructs without a lossless mapping use UNSUPPORTED and retain their
 * Slang kind name in detail. */
enum {
  LLG_SLANG_SEMANTIC_INSTANCE = 1,
  LLG_SLANG_SEMANTIC_PACKAGE = 2,
  LLG_SLANG_SEMANTIC_CLASS = 3,
  LLG_SLANG_SEMANTIC_GENERATE_SCOPE = 4,
  LLG_SLANG_SEMANTIC_PORT = 5,
  LLG_SLANG_SEMANTIC_MODPORT = 6,
  LLG_SLANG_SEMANTIC_INTERFACE_CONNECTION = 7,
  LLG_SLANG_SEMANTIC_NET = 8,
  LLG_SLANG_SEMANTIC_VARIABLE = 9,
  LLG_SLANG_SEMANTIC_ARRAY = 10,
  LLG_SLANG_SEMANTIC_NAMED_EVENT = 11,
  LLG_SLANG_SEMANTIC_PARAMETER = 12,
  LLG_SLANG_SEMANTIC_PROCESS = 13,
  LLG_SLANG_SEMANTIC_CONTINUOUS_ASSIGN = 14,
  LLG_SLANG_SEMANTIC_PRIMITIVE = 15,
  LLG_SLANG_SEMANTIC_SUBROUTINE = 16,
  LLG_SLANG_SEMANTIC_ARGUMENT = 17,
  LLG_SLANG_SEMANTIC_STATEMENT = 18,
  LLG_SLANG_SEMANTIC_EXPRESSION = 19,
  LLG_SLANG_SEMANTIC_SYSTEM_CALL = 20,
  LLG_SLANG_SEMANTIC_METHOD_CALL = 21,
  LLG_SLANG_SEMANTIC_FUNCTION_CALL = 22,
  LLG_SLANG_SEMANTIC_ENUM_CONSTANT = 23,
  LLG_SLANG_SEMANTIC_DEFINITION = 24,
  LLG_SLANG_SEMANTIC_SCOPE = 25,
  LLG_SLANG_SEMANTIC_TIMING_CONTROL = 26,
  LLG_SLANG_SEMANTIC_NET_ALIAS = 27,
  /* Assertion expressions are retained as typed property/sequence nodes so
   * downstream consumers can reject unsupported forms without falling back
   * to native Slang pointers. */
  LLG_SLANG_SEMANTIC_ASSERTION_EXPR = 28,
  LLG_SLANG_SEMANTIC_UNSUPPORTED = 255
};

enum {
  LLG_SLANG_SEMANTIC_BAD = 1u << 0,
  LLG_SLANG_SEMANTIC_UNINSTANTIATED = 1u << 1,
  LLG_SLANG_SEMANTIC_AUTOMATIC = 1u << 2,
  LLG_SLANG_SEMANTIC_STATIC = 1u << 3,
  LLG_SLANG_SEMANTIC_TOP = 1u << 4,
  LLG_SLANG_SEMANTIC_IMPLICIT = 1u << 5,
  LLG_SLANG_SEMANTIC_LOCAL = 1u << 6,
  LLG_SLANG_SEMANTIC_NONBLOCKING = 1u << 7,
  LLG_SLANG_SEMANTIC_INPUT = 1u << 8,
  LLG_SLANG_SEMANTIC_OUTPUT = 1u << 9,
  LLG_SLANG_SEMANTIC_INOUT = 1u << 10,
  LLG_SLANG_SEMANTIC_REF = 1u << 11,
  LLG_SLANG_SEMANTIC_IMPLICIT_CONVERSION = 1u << 12,
  LLG_SLANG_SEMANTIC_MODULE = 1u << 13,
  LLG_SLANG_SEMANTIC_INTERFACE = 1u << 14,
  LLG_SLANG_SEMANTIC_PROGRAM = 1u << 15,
  LLG_SLANG_SEMANTIC_INDEXED_UP = 1u << 16,
  LLG_SLANG_SEMANTIC_INDEXED_DOWN = 1u << 17,
  LLG_SLANG_SEMANTIC_CASE_X_OR_Z = 1u << 18,
  LLG_SLANG_SEMANTIC_CASE_Z = 1u << 19,
  LLG_SLANG_SEMANTIC_CASE_INSIDE = 1u << 20,
  LLG_SLANG_SEMANTIC_POSEDGE = 1u << 21,
  LLG_SLANG_SEMANTIC_NEGEDGE = 1u << 22,
  LLG_SLANG_SEMANTIC_BOTH_EDGES = 1u << 23,
  LLG_SLANG_SEMANTIC_PRIMITIVE_DECLARATION = 1u << 24,
  LLG_SLANG_SEMANTIC_PRIMITIVE_INSTANCE = 1u << 25,
  LLG_SLANG_SEMANTIC_PRIMITIVE_PORT = 1u << 26,
  LLG_SLANG_SEMANTIC_TASK = 1u << 27,
  LLG_SLANG_SEMANTIC_PORT_CONNECTION_PRESENT = 1u << 28,
  LLG_SLANG_SEMANTIC_PORT_CONNECTION_OPEN = 1u << 29,
  LLG_SLANG_SEMANTIC_PROPAGATED_CONVERSION = 1u << 30,
  LLG_SLANG_SEMANTIC_METHOD_WITH_CLAUSE = 1u << 31
};

/* Argument-only qualifiers carried in LlgSlangSemanticNode::auxiliary. */
enum {
  LLG_SLANG_ARGUMENT_CONST_REF = 1ull << 0,
  LLG_SLANG_ARGUMENT_REF_STATIC = 1ull << 1
};

/* Subroutine qualifiers carried in LlgSlangSemanticNode::auxiliary. */
enum {
  LLG_SLANG_SUBROUTINE_STATIC = 1ull << 0,
  LLG_SLANG_SUBROUTINE_VIRTUAL = 1ull << 1,
  LLG_SLANG_SUBROUTINE_PURE = 1ull << 2,
  LLG_SLANG_SUBROUTINE_FINAL = 1ull << 3,
  LLG_SLANG_SUBROUTINE_CONSTRUCTOR = 1ull << 4,
  /* The remaining bits preserve the DPI-C import contract after native Slang
   * memory has been released.  They are meaningful only on subroutine nodes;
   * definition_name carries the optional C linkage identifier. */
  LLG_SLANG_SUBROUTINE_DPI_IMPORT = 1ull << 8,
  LLG_SLANG_SUBROUTINE_DPI_CONTEXT = 1ull << 9,
  LLG_SLANG_SUBROUTINE_DPI_PURE = 1ull << 10
};

/* Class qualifiers carried in LlgSlangSemanticNode::auxiliary. */
enum {
  LLG_SLANG_CLASS_ABSTRACT = 1ull << 0,
  LLG_SLANG_CLASS_FINAL = 1ull << 1,
  LLG_SLANG_CLASS_INTERFACE = 1ull << 2
};

/* New-class expression qualifiers carried in auxiliary. */
enum {
  LLG_SLANG_NEW_CLASS_SUPER = 1ull << 0
};

/* User subroutine-call qualifiers carried in auxiliary. */
enum {
  LLG_SLANG_CALL_SUPER = 1ull << 0
};

/* Conditional/case qualifiers carried in LlgSlangSemanticNode::auxiliary.
 * These values are owned by this ABI; they intentionally do not depend on
 * Slang's enum representation. */
enum {
  LLG_SLANG_UNIQUE_PRIORITY_NONE = 0,
  LLG_SLANG_UNIQUE_PRIORITY_UNIQUE = 1,
  LLG_SLANG_UNIQUE_PRIORITY_UNIQUE0 = 2,
  LLG_SLANG_UNIQUE_PRIORITY_PRIORITY = 3
};

/* Immediate assertion metadata carried in LlgSlangSemanticNode::auxiliary.
 * Deferred and final assertions remain explicit so downstream consumers can
 * reject them rather than silently executing them as ordinary assertions. */
enum {
  LLG_SLANG_ASSERTION_DEFERRED = 1ull << 0,
  LLG_SLANG_ASSERTION_FINAL = 1ull << 1
};

enum {
  LLG_SLANG_SUBKIND_NONE = 0,
  LLG_SLANG_PROCESS_INITIAL = 1,
  LLG_SLANG_PROCESS_FINAL = 2,
  LLG_SLANG_PROCESS_ALWAYS = 3,
  LLG_SLANG_PROCESS_ALWAYS_COMB = 4,
  LLG_SLANG_PROCESS_ALWAYS_LATCH = 5,
  LLG_SLANG_PROCESS_ALWAYS_FF = 6,
  LLG_SLANG_STMT_BLOCK = 32,
  LLG_SLANG_STMT_IF = 33,
  LLG_SLANG_STMT_CASE = 34,
  LLG_SLANG_STMT_FOR = 35,
  LLG_SLANG_STMT_WHILE = 36,
  LLG_SLANG_STMT_DO_WHILE = 37,
  LLG_SLANG_STMT_REPEAT = 38,
  LLG_SLANG_STMT_FOREVER = 39,
  LLG_SLANG_STMT_TIMED = 40,
  LLG_SLANG_STMT_EVENT_TRIGGER = 41,
  LLG_SLANG_STMT_WAIT = 42,
  LLG_SLANG_STMT_RETURN = 43,
  LLG_SLANG_STMT_BREAK = 44,
  LLG_SLANG_STMT_CONTINUE = 45,
  LLG_SLANG_STMT_DISABLE = 46,
  LLG_SLANG_STMT_EMPTY = 47,
  LLG_SLANG_STMT_EXPRESSION = 48,
  LLG_SLANG_STMT_VARIABLE_DECL = 49,
  LLG_SLANG_STMT_PROCEDURAL_ASSIGN = 50,
  LLG_SLANG_STMT_FORCE = 51,
  LLG_SLANG_STMT_PROCEDURAL_DEASSIGN = 52,
  LLG_SLANG_STMT_RELEASE = 53,
  LLG_SLANG_STMT_WAIT_FORK = 54,
  LLG_SLANG_STMT_DISABLE_FORK = 55,
  LLG_SLANG_STMT_FORK_JOIN_ALL = 56,
  LLG_SLANG_STMT_FORK_JOIN_ANY = 57,
  LLG_SLANG_STMT_FORK_JOIN_NONE = 58,
  LLG_SLANG_STMT_FOREACH = 59,
  LLG_SLANG_STMT_LIST = 60,
  /* Immediate assertion statements use repository-owned tags so the safe
   * facade never depends on Slang's StatementKind or AssertionKind values. */
  LLG_SLANG_STMT_IMMEDIATE_ASSERT = 61,
  LLG_SLANG_STMT_IMMEDIATE_ASSUME = 62,
  LLG_SLANG_STMT_IMMEDIATE_COVER = 63,
  LLG_SLANG_STMT_CONCURRENT_ASSERT = 64,
  LLG_SLANG_STMT_CONCURRENT_ASSUME = 65,
  LLG_SLANG_STMT_CONCURRENT_COVER = 66,
  /* AssertionExprKind tags. */
  LLG_SLANG_ASSERTION_EXPR_INVALID = 1,
  LLG_SLANG_ASSERTION_EXPR_SIMPLE = 2,
  LLG_SLANG_ASSERTION_EXPR_SEQUENCE_CONCAT = 3,
  LLG_SLANG_ASSERTION_EXPR_SEQUENCE_WITH_MATCH = 4,
  LLG_SLANG_ASSERTION_EXPR_UNARY = 5,
  LLG_SLANG_ASSERTION_EXPR_BINARY = 6,
  LLG_SLANG_ASSERTION_EXPR_FIRST_MATCH = 7,
  LLG_SLANG_ASSERTION_EXPR_CLOCKING = 8,
  LLG_SLANG_ASSERTION_EXPR_STRONG_WEAK = 9,
  LLG_SLANG_ASSERTION_EXPR_ABORT = 10,
  LLG_SLANG_ASSERTION_EXPR_CONDITIONAL = 11,
  LLG_SLANG_ASSERTION_EXPR_CASE = 12,
  LLG_SLANG_ASSERTION_EXPR_DISABLE_IFF = 13,
  LLG_SLANG_EXPR_LITERAL = 64,
  LLG_SLANG_EXPR_REFERENCE = 65,
  LLG_SLANG_EXPR_UNARY = 66,
  LLG_SLANG_EXPR_BINARY = 67,
  LLG_SLANG_EXPR_CONDITIONAL = 68,
  LLG_SLANG_EXPR_CONCAT = 69,
  LLG_SLANG_EXPR_REPLICATION = 70,
  LLG_SLANG_EXPR_ASSIGNMENT = 71,
  LLG_SLANG_EXPR_CONVERSION = 72,
  LLG_SLANG_EXPR_ELEMENT_SELECT = 73,
  LLG_SLANG_EXPR_RANGE_SELECT = 74,
  LLG_SLANG_EXPR_MEMBER_ACCESS = 75,
  LLG_SLANG_EXPR_CALL = 76,
  LLG_SLANG_EXPR_INSIDE = 77,
  LLG_SLANG_EXPR_ASSIGNMENT_PATTERN = 78,
  LLG_SLANG_EXPR_MIN_TYP_MAX = 80,
  LLG_SLANG_EXPR_PATTERN_MEMBER_KEY = 81,
  LLG_SLANG_EXPR_PATTERN_TYPE_KEY = 82,
  LLG_SLANG_EXPR_PATTERN_INDEX_KEY = 83,
  LLG_SLANG_EXPR_PATTERN_DEFAULT_KEY = 84,
  LLG_SLANG_EXPR_TIME_LITERAL = 85,
  LLG_SLANG_EXPR_NEW_ARRAY = 86,
  LLG_SLANG_EXPR_NEW_CLASS = 87,
  LLG_SLANG_EXPR_NEW_COVERGROUP = 88,
  LLG_SLANG_EXPR_VALUE_RANGE = 89,
  LLG_SLANG_EXPR_ASSERTION_INSTANCE = 90,
  /* `@(event)` arguments carried by sampled-value system functions. */
  LLG_SLANG_EXPR_CLOCKING_EVENT = 91,
  LLG_SLANG_TIMING_DELAY = 112,
  LLG_SLANG_TIMING_SIGNAL_EVENT = 113,
  LLG_SLANG_TIMING_EVENT_LIST = 114,
  LLG_SLANG_TIMING_IMPLICIT_EVENT = 115,
  LLG_SLANG_TIMING_REPEATED_EVENT = 116,
  LLG_SLANG_TIMING_CYCLE_DELAY = 117,
  LLG_SLANG_TIMING_ONE_STEP_DELAY = 118,
  LLG_SLANG_NET_WIRE = 128,
  LLG_SLANG_NET_WAND = 129,
  LLG_SLANG_NET_WOR = 130,
  LLG_SLANG_NET_TRI = 131,
  LLG_SLANG_NET_TRIAND = 132,
  LLG_SLANG_NET_TRIOR = 133,
  LLG_SLANG_NET_TRI0 = 134,
  LLG_SLANG_NET_TRI1 = 135,
  LLG_SLANG_NET_TRIREG = 136,
  LLG_SLANG_NET_SUPPLY0 = 137,
  LLG_SLANG_NET_SUPPLY1 = 138,
  LLG_SLANG_NET_UWIRE = 139,
  LLG_SLANG_NET_INTERCONNECT = 140,
  LLG_SLANG_NET_USER_DEFINED = 141,
  LLG_SLANG_PRIMITIVE_FIXED = 160,
  LLG_SLANG_PRIMITIVE_N_INPUT = 161,
  LLG_SLANG_PRIMITIVE_N_OUTPUT = 162,
  LLG_SLANG_PRIMITIVE_BIDIRECTIONAL_SWITCH = 163,
  LLG_SLANG_PRIMITIVE_USER_DEFINED = 164,
  LLG_SLANG_INSTANCE_SINGLE = 192,
  LLG_SLANG_INSTANCE_ARRAY = 193,
  LLG_SLANG_INSTANCE_BODY = 194,
  LLG_SLANG_GENERATE_BLOCK = 195,
  LLG_SLANG_GENERATE_BLOCK_ARRAY = 196,
  LLG_SLANG_PRIMITIVE_AND = 200,
  LLG_SLANG_PRIMITIVE_NAND = 201,
  LLG_SLANG_PRIMITIVE_NOR = 202,
  LLG_SLANG_PRIMITIVE_OR = 203,
  LLG_SLANG_PRIMITIVE_XOR = 204,
  LLG_SLANG_PRIMITIVE_XNOR = 205,
  LLG_SLANG_PRIMITIVE_BUF = 206,
  LLG_SLANG_PRIMITIVE_NOT = 207,
  LLG_SLANG_PRIMITIVE_BUFIF0 = 208,
  LLG_SLANG_PRIMITIVE_BUFIF1 = 209,
  LLG_SLANG_PRIMITIVE_NOTIF0 = 210,
  LLG_SLANG_PRIMITIVE_NOTIF1 = 211,
  LLG_SLANG_PRIMITIVE_NMOS = 212,
  LLG_SLANG_PRIMITIVE_PMOS = 213,
  LLG_SLANG_PRIMITIVE_CMOS = 214,
  LLG_SLANG_PRIMITIVE_RNMOS = 215,
  LLG_SLANG_PRIMITIVE_RPMOS = 216,
  LLG_SLANG_PRIMITIVE_RCMOS = 217,
  LLG_SLANG_PRIMITIVE_RTRAN = 218,
  LLG_SLANG_PRIMITIVE_RTRANIF0 = 219,
  LLG_SLANG_PRIMITIVE_RTRANIF1 = 220,
  LLG_SLANG_PRIMITIVE_TRAN = 221,
  LLG_SLANG_PRIMITIVE_TRANIF0 = 222,
  LLG_SLANG_PRIMITIVE_TRANIF1 = 223,
  LLG_SLANG_PRIMITIVE_PULLUP = 224,
  LLG_SLANG_PRIMITIVE_PULLDOWN = 225,
  LLG_SLANG_PRIMITIVE_UDP_SEQUENTIAL = 226,
  LLG_SLANG_PRIMITIVE_UDP_COMBINATIONAL = 227,
  LLG_SLANG_CONTINUOUS_ASSIGN_NET_DECL = 228,
  LLG_SLANG_VARIABLE_GENVAR = 229,
  LLG_SLANG_SCOPE_CLOCKING_BLOCK = 230,
  LLG_SLANG_VARIABLE_CLOCKING = 231
};

/* Clocking metadata carried in LlgSlangSemanticNode::auxiliary. Edge codes
 * are owned by this ABI: none=0, posedge=1, negedge=2, both-edges=3. */
enum {
  LLG_SLANG_CLOCKING_BLOCK_DEFAULT = 1ull << 0,
  LLG_SLANG_CLOCKING_BLOCK_GLOBAL = 1ull << 1,
  LLG_SLANG_CLOCKING_INPUT_EDGE_SHIFT = 2,
  LLG_SLANG_CLOCKING_OUTPUT_EDGE_SHIFT = 4,
  LLG_SLANG_CLOCKING_VAR_OUTPUT_EDGE_SHIFT = 2,
  LLG_SLANG_CLOCKING_EDGE_MASK = 3
};

enum {
  /* Assertion-expression scalar metadata. */
  LLG_SLANG_ASSERTION_REPETITION = 1ull << 0,
  LLG_SLANG_ASSERTION_RANGE = 1ull << 1,
  LLG_SLANG_ASSERTION_STRONG = 1ull << 2,
  LLG_SLANG_ASSERTION_ABORT_REJECT = 1ull << 3,
  LLG_SLANG_ASSERTION_ABORT_SYNC = 1ull << 4
};

enum {
  LLG_SLANG_OP_NONE = 0,
  LLG_SLANG_OP_PLUS = 1,
  LLG_SLANG_OP_MINUS = 2,
  LLG_SLANG_OP_MULTIPLY = 3,
  LLG_SLANG_OP_DIVIDE = 4,
  LLG_SLANG_OP_MODULO = 5,
  LLG_SLANG_OP_POWER = 6,
  LLG_SLANG_OP_BIT_NOT = 7,
  LLG_SLANG_OP_BIT_AND = 8,
  LLG_SLANG_OP_BIT_OR = 9,
  LLG_SLANG_OP_BIT_XOR = 10,
  LLG_SLANG_OP_BIT_NAND = 11,
  LLG_SLANG_OP_BIT_NOR = 12,
  LLG_SLANG_OP_BIT_XNOR = 13,
  LLG_SLANG_OP_LOGICAL_NOT = 14,
  LLG_SLANG_OP_LOGICAL_AND = 15,
  LLG_SLANG_OP_LOGICAL_OR = 16,
  LLG_SLANG_OP_LOGICAL_IMPLICATION = 17,
  LLG_SLANG_OP_LOGICAL_EQUIVALENCE = 18,
  LLG_SLANG_OP_EQUAL = 19,
  LLG_SLANG_OP_NOT_EQUAL = 20,
  LLG_SLANG_OP_CASE_EQUAL = 21,
  LLG_SLANG_OP_CASE_NOT_EQUAL = 22,
  LLG_SLANG_OP_WILDCARD_EQUAL = 23,
  LLG_SLANG_OP_WILDCARD_NOT_EQUAL = 24,
  LLG_SLANG_OP_GREATER = 25,
  LLG_SLANG_OP_GREATER_EQUAL = 26,
  LLG_SLANG_OP_LESS = 27,
  LLG_SLANG_OP_LESS_EQUAL = 28,
  LLG_SLANG_OP_SHIFT_LEFT = 29,
  LLG_SLANG_OP_SHIFT_RIGHT = 30,
  LLG_SLANG_OP_ARITH_SHIFT_LEFT = 31,
  LLG_SLANG_OP_ARITH_SHIFT_RIGHT = 32,
  LLG_SLANG_OP_PRE_INCREMENT = 33,
  LLG_SLANG_OP_PRE_DECREMENT = 34,
  LLG_SLANG_OP_POST_INCREMENT = 35,
  LLG_SLANG_OP_POST_DECREMENT = 36,
  LLG_SLANG_OP_CONCAT = 37,
  LLG_SLANG_OP_REPLICATE = 38,
  LLG_SLANG_OP_CONDITIONAL = 39,
  LLG_SLANG_OP_STREAM_LEFT = 40,
  LLG_SLANG_OP_STREAM_RIGHT = 41,
  LLG_SLANG_OP_ASSIGN = 42,
  LLG_SLANG_OP_INSIDE = 43,
  LLG_SLANG_OP_ASSIGNMENT_PATTERN = 44,
  LLG_SLANG_OP_MIN_TYP_MAX = 45,
  LLG_SLANG_OP_MULTI_ASSIGNMENT_PATTERN = 46,
  LLG_SLANG_OP_LIST = 47,
  LLG_SLANG_OP_ASSERTION_AND = 48,
  LLG_SLANG_OP_ASSERTION_OR = 49,
  LLG_SLANG_OP_ASSERTION_INTERSECT = 50,
  LLG_SLANG_OP_ASSERTION_THROUGHOUT = 51,
  LLG_SLANG_OP_ASSERTION_WITHIN = 52,
  LLG_SLANG_OP_ASSERTION_IFF = 53,
  LLG_SLANG_OP_ASSERTION_UNTIL = 54,
  LLG_SLANG_OP_ASSERTION_SUNTIL = 55,
  LLG_SLANG_OP_ASSERTION_UNTIL_WITH = 56,
  LLG_SLANG_OP_ASSERTION_SUNTIL_WITH = 57,
  LLG_SLANG_OP_ASSERTION_IMPLIES = 58,
  LLG_SLANG_OP_ASSERTION_OVERLAPPED_IMPLIES = 59,
  LLG_SLANG_OP_ASSERTION_NONOVERLAPPED_IMPLIES = 60,
  LLG_SLANG_OP_ASSERTION_OVERLAPPED_FOLLOWED_BY = 61,
  LLG_SLANG_OP_ASSERTION_NONOVERLAPPED_FOLLOWED_BY = 62,
  LLG_SLANG_OP_ASSERTION_NOT = 63,
  LLG_SLANG_OP_ASSERTION_NEXT_TIME = 64,
  LLG_SLANG_OP_ASSERTION_SNEXT_TIME = 65,
  LLG_SLANG_OP_ASSERTION_ALWAYS = 66,
  LLG_SLANG_OP_ASSERTION_SALWAYS = 67,
  LLG_SLANG_OP_ASSERTION_EVENTUALLY = 68,
  LLG_SLANG_OP_ASSERTION_SEVENTUALLY = 69
};

enum {
  LLG_SLANG_TIME_UNIT_NONE = 0,
  LLG_SLANG_TIME_UNIT_SECONDS = 1,
  LLG_SLANG_TIME_UNIT_MILLISECONDS = 2,
  LLG_SLANG_TIME_UNIT_MICROSECONDS = 3,
  LLG_SLANG_TIME_UNIT_NANOSECONDS = 4,
  LLG_SLANG_TIME_UNIT_PICOSECONDS = 5,
  LLG_SLANG_TIME_UNIT_FEMTOSECONDS = 6
};

enum {
  LLG_SLANG_STRENGTH_UNSPECIFIED = 0,
  LLG_SLANG_STRENGTH_SUPPLY = 1,
  LLG_SLANG_STRENGTH_STRONG = 2,
  LLG_SLANG_STRENGTH_PULL = 3,
  LLG_SLANG_STRENGTH_WEAK = 4,
  LLG_SLANG_STRENGTH_HIGH_Z = 5
};

enum {
  LLG_SLANG_VARIABLE_LIFETIME_NONE = 0,
  LLG_SLANG_VARIABLE_LIFETIME_STATIC = 1,
  LLG_SLANG_VARIABLE_LIFETIME_AUTOMATIC = 2
};

enum {
  LLG_SLANG_EDGE_CHILD = 1,
  LLG_SLANG_EDGE_HIGH_CONNECTION = 2,
  LLG_SLANG_EDGE_LOW_CONNECTION = 3,
  LLG_SLANG_EDGE_INITIALIZER = 4,
  LLG_SLANG_EDGE_LHS = 5,
  LLG_SLANG_EDGE_RHS = 6,
  LLG_SLANG_EDGE_CONDITION = 7,
  LLG_SLANG_EDGE_THEN = 8,
  LLG_SLANG_EDGE_ELSE = 9,
  LLG_SLANG_EDGE_BODY = 10,
  LLG_SLANG_EDGE_OPERAND = 11,
  LLG_SLANG_EDGE_INDEX = 12,
  LLG_SLANG_EDGE_LEFT = 13,
  LLG_SLANG_EDGE_RIGHT = 14,
  LLG_SLANG_EDGE_BASE = 15,
  LLG_SLANG_EDGE_WIDTH = 16,
  LLG_SLANG_EDGE_DELAY = 17,
  LLG_SLANG_EDGE_EVENT = 18,
  LLG_SLANG_EDGE_ARGUMENT = 19,
  LLG_SLANG_EDGE_RECEIVER = 20,
  LLG_SLANG_EDGE_CALLEE = 21,
  LLG_SLANG_EDGE_ACTUAL = 22,
  LLG_SLANG_EDGE_DEFAULT_VALUE = 23,
  LLG_SLANG_EDGE_CASE_ITEM = 24,
  LLG_SLANG_EDGE_CASE_EXPRESSION = 25,
  LLG_SLANG_EDGE_BRANCH = 26,
  LLG_SLANG_EDGE_INCREMENT = 27,
  LLG_SLANG_EDGE_DECLARATION = 28,
  LLG_SLANG_EDGE_REFERENCE = 29,
  /* Exact equivalence of cloned symbols originating from one Slang syntax
   * declaration. Non-representatives target the lowest semantic node ID. */
  LLG_SLANG_EDGE_SOURCE_IDENTITY = 30,
  /* Synthesized function return storage targets its owning subroutine. */
  LLG_SLANG_EDGE_RETURN_OWNER = 31,
  /* A net alias declaration's ordered net-reference expression. */
  LLG_SLANG_EDGE_ALIAS_NET = 32,
  /* Concurrent assertion property structure. */
  LLG_SLANG_EDGE_PROPERTY_SPEC = 33,
  LLG_SLANG_EDGE_CLOCKING = 34,
  /* Assertion-instance formal/actual pairs use the same positional index. */
  LLG_SLANG_EDGE_ASSERTION_FORMAL = 35,
  LLG_SLANG_EDGE_ASSERTION_ACTUAL = 36,
  /* Implicit/extends-clause base-constructor invocation owned by a class. */
  LLG_SLANG_EDGE_BASE_CONSTRUCTOR = 37
};

typedef struct {
  uint64_t id;
  uint64_t parent_id;
  uint32_t kind;
  uint32_t subkind;
  uint32_t operation;
  uint32_t flags;
  LlgSlangString name;
  LlgSlangString detail;
  LlgSlangString definition_name;
  LlgSlangSourceRange range;
  uint64_t type_id;
  uint64_t constant_id;
  uint64_t target_id;
  uint64_t edge_start;
  uint64_t edge_count;
  /* A zero unit means this node has no applicable time scale. Magnitudes are
   * exactly 1, 10, or 100 when the corresponding unit is nonzero. */
  uint32_t time_unit;
  uint32_t time_unit_magnitude;
  uint32_t time_precision_unit;
  uint32_t time_precision_magnitude;
  uint32_t strength0;
  uint32_t strength1;
  /* Kind-specific repository-owned scalar metadata. Streaming concatenations
   * store their exact Slang slice size here (zero denotes left streaming).
   * Variable and NamedEvent nodes store an
   * LLG_SLANG_VARIABLE_LIFETIME_* value. Conditional/case statements store
   * an LLG_SLANG_UNIQUE_PRIORITY_* qualifier. */
  uint64_t auxiliary;
} LlgSlangSemanticNode;

typedef struct {
  uint32_t role;
  uint32_t index;
  uint64_t target_id;
} LlgSlangSemanticEdge;

enum {
  LLG_SLANG_LEXICAL_UNKNOWN = 0,
  LLG_SLANG_LEXICAL_MODULE = 1,
  LLG_SLANG_LEXICAL_INTERFACE = 2,
  LLG_SLANG_LEXICAL_PROGRAM = 3,
  LLG_SLANG_LEXICAL_PACKAGE = 4,
  LLG_SLANG_LEXICAL_CLASS = 5,
  LLG_SLANG_LEXICAL_STRUCT = 6,
  LLG_SLANG_LEXICAL_UNION = 7,
  LLG_SLANG_LEXICAL_ENUM = 8,
  LLG_SLANG_LEXICAL_ENUM_MEMBER = 9,
  LLG_SLANG_LEXICAL_TYPE_ALIAS = 10,
  LLG_SLANG_LEXICAL_PARAMETER = 11,
  LLG_SLANG_LEXICAL_PORT = 12,
  LLG_SLANG_LEXICAL_VARIABLE = 13,
  LLG_SLANG_LEXICAL_NET = 14,
  LLG_SLANG_LEXICAL_FUNCTION = 15,
  LLG_SLANG_LEXICAL_TASK = 16,
  LLG_SLANG_LEXICAL_METHOD = 17,
  LLG_SLANG_LEXICAL_MACRO = 18,
  LLG_SLANG_LEXICAL_KEYWORD = 19,
  LLG_SLANG_LEXICAL_STRING = 20,
  LLG_SLANG_LEXICAL_NUMBER = 21,
  LLG_SLANG_LEXICAL_OPERATOR = 22,
  LLG_SLANG_LEXICAL_IDENTIFIER = 23,
  LLG_SLANG_LEXICAL_GENVAR = 24
};

enum {
  LLG_SLANG_LEXICAL_ROLE_NONE = 0,
  LLG_SLANG_LEXICAL_ROLE_DECLARATION = 1,
  LLG_SLANG_LEXICAL_ROLE_REFERENCE = 2,
  LLG_SLANG_LEXICAL_ROLE_CONNECTION_LABEL = 3,
  LLG_SLANG_LEXICAL_ROLE_KEYWORD = 4,
  LLG_SLANG_LEXICAL_ROLE_CONNECTION_ACTUAL = 5
};

enum {
  LLG_SLANG_LEXICAL_MISSING = 1u << 0,
  LLG_SLANG_LEXICAL_SKIPPED = 1u << 1,
  LLG_SLANG_LEXICAL_MACRO_EXPANSION = 1u << 2
};

typedef struct {
  LlgSlangSourceRange range;
  uint32_t kind;
  uint32_t role;
  uint32_t flags;
  uint32_t reserved;
  uint64_t semantic_id;
  LlgSlangString text;
} LlgSlangLexicalToken;

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
  const LlgSlangSemanticNode* semantic_nodes;
  uint64_t semantic_node_count;
  const LlgSlangSemanticEdge* semantic_edges;
  uint64_t semantic_edge_count;
  const LlgSlangLexicalToken* lexical_tokens;
  uint64_t lexical_token_count;
  const LlgSlangTypeRange* type_ranges;
  uint64_t type_range_count;
  const LlgSlangTypeMember* type_members;
  uint64_t type_member_count;
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
