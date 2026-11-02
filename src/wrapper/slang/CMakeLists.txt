cmake_minimum_required(VERSION 3.20...3.31)

project(llg_slang_wrapper LANGUAGES CXX)

option(LLG_SLANG_MUSL "Build Slang with a musl C++ toolchain" OFF)
# AUTO tries link-time optimization and falls back to ordinary objects when the
# toolchain cannot produce LTO archives; ON requires it; OFF never uses it.
set(LLG_SLANG_LTO OFF CACHE STRING "Link-time optimization: OFF, AUTO or ON")
set_property(CACHE LLG_SLANG_LTO PROPERTY STRINGS OFF AUTO ON)
# Compile options only GCC LTO builds need on this target, chosen by build.rs
# (build_support/native_lto.rs `gnu_lto_compile_options`).
set(LLG_SLANG_GNU_LTO_OPTIONS "" CACHE STRING
    "Extra compile options for GCC LTO builds of the frontend")

if(LLG_SLANG_MUSL)
  if(NOT UNIX OR APPLE)
    message(FATAL_ERROR "LLG_SLANG_MUSL requires a Linux target")
  endif()
  # The wrapper and frontend are archives; Cargo performs the final static
  # executable link. Limit dependency discovery to static libraries so a
  # cross build cannot accidentally capture a host shared object.
  set(CMAKE_FIND_LIBRARY_SUFFIXES ".a")
endif()

if(NOT LLG_SLANG_SOURCE_DIR OR
   NOT EXISTS "${LLG_SLANG_SOURCE_DIR}/CMakeLists.txt")
  message(FATAL_ERROR "LLG_SLANG_SOURCE_DIR must name a Slang source checkout")
endif()

# Keep the frontend build limited to the library consumed by the C ABI shim.
# Installation is enabled so Cargo can link archives from one stable prefix.
set(SLANG_MASTER_PROJECT OFF CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_TOOLS OFF CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_TESTS OFF CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_DOCS OFF CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_PYLIB OFF CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_PYTHON_DOCS OFF CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_INSTALL ON CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_COVERAGE OFF CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_THREADTEST OFF CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_UVM_TEST OFF CACHE BOOL "" FORCE)
set(SLANG_INCLUDE_BENCHMARKS OFF CACHE BOOL "" FORCE)
set(SLANG_USE_MIMALLOC OFF CACHE BOOL "" FORCE)
set(SLANG_USE_CPPTRACE OFF CACHE BOOL "" FORCE)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)

# Link-time optimization of Slang, fmt and the wrapper. CMake's IPO flags are
# GCC `-flto=auto -fno-fat-lto-objects` (GCC has no ThinLTO; this is its
# parallel partitioned mode), Clang/AppleClang `-flto=thin` and MSVC `/GL`
# (archives with `/LTCG`). The archives then hold only IR, so the final link,
# which rustc's linker performs, must run the matching LTO backend: build.rs
# reads the mode recorded below and adds the link arguments it needs.
set(llg_slang_lto_mode "off")
if(NOT LLG_SLANG_LTO STREQUAL "OFF")
  include(CheckIPOSupported)
  check_ipo_supported(RESULT llg_ipo_supported OUTPUT llg_ipo_output
                      LANGUAGES CXX)
  if(llg_ipo_supported)
    set(CMAKE_INTERPROCEDURAL_OPTIMIZATION ON)
    set(llg_slang_lto_mode "${CMAKE_CXX_COMPILER_ID}")
    # Directory options reach Slang, fmt (fetched inside Slang's tree) and the
    # wrapper, which are all added below.
    if(CMAKE_CXX_COMPILER_ID STREQUAL "GNU" AND LLG_SLANG_GNU_LTO_OPTIONS)
      add_compile_options(${LLG_SLANG_GNU_LTO_OPTIONS})
    endif()
  elseif(LLG_SLANG_LTO STREQUAL "ON")
    message(FATAL_ERROR
      "LLG_SLANG_LTO=ON but ${CMAKE_CXX_COMPILER_ID} IPO is unavailable: "
      "${llg_ipo_output}")
  else()
    message(WARNING
      "Building Slang without LTO; ${CMAKE_CXX_COMPILER_ID} IPO is "
      "unavailable: ${llg_ipo_output}")
  endif()
endif()
file(WRITE "${CMAKE_BINARY_DIR}/llg_slang_lto.txt" "${llg_slang_lto_mode}\n")

# Slang v11 normally fetches fmt with Git. The release archive is equivalent,
# smaller, and works where only GitHub's codeload endpoint is admitted.
# FetchContent keeps the first declaration of a dependency.
include(FetchContent)
if(CMAKE_VERSION VERSION_GREATER_EQUAL 3.24)
  set(fmt_timestamp_option DOWNLOAD_EXTRACT_TIMESTAMP FALSE)
endif()
FetchContent_Declare(
  fmt
  URL https://codeload.github.com/fmtlib/fmt/tar.gz/refs/tags/12.1.0
  URL_HASH
    SHA256=ea7de4299689e12b6dddd392f9896f08fb0777ac7168897a244a6d6085043fea
  ${fmt_timestamp_option})

add_subdirectory("${LLG_SLANG_SOURCE_DIR}" slang)

add_library(llg_slang_wrapper STATIC ../slang_c_api.cpp ../slang_platform.cpp)
target_include_directories(llg_slang_wrapper PUBLIC ..)
target_link_libraries(llg_slang_wrapper PRIVATE slang::slang)
target_compile_features(llg_slang_wrapper PRIVATE cxx_std_20)

install(TARGETS llg_slang_wrapper ARCHIVE DESTINATION lib)
