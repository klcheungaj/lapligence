cmake_minimum_required(VERSION 3.20...3.31)

project(llg_slang_wrapper LANGUAGES CXX)

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

add_library(llg_slang_wrapper STATIC ../slang_c_api.cpp)
target_include_directories(llg_slang_wrapper PUBLIC ..)
target_link_libraries(llg_slang_wrapper PRIVATE slang::slang)
target_compile_features(llg_slang_wrapper PRIVATE cxx_std_20)

install(TARGETS llg_slang_wrapper ARCHIVE DESTINATION lib)
