/*
 Copyright 2019 Alain Dargelas

 Licensed under the Apache License, Version 2.0 (the "License");
 you may not use this file except in compliance with the License.
 You may obtain a copy of the License at

 http://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing, software
 distributed under the License is distributed on an "AS IS" BASIS,
 WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 See the License for the specific language governing permissions and
 limitations under the License.
 */

/*
 * File:   surelog_c_api.h
 *
 * C wrapper API for the Surelog C++ library.
 * Provides a C-linkage interface suitable for use from Rust via FFI.
 */

#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque handle types */
typedef void* SL_SymbolTable;
typedef void* SL_ErrorContainer;
typedef void* SL_CommandLineParser;
typedef void* SL_Compiler;
typedef void* SL_Design;
typedef void* SL_ModuleInstance;

/* ─── SymbolTable ─────────────────────────────────────────────────────────── */
SL_SymbolTable sl_create_symbol_table(void);
void           sl_free_symbol_table(SL_SymbolTable st);

/* ─── ErrorContainer ──────────────────────────────────────────────────────── */
SL_ErrorContainer sl_create_error_container(SL_SymbolTable st);
void              sl_free_error_container(SL_ErrorContainer ec);
void              sl_errors_print_messages(SL_ErrorContainer ec, int mute);
uint32_t          sl_errors_get_fatal_count(SL_ErrorContainer ec);
uint32_t          sl_errors_get_syntax_count(SL_ErrorContainer ec);
uint32_t          sl_errors_get_error_count(SL_ErrorContainer ec);

/*
 * One structured diagnostic, copied out of the container.  `file` and
 * `message` are malloc'd strings that the caller must free with
 * sl_free_string(); all other fields are plain scalars.
 *
 *   severity – ErrorDefinition::ErrorSeverity ordinal:
 *              0=FATAL 1=SYNTAX 2=ERROR 3=WARNING 4=INFO 5=NOTE
 *   file     – absolute source path, empty string when unknown
 *   line     – 1-based, 0 when unknown
 *   col      – 1-based, 0 when unknown
 *   message  – message text with the first "%s" replaced by the object name
 */
typedef struct {
    int      severity;
    char*    file;
    uint32_t line;
    uint32_t col;
    char*    message;
} SL_Diag;

/* Total number of errors currently held by the container (after dedup). */
uint32_t sl_errors_get_count(SL_ErrorContainer ec);

/*
 * Copy diagnostic `index` into *out.  Returns 1 on success, 0 when `index`
 * is out of range or when the entry has no registered message template
 * (such entries are skipped by the caller).
 */
int      sl_errors_get_item(SL_ErrorContainer ec, uint32_t index,
                            SL_Diag* out);

/* ─── CommandLineParser ───────────────────────────────────────────────────── */
SL_CommandLineParser sl_create_command_line_parser(SL_ErrorContainer ec,
                                                    SL_SymbolTable st);
void sl_free_command_line_parser(SL_CommandLineParser clp);
void sl_clp_no_python(SL_CommandLineParser clp);
int  sl_clp_parse_command_line(SL_CommandLineParser clp, int argc,
                                const char** argv);
int  sl_clp_help(SL_CommandLineParser clp);
int  sl_clp_mute_stdout(SL_CommandLineParser clp);
void sl_clp_set_mute_stdout(SL_CommandLineParser clp);

/* ─── Compiler ────────────────────────────────────────────────────────────── */
SL_Compiler sl_start_compiler(SL_CommandLineParser clp);
SL_Design   sl_get_design(SL_Compiler compiler);
void        sl_shutdown_compiler(SL_Compiler compiler);

/* ─── Design ──────────────────────────────────────────────────────────────── */
uint32_t          sl_design_get_top_instance_count(SL_Design design);
SL_ModuleInstance sl_design_get_top_instance(SL_Design design, uint32_t i);

/* ─── FileContent (parse tree / token stream) ────────────────────────────── */
typedef void* SL_FileContent;

