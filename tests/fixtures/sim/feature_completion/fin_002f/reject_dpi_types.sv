// Every marked line is refused and no other one is: 35.5.4, 35.5.5, 35.5.6.
`timescale 1ns/1ns
module tb;
  class c_t;
    int v;
  endclass
  typedef struct {
    int a;
    c_t h;
  } object_t;
  typedef struct packed {logic [3:0] h, l;} packed_t;
  typedef enum {A, B} e_t;

  import "DPI-C" function int ok_enum(input e_t e, input packed_t p, input int a[], output string s);
  import "DPI-C" function void bad_class(input c_t h);  // wrong: SLANG-InvalidDPIArgType
  import "DPI-C" function void bad_queue(input int q[$]);  // wrong: SLANG-InvalidDPIArgType
  import "DPI-C" function void bad_map(input int m[string]);  // wrong: SLANG-InvalidDPIArgType
  import "DPI-C" function void bad_event(input event e);  // wrong: SLANG-InvalidDPIArgType
  import "DPI-C" function void bad_mailbox(input mailbox m);  // wrong: SLANG-InvalidDPIArgType
  import "DPI-C" function void bad_member(input object_t o);  // wrong: SLANG-InvalidDPIArgType
  import "DPI-C" function void bad_ref(ref int a);  // wrong: SLANG-DPIRefArg
  import "DPI-C" function packed_t bad_result();  // wrong: SLANG-InvalidDPIReturnType
  import "DPI-C" function logic [7:0] bad_vector();  // wrong: SLANG-InvalidDPIReturnType
  import "DPI-C" function integer bad_integer();  // wrong: SLANG-InvalidDPIReturnType
  import "DPI-C" function time bad_time();  // wrong: SLANG-InvalidDPIReturnType
  import "DPI-C" pure function void bad_pure_void(input int a);  // wrong: SLANG-DPIPureReturn
  import "DPI-C" pure function int bad_pure_output(output int a);  // wrong: SLANG-DPIPureArg
  import "DPI-C" c_twice = function int first(input int a);
  import "DPI-C" c_twice = function int second(input byte a);  // wrong: SLANG-DPISignatureMismatch

  function int exported(input int a);
    return a;
  endfunction
  task timed(input int a);
    #1;
  endtask
  function int queue_formal(input int q[$]);  // wrong: SLANG-InvalidDPIArgType
    return q.size();
  endfunction
  function packed_t packed_result();  // wrong: SLANG-InvalidDPIReturnType
    return 0;
  endfunction
  export "DPI-C" function exported;
  export "DPI-C" function timed;  // wrong: SLANG-DPIExportKindMismatch
  export "DPI-C" function queue_formal;
  export "DPI-C" function packed_result;
  export "DPI-C" function nowhere;  // wrong: SLANG-UndeclaredIdentifier
  export "DPI-C" function ok_enum;  // wrong: SLANG-DPIExportImportedFunc
endmodule
