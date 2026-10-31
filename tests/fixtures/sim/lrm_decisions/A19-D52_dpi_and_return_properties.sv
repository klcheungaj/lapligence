// Decision A19-D52: the DPI properties and return range of tasks and
// functions, and the interface of a modport.
`timescale 1ns/1ns
interface ifc;
  logic a;
  modport mp (input a);
endinterface
module tb;
  ifc i0 ();
  import "DPI-C" pure function int cadd(input int a, input int b);
  import "DPI-C" context task ctask(input int a);
  import "DPI-C" d52_other = function int other(input int a);
  export "DPI-C" function sv_f;
  export "DPI-C" d52_task = task sv_t;
  function int sv_f(input int a); return a; endfunction
  task sv_t; endtask
  function logic [7:2] rng(input int a); return a; endfunction
  function logic bit1; return 1'b0; endfunction
  function real rf; return 1.0; endfunction
  function void vf; endfunction
  initial #1 $d52;
endmodule
