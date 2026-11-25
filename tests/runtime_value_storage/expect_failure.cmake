# Below the CTest limit of the tests that run this, which prints no reason.
set(limit 60)
execute_process(
  COMMAND "${PROBE}" "${MODE}"
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
