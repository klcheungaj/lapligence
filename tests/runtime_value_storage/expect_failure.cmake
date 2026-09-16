execute_process(
  COMMAND "${PROBE}" "${MODE}"
  RESULT_VARIABLE result
  OUTPUT_VARIABLE output
  ERROR_VARIABLE diagnostic
  TIMEOUT 15
)
if("${result}" MATCHES "[Tt]imeout|[Tt]imed out")
  message(FATAL_ERROR "${MODE} timed out instead of terminating: ${diagnostic}")
endif()
if("${result}" STREQUAL "0")
  message(FATAL_ERROR "${MODE} unexpectedly succeeded: ${output}")
endif()
string(FIND "${diagnostic}" "${EXPECTED}" found)
if(found EQUAL -1)
  message(FATAL_ERROR "${MODE}: wrong failure (${result}): ${diagnostic}")
endif()
