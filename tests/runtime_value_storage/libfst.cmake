# libfst sources for the waveform probes.
#
# `vendor/libfst` holds the pristine upstream snapshot; the runtime embeds it
# with `patches/libfst` applied (rendered by the Rust build script). Cargo
# tests pass that rendering as LLG_FST_SOURCE_DIR. Standalone configures (for
# example validate.py without Rust) render it here with `git apply` and
# authenticate every patched file against the applied digests in
# `patches/libfst/files.sha256`, so both paths compile identical sources.

set(LLG_FST_SOURCE_DIR "" CACHE PATH
  "Directory with the patched libfst sources (default: render vendor/libfst here)")

function(llg_render_libfst root out_var)
  set(source "${root}/vendor/libfst")
  set(patches "${root}/patches/libfst")
  set(rendered "${CMAKE_CURRENT_BINARY_DIR}/libfst")
  file(REMOVE_RECURSE "${rendered}")
  file(MAKE_DIRECTORY "${rendered}")
  file(GLOB vendor_files LIST_DIRECTORIES false "${source}/*")
  file(COPY ${vendor_files} DESTINATION "${rendered}")
  find_package(Git QUIET)
  if(NOT GIT_FOUND)
    message(FATAL_ERROR "Rendering patches/libfst needs Git; install it or pass "
      "-DLLG_FST_SOURCE_DIR=<rendered libfst> (Cargo builds write OUT_DIR/libfst)")
  endif()
  file(GLOB patch_files "${patches}/*.patch")
  list(SORT patch_files)
  foreach(patch IN LISTS patch_files)
    # The ceiling keeps Git from treating the build tree as part of an
    # enclosing repository, so paths apply relative to the rendered copy.
    execute_process(
      COMMAND "${CMAKE_COMMAND}" -E env "GIT_CEILING_DIRECTORIES=${CMAKE_CURRENT_BINARY_DIR}"
              "${GIT_EXECUTABLE}" apply --whitespace=nowarn "${patch}"
      WORKING_DIRECTORY "${rendered}"
      RESULT_VARIABLE status
      ERROR_VARIABLE error)
    if(NOT status EQUAL 0)
      message(FATAL_ERROR "Applying ${patch} to vendor/libfst failed: ${error}"
        "Restore the pristine snapshot in vendor/libfst.")
    endif()
  endforeach()
  file(STRINGS "${patches}/files.sha256" manifest REGEX "^[^#]")
  foreach(entry IN LISTS manifest)
    string(REGEX MATCH "^([^ ]+) ([0-9a-f]+) ([0-9a-f]+)$" matched "${entry}")
    if(NOT matched)
      message(FATAL_ERROR "Malformed libfst manifest line: ${entry}")
    endif()
    set(file "${CMAKE_MATCH_1}")
    set(applied "${CMAKE_MATCH_3}")
    file(SHA256 "${rendered}/${file}" digest)
    if(NOT digest STREQUAL applied)
      message(FATAL_ERROR "Rendered libfst ${file} does not match its applied "
        "digest; vendor/libfst or patches/libfst is modified")
    endif()
  endforeach()
  set(${out_var} "${rendered}" PARENT_SCOPE)
endfunction()

if(LLG_FST_SOURCE_DIR)
  set(LLG_FST "${LLG_FST_SOURCE_DIR}")
else()
  llg_render_libfst("${LLG_ROOT}" LLG_FST)
endif()
