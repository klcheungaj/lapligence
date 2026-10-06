# GMP for compact GMP kernels: by default the bundled vendor/gmp subset, built
# with the recipe generated models use; LLG_GMP_ROOT selects an installation
# instead. Sets LLG_STORAGE_GMP_INCLUDE (for units including <gmp.h>) and
# LLG_STORAGE_GMP_LIBRARY (to link).
include_guard(GLOBAL)
set(LLG_GMP_ROOT "" CACHE PATH "Optional GMP installation prefix (default: bundled vendor/gmp)")
if(LLG_GMP_ROOT)
  find_path(llg_storage_gmp_include gmp.h PATHS "${LLG_GMP_ROOT}/include" NO_DEFAULT_PATH)
  find_library(llg_storage_gmp_library gmp PATHS "${LLG_GMP_ROOT}/lib" NO_DEFAULT_PATH)
  if(NOT llg_storage_gmp_include OR NOT llg_storage_gmp_library)
    message(FATAL_ERROR "LLG_GMP_ROOT must contain matching include/gmp.h and lib/libgmp")
  endif()
  set(LLG_STORAGE_GMP_INCLUDE "${llg_storage_gmp_include}")
  set(LLG_STORAGE_GMP_LIBRARY "${llg_storage_gmp_library}")
else()
  set(LLG_GMP_SOURCE_DIR "${LLG_ROOT}/vendor/gmp")
  set(LLG_GMP_TABLE_DIR "${LLG_RT}/gmp/generated")
  include("${LLG_RT}/gmp/llg_gmp.cmake")
  add_library(llg_storage_gmp STATIC $<TARGET_OBJECTS:llg_gmp>)
  set(LLG_STORAGE_GMP_INCLUDE ${LLG_GMP_INCLUDE_DIRS})
  set(LLG_STORAGE_GMP_LIBRARY llg_storage_gmp)
endif()
