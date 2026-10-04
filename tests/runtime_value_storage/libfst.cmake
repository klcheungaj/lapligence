# libfst sources for the waveform probes.
#
# The probes compile `vendor/libfst` directly, exactly as the generated
# runtime embeds it. The repository holds the pristine upstream snapshot and
# the root build script applies `patches/libfst` in place (Cargo builds do this
# before any test runs). A configure that did not go through Cargo therefore
# verifies that every patched file already has the applied digest of
# `patches/libfst/files.sha256` and stops with the fix otherwise (CI jobs that
# run no Rust build apply the patch with `git apply` first).

set(LLG_FST "${LLG_ROOT}/vendor/libfst")

file(STRINGS "${LLG_ROOT}/patches/libfst/files.sha256" llg_fst_manifest REGEX "^[^#]")
foreach(entry IN LISTS llg_fst_manifest)
  string(REGEX MATCH "^([^ ]+) ([0-9a-f]+) ([0-9a-f]+)$" matched "${entry}")
  if(NOT matched)
    message(FATAL_ERROR "Malformed libfst manifest line: ${entry}")
  endif()
  set(llg_fst_file "${CMAKE_MATCH_1}")
  set(llg_fst_clean "${CMAKE_MATCH_2}")
  set(llg_fst_applied "${CMAKE_MATCH_3}")
  file(SHA256 "${LLG_FST}/${llg_fst_file}" llg_fst_digest)
  if(NOT llg_fst_digest STREQUAL llg_fst_applied)
    if(llg_fst_digest STREQUAL llg_fst_clean)
      message(FATAL_ERROR "vendor/libfst/${llg_fst_file} is still the pristine "
        "upstream file. Run `cargo build` (the build script applies "
        "patches/libfst in place) or, without Rust, "
        "`git apply --whitespace=nowarn --directory=vendor/libfst patches/libfst/libfst-local-changes.patch` "
        "from the repository root before configuring the waveform probes.")
    endif()
    message(FATAL_ERROR "vendor/libfst/${llg_fst_file} matches neither the "
      "pristine nor the patched digest in patches/libfst/files.sha256; restore "
      "it with `git restore -- vendor/libfst` and run `cargo build`.")
  endif()
endforeach()
