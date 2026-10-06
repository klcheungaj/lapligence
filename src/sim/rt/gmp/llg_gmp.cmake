# Bundled GMP (vendor/gmp, GMP 6.3.0) for compact GMP kernels.
#
# Builds the generic-C mpn subset behind the five functions the kernels call
# (mpn_mul_n, mpn_mul_1, mpn_addmul_1, mpn_tdiv_qr, mpn_get_str) as the OBJECT
# library `llg_gmp`, without GMP's autotools build. Consumers add
# $<TARGET_OBJECTS:llg_gmp> to their library and LLG_GMP_INCLUDE_DIRS to the
# translation units that include <gmp.h>.
#
# Inputs:
#   LLG_GMP_SOURCE_DIR  GMP sources (the vendor/gmp tree, or the copy llg
#                       writes next to a generated model as gmp/).
#   LLG_GMP_TABLE_DIR   Tables generated for 64-bit limbs without nails
#                       (src/sim/rt/gmp/generated, see scripts/gmp_tables.py).
# Output:
#   llg_gmp, LLG_GMP_INCLUDE_DIRS
include_guard(GLOBAL)

if(NOT LLG_GMP_SOURCE_DIR OR NOT EXISTS "${LLG_GMP_SOURCE_DIR}/gmp-h.in")
  message(FATAL_ERROR "LLG_GMP_SOURCE_DIR must name the bundled GMP sources")
endif()
if(NOT LLG_GMP_TABLE_DIR OR NOT EXISTS "${LLG_GMP_TABLE_DIR}/mp_bases.h")
  message(FATAL_ERROR "LLG_GMP_TABLE_DIR must name the generated GMP tables")
endif()

include(CheckTypeSize)
check_type_size("void*" LLG_GMP_SIZEOF_VOID_P LANGUAGE C)
check_type_size("unsigned short" LLG_GMP_SIZEOF_UNSIGNED_SHORT LANGUAGE C)
check_type_size("unsigned int" LLG_GMP_SIZEOF_UNSIGNED LANGUAGE C)
check_type_size("unsigned long" LLG_GMP_SIZEOF_UNSIGNED_LONG LANGUAGE C)
check_type_size("unsigned long long" LLG_GMP_SIZEOF_UNSIGNED_LONG_LONG LANGUAGE C)
if(NOT LLG_GMP_SIZEOF_VOID_P EQUAL 8 OR NOT LLG_GMP_SIZEOF_UNSIGNED_LONG_LONG EQUAL 8)
  message(FATAL_ERROR "compact GMP kernels require a 64-bit target")
endif()
if(CMAKE_C_BYTE_ORDER STREQUAL "BIG_ENDIAN")
  message(FATAL_ERROR "compact GMP kernels require a little-endian target")
endif()

# 64-bit limbs without nails: `unsigned long` where it has 64 bits (LP64),
# otherwise `unsigned long long` (LLP64, Windows), as GMP's configure selects.
set(GMP_LIMB_BITS 64)
set(GMP_NAIL_BITS 0)
set(LIBGMP_DLL 0)
set(CC "${CMAKE_C_COMPILER_ID}")
set(CFLAGS "")
if(LLG_GMP_SIZEOF_UNSIGNED_LONG EQUAL 8)
  set(DEFN_LONG_LONG_LIMB "/* #undef _LONG_LONG_LIMB */")
else()
  set(DEFN_LONG_LONG_LIMB "#define _LONG_LONG_LIMB 1")
endif()
set(LLG_GMP_GENERATED_DIR "${CMAKE_CURRENT_BINARY_DIR}/llg_gmp")
configure_file("${LLG_GMP_SOURCE_DIR}/gmp-h.in" "${LLG_GMP_GENERATED_DIR}/gmp.h" @ONLY)
configure_file("${LLG_GMP_SOURCE_DIR}/mpn/generic/gmp-mparam.h"
  "${LLG_GMP_GENERATED_DIR}/gmp-mparam.h" COPYONLY)
