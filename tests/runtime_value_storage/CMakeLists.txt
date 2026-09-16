cmake_minimum_required(VERSION 3.16)
project(llg_dynamic_storage_tests C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
include(CTest)

option(LLG_STORAGE_TEST_WAVEFORMS "Test the waveform snapshot ownership pilot" ON)
option(LLG_STORAGE_TEST_SANITIZERS "Enable AddressSanitizer and UndefinedBehaviorSanitizer" OFF)
set(LLG_STORAGE_TEST_MODEL_WIDTH 1024 CACHE STRING "Legacy source-value capacity for the waveform probe")
get_filename_component(LLG_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(LLG_RT "${LLG_ROOT}/src/sim/rt")

if(LLG_STORAGE_TEST_SANITIZERS)
  if(MSVC OR NOT CMAKE_C_COMPILER_ID MATCHES "GNU|Clang")
    message(FATAL_ERROR "This sanitizer option requires a GCC/Clang-style compiler driver")
  endif()
  add_compile_options(-fsanitize=address,undefined -fno-omit-frame-pointer)
  add_link_options(-fsanitize=address,undefined)
endif()

function(strict_c target)
  if(MSVC)
    target_compile_definitions(${target} PRIVATE _CRT_SECURE_NO_WARNINGS)
    target_compile_options(${target} PRIVATE /W4 /WX)
  else()
    target_compile_options(${target} PRIVATE -Wall -Wextra -Werror -pedantic-errors)
  endif()
endfunction()

add_executable(sv4_storage_probe storage_probe.c)
target_include_directories(sv4_storage_probe PRIVATE "${LLG_RT}")
# Prove that dynamic storage can exceed the otherwise one-bit legacy layout.
target_compile_definitions(sv4_storage_probe PRIVATE LLG_MODEL_MAX_WIDTH=1)
strict_c(sv4_storage_probe)
add_test(NAME storage_lifecycle COMMAND sv4_storage_probe)
foreach(mode IN ITEMS limit uint32-max oom oom-copy)
  if(mode MATCHES "oom")
    set(expected "packed storage allocation failed")
  else()
    set(expected "packed storage width")
  endif()
  add_test(NAME storage_reject_${mode}
    COMMAND "${CMAKE_COMMAND}"
      "-DPROBE=$<TARGET_FILE:sv4_storage_probe>" "-DMODE=${mode}"
      "-DEXPECTED=${expected}" -P "${CMAKE_CURRENT_SOURCE_DIR}/expect_failure.cmake")
endforeach()

if(LLG_STORAGE_TEST_WAVEFORMS)
  find_package(Threads REQUIRED)
  find_package(ZLIB REQUIRED)
  add_library(storage_probe_fst STATIC
    "${LLG_RT}/gtkwave/fstapi.c"
    "${LLG_RT}/gtkwave/fastlz.c"
    "${LLG_RT}/gtkwave/lz4.c")
  target_include_directories(storage_probe_fst PUBLIC "${LLG_RT}/gtkwave")
  target_compile_definitions(storage_probe_fst PRIVATE FST_CONFIG_INCLUDE="fst_config.h")
  target_link_libraries(storage_probe_fst PUBLIC ZLIB::ZLIB Threads::Threads)
  if(NOT MSVC)
    target_link_libraries(storage_probe_fst PUBLIC m)
  endif()
  add_executable(sv4_waveform_probe waveform_snapshot_probe.c storage_runtime.c)
  target_include_directories(sv4_waveform_probe PRIVATE "${LLG_RT}")
  target_compile_definitions(sv4_waveform_probe PRIVATE
    LLG_MODEL_MAX_WIDTH=${LLG_STORAGE_TEST_MODEL_WIDTH})
  target_link_libraries(sv4_waveform_probe PRIVATE storage_probe_fst)
  strict_c(sv4_waveform_probe)
  file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/waveform-output")
  add_test(NAME waveform_snapshot_lifecycle COMMAND sv4_waveform_probe)
  set_tests_properties(waveform_snapshot_lifecycle PROPERTIES
    WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/waveform-output")
endif()

get_property(all_tests DIRECTORY PROPERTY TESTS)
set_tests_properties(${all_tests} PROPERTIES TIMEOUT 60)
