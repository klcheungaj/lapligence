# A core dump handler (a piped core_pattern) can hold an aborting process for
# long on a busy host, and a core limit of 0 does not stop one: the kernel
# skips it for a limit of 1 (do_coredump).
set(launcher)
if(CMAKE_HOST_SYSTEM_NAME STREQUAL "Linux")
  find_program(prlimit prlimit)
  if(prlimit)
    set(launcher "${prlimit}" --core=1:1)
  endif()
endif()
# Below the CTest limit of the tests that run this, which prints no reason.
set(limit 60)
execute_process(
  COMMAND ${launcher} "${PROBE}" "${MODE}"
  RESULT_VARIABLE result
  OUTPUT_VARIABLE output
  ERROR_VARIABLE diagnostic
  TIMEOUT ${limit}
)
if("${result}" MATCHES "[Tt]imeout|[Tt]imed out")
  message(FATAL_ERROR
    "${MODE}: the probe did not end within ${limit} s; it printed: ${output}${diagnostic}")
endif()
if("${result}" STREQUAL "0")
  message(FATAL_ERROR "${MODE} unexpectedly succeeded: ${output}")
endif()
string(FIND "${diagnostic}" "${EXPECTED}" found)
if(found EQUAL -1)
  message(FATAL_ERROR "${MODE}: wrong failure (${result}): ${diagnostic}")
endif()