file(WRITE "${LLG_GMP_GENERATED_DIR}/config.h.new" "/* Generated for the bundled GMP subset. */
#define HAVE_INTMAX_T 1
#define HAVE_INTPTR_T 1
#define HAVE_INTTYPES_H 1
#define HAVE_LIMB_LITTLE_ENDIAN 1
#define HAVE_DOUBLE_IEEE_LITTLE_ENDIAN 1
#define HAVE_LONG_LONG 1
#define HAVE_PTRDIFF_T 1
#define HAVE_STDINT_H 1
#define HAVE_STDLIB_H 1
#define HAVE_STRING_H 1
#define HAVE_UINT_LEAST32_T 1
#define SIZEOF_MP_LIMB_T 8
#define SIZEOF_UNSIGNED ${LLG_GMP_SIZEOF_UNSIGNED}
#define SIZEOF_UNSIGNED_LONG ${LLG_GMP_SIZEOF_UNSIGNED_LONG}
#define SIZEOF_UNSIGNED_SHORT ${LLG_GMP_SIZEOF_UNSIGNED_SHORT}
#define SIZEOF_VOID_P 8
#define TUNE_SQR_TOOM2_MAX SQR_TOOM2_MAX_GENERIC
#define WANT_FFT 1
#define WANT_TMP_ALLOCA 1
")
# Rewrite only on change so a reconfigure does not rebuild the subset.
configure_file("${LLG_GMP_GENERATED_DIR}/config.h.new" "${LLG_GMP_GENERATED_DIR}/config.h" COPYONLY)

# The link closure of the five kernel entry points in GMP's generic-C build,
# including the out-of-line copies of gmp.h's inline mpn functions (add,
# add_1, cmp, neg, sub, sub_1, zero_p) and the paths -O0 builds keep
# (toom4_sqr). Recompute it when the submodule moves: the compact GMP storage
# probes (Debug) and generated models (Release) fail to link when it is
# incomplete.
set(LLG_GMP_MPN_SOURCES
  add add_1 add_n addmul_1 bdiv_dbm1c bdiv_q_1 cmp com compute_powtab
  dcpi1_div_qr dive_1 divrem_1 divrem_2 get_str invertappr lshift lshiftc
  mod_34lsub1 mu_div_qr mul mul_1 mul_basecase mul_fft mul_n mulmod_bknp1
  mulmod_bnm1 neg nussbaumer_mul pre_divrem_1 rshift sbpi1_div_qr
  sbpi1_divappr_q sqr sqr_basecase sqrmod_bnm1 sub sub_1 sub_n submul_1
  tdiv_qr toom22_mul toom2_sqr toom32_mul toom33_mul toom3_sqr toom42_mul
  toom43_mul toom44_mul toom4_sqr toom53_mul toom63_mul toom6_sqr toom6h_mul
  toom8_sqr toom8h_mul toom_couple_handling toom_eval_dgr3_pm1
  toom_eval_dgr3_pm2 toom_eval_pm1 toom_eval_pm2 toom_eval_pm2exp
  toom_eval_pm2rexp toom_interpolate_12pts toom_interpolate_16pts
  toom_interpolate_5pts toom_interpolate_6pts toom_interpolate_7pts
  toom_interpolate_8pts zero_p)
set(LLG_GMP_SOURCES
  "${LLG_GMP_SOURCE_DIR}/assert.c" "${LLG_GMP_SOURCE_DIR}/errno.c"
  "${LLG_GMP_SOURCE_DIR}/memory.c" "${LLG_GMP_SOURCE_DIR}/mp_clz_tab.c"
  "${LLG_GMP_SOURCE_DIR}/mp_minv_tab.c" "${LLG_GMP_SOURCE_DIR}/tal-reent.c"
  "${LLG_GMP_TABLE_DIR}/mp_bases.c")
foreach(name IN LISTS LLG_GMP_MPN_SOURCES)
  set(source "${LLG_GMP_SOURCE_DIR}/mpn/generic/${name}.c")
  list(APPEND LLG_GMP_SOURCES "${source}")
  # GMP compiles each mpn source with its operation name defined.
  set_source_files_properties("${source}" PROPERTIES COMPILE_DEFINITIONS "OPERATION_${name}")
endforeach()

set(LLG_GMP_INCLUDE_DIRS "${LLG_GMP_GENERATED_DIR}")
add_library(llg_gmp OBJECT ${LLG_GMP_SOURCES})
target_include_directories(llg_gmp PRIVATE "${LLG_GMP_GENERATED_DIR}" "${LLG_GMP_TABLE_DIR}"
  "${LLG_GMP_SOURCE_DIR}" "${LLG_GMP_SOURCE_DIR}/mpn")
target_compile_definitions(llg_gmp PRIVATE __GMP_WITHIN_GMP)
set_property(TARGET llg_gmp PROPERTY POSITION_INDEPENDENT_CODE ON)
# Third-party code: keep the project's warning flags from flooding its output.
if(MSVC)
  target_compile_options(llg_gmp PRIVATE /w)
else()
  target_compile_options(llg_gmp PRIVATE -w)
endif()
