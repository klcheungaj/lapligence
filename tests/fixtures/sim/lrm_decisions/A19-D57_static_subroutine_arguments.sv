// Decision A19-D57: the argument variables of static tasks and functions
// (IEEE 1800-2009 13.3, 37.13, 37.17). Companion of A19-D57_static_subroutine_arguments.c.
`timescale 1ns/1ns
module tb;
  int acc;
  task st(input int a, output logic [3:0] b, inout int c);
    int loc;
    loc = a + 1;
    b = a[3:0];
    c = c + a;
  endtask
  function int sf(input int x, input byte y);
    return x + y;
  endfunction
  task automatic at(input int q);
    acc = q;
  endtask
  logic [3:0] bo;
  int cio = 10;
  initial begin
    st(5, bo, cio);
    acc = sf(3, 8'sd4);
    at(9);
    #1 $d57;
  end
endmodule
