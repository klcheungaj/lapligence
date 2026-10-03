// SV2009 3.12.1, 26.3, 26.6: with merged compilation units, compilation-unit
// variables, functions, types and parameters declared in one file are
// visible in later files. A package re-exports imported names.
int unit_count = 2;
function int unit_next();
  unit_count = unit_count + 1;
  return unit_count;
endfunction
typedef struct packed { logic [3:0] a; logic [3:0] b; } unit_pair_t;
localparam unit_pair_t UNIT_P = '{4'h1, 4'h2};
package base_ns;
  int shared = 40;
  function automatic int add_shared(int v);
    return v + shared;
  endfunction
endpackage
package reexport_ns;
  import base_ns::*;
  export base_ns::shared;
  export base_ns::add_shared;
  int mine = add_shared(2);
endpackage