/*
 * A single parse-tree node.  All pointer fields are valid for the lifetime of
 * the compiler (i.e. until sl_shutdown_compiler is called).  Do NOT free them.
 *
 *   type_name   – points into a static string-literal table (never changes)
 *   symbol_name – points into the SymbolTable intern pool; NULL for unnamed
 *                 grammar-rule nodes (non-leaf positions in the tree)
 *   file_id     – raw PathId of the originating source file; compare against
 *                 sl_file_content_get_file_id() to detect `include-d nodes
 *   child_index / sibling_index – parse-tree links; zero is
 *                 Surelog's InvalidNodeId
 */
typedef struct {
    uint32_t    line;
    uint16_t    col;
    /* 2 bytes implicit padding (compiler-inserted before end_line) */
    uint32_t    end_line;
    uint16_t    end_col;
    uint16_t    type_id;      /* raw VObjectType discriminant */
    uint32_t    file_id;      /* RawPathId – originating file */
    uint32_t    parent_index; /* NodeId of parent in parse tree; 0 = no parent */
    uint32_t    child_index;  /* first child NodeId; 0 = no child */
    uint32_t    sibling_index;/* next sibling NodeId; 0 = no sibling */
    const char* type_name;    /* static – do NOT free */
    const char* symbol_name;  /* SymbolTable intern – do NOT free; may be NULL */
} SL_VObjectInfo;

/* Number of parsed FileContent objects in the design. */
uint32_t       sl_design_get_file_content_count(SL_Design design);

/* Return the i-th FileContent handle, or NULL if i is out of range. */
SL_FileContent sl_design_get_file_content(SL_Design design, uint32_t i);

/* File-system path for a FileContent.  Caller must free with sl_free_string. */
char*          sl_file_content_get_path(SL_FileContent fc);

/* Raw PathId for the primary file backing this FileContent. */
uint32_t       sl_file_content_get_file_id(SL_FileContent fc);

/* Total number of parse-tree nodes stored in this FileContent. */
uint32_t       sl_file_content_get_node_count(SL_FileContent fc);

/*
 * Fill *out with data for the node at position `index`.
 * Returns 1 on success, 0 if index is out of range.
 */
int            sl_file_content_get_node(SL_FileContent fc, uint32_t index,
                                        SL_VObjectInfo* out);

/* ─── ModuleInstance ──────────────────────────────────────────────────────── */

/* Returned strings must be freed with sl_free_string(). */
char*   sl_instance_get_full_path_name(SL_ModuleInstance inst);
char*   sl_instance_get_file_path(SL_ModuleInstance inst);
uint32_t sl_instance_get_child_count(SL_ModuleInstance inst);
SL_ModuleInstance sl_instance_get_child(SL_ModuleInstance inst, uint32_t i);

/* ─── Memory ──────────────────────────────────────────────────────────────── */
void sl_free_string(char* s);

/* ─── CommandLineParser extras ───────────────────────────────────────────────
 * These setters are needed to replicate the hellouhdm compile flow:
 *   clp->setParse(true); clp->setwritePpOutput(true);
 *   clp->setCompile(true); clp->setElaborate(true);
 *   clp->setElabUhdm(true);   // UHDM full elaboration (ref binding)
 */
void sl_clp_set_parse(SL_CommandLineParser clp);
void sl_clp_set_write_pp_output(SL_CommandLineParser clp);
void sl_clp_set_compile(SL_CommandLineParser clp);
void sl_clp_set_elaborate(SL_CommandLineParser clp);
void sl_clp_set_elab_uhdm(SL_CommandLineParser clp);

/* ─── UHDM design ─────────────────────────────────────────────────────────── */
/*
 * Returns the UHDM design as a vpiHandle (opaque void*).  The handle is
 * owned by the compiler and must NOT be freed independently.
 */
void* sl_get_uhdm_design(SL_Compiler compiler);

/*
 * Elaborate the UHDM design in-place if it has not been elaborated yet.
 * Equivalent to the UHDM::ElaboratorContext block in hellouhdm.cpp.
 */
void sl_uhdm_elaborate(void* vpi_design);


#ifdef __cplusplus
}
#endif
