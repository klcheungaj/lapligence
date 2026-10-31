// Decision A19-D53: the variable that holds a function's result.
`timescale 1ns/1ns
typedef struct packed { logic [3:0] a; logic b; } st_t;
module tb;
  function int sv_f(input int a); return a + 1; endfunction
  function automatic int af(input int a); return a * 2; endfunction
  function logic [7:2] rng(input int a); rng = a; endfunction
  function st_t sf; return '{a: 4'h5, b: 1'b1}; endfunction
  function real rf; return 2.5; endfunction
  function void vf; endfunction
  function logic bit1; return 1'b1; endfunction
  task tk; endtask
  int x;
  initial begin
    x = sv_f(4);
    x = af(3);
    x = rng(9);
    x = sf();
    #1 $d53;
  end
endmodule
