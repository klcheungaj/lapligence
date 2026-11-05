# The production facade client facade_compare_bridge.c, built unchanged against
# each backend as a generated model selects it (llg_value.h only).
if(NOT TARGET facade_compare_legacy)
  add_executable(facade_compare_legacy facade_compare_bridge.c)
  target_link_libraries(facade_compare_legacy PRIVATE compact_legacy)
  strict_c(facade_compare_legacy)
  add_custom_target(facade_compare_all)
endif()
set(facade_compare_kernels 0)
if(mode STREQUAL "gmp")
  set(facade_compare_kernels 1)
endif()
add_executable(facade_compare_${mode} facade_compare_bridge.c facade_compare_selected.c)
target_compile_definitions(facade_compare_${mode} PRIVATE
  LLG_SV4_USE_GMP=1 LLG_SV4_GMP_KERNELS=${facade_compare_kernels})
target_include_directories(facade_compare_${mode} PRIVATE "${LLG_RT}")
target_link_libraries(facade_compare_${mode} PRIVATE compact_${mode})
strict_c(facade_compare_${mode})
add_dependencies(facade_compare_all facade_compare_legacy facade_compare_${mode})
add_test(NAME facade_compare_${mode} COMMAND "${Python3_EXECUTABLE}"
  "${CMAKE_CURRENT_SOURCE_DIR}/facade_compare.py" "${CMAKE_CURRENT_SOURCE_DIR}/facade_comparisons.tsv"
  "$<TARGET_FILE:facade_compare_legacy>" "$<TARGET_FILE:facade_compare_${mode}>")
set_tests_properties(facade_compare_${mode} PROPERTIES TIMEOUT 300)
