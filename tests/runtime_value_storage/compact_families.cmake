# V05/S2 and S3 probes are separate from G1's core harness.
add_executable(compact_${mode}_families_probe compact_families_probe.c)
target_link_libraries(compact_${mode}_families_probe PRIVATE compact_${mode} compact_legacy)
strict_c(compact_${mode}_families_probe)
foreach(case IN ITEMS exhaustive ranges wide)
  add_test(NAME compact_${mode}_families_${case} COMMAND compact_${mode}_families_probe ${case})
  set_tests_properties(compact_${mode}_families_${case} PROPERTIES TIMEOUT 600)
endforeach()
if(NOT TARGET compact_families_legacy)
  add_executable(compact_families_legacy compact_families_bridge.c)
  target_link_libraries(compact_families_legacy PRIVATE compact_legacy)
  strict_c(compact_families_legacy)
endif()
add_executable(compact_families_${mode} compact_families_bridge.c)
target_compile_definitions(compact_families_${mode} PRIVATE LLG_ORACLE_COMPACT=1)
target_link_libraries(compact_families_${mode} PRIVATE compact_${mode})
strict_c(compact_families_${mode})
add_test(NAME compact_${mode}_families_python COMMAND "${Python3_EXECUTABLE}"
  "${CMAKE_CURRENT_SOURCE_DIR}/compact_families_oracle.py"
  "$<TARGET_FILE:compact_families_${mode}>" "$<TARGET_FILE:compact_families_legacy>")
set_tests_properties(compact_${mode}_families_python PROPERTIES TIMEOUT 600)
add_executable(compact_${mode}_families_benchmark compact_families_benchmark.c)
target_link_libraries(compact_${mode}_families_benchmark PRIVATE compact_${mode} compact_legacy)
strict_c(compact_${mode}_families_benchmark)
add_test(NAME compact_${mode}_families_benchmark_smoke
  COMMAND compact_${mode}_families_benchmark --smoke)
set_tests_properties(compact_${mode}_families_benchmark_smoke PROPERTIES TIMEOUT 60)
if(UNIX AND NOT APPLE)
  add_executable(compact_${mode}_families_allocations compact_families_allocations.c)
  target_link_libraries(compact_${mode}_families_allocations PRIVATE compact_${mode})
  target_link_options(compact_${mode}_families_allocations PRIVATE
    -Wl,--wrap=malloc -Wl,--wrap=calloc -Wl,--wrap=realloc)
  strict_c(compact_${mode}_families_allocations)
  add_test(NAME compact_${mode}_families_allocations COMMAND compact_${mode}_families_allocations)
  set_tests_properties(compact_${mode}_families_allocations PROPERTIES TIMEOUT 60)
endif()
if(NOT TARGET compact_v05a_checks)
  add_custom_target(compact_v05a_checks)
endif()
add_dependencies(compact_v05a_checks
  compact_${mode}_families_probe compact_families_${mode} compact_families_legacy
  compact_${mode}_families_benchmark compact_${mode}_probe compact_oracle_${mode}
  compact_oracle_legacy compact_${mode}_benchmark)
if(UNIX AND NOT APPLE)
  add_dependencies(compact_v05a_checks
    compact_${mode}_families_allocations compact_${mode}_allocation_probe)
endif()
