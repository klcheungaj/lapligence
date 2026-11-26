`timescale 1ns/1ns
// Decision SD10-D2: a read, inside a called subroutine, of storage declared in
// that subroutine adds nothing to what a continuous assignment, a port
// expression, always_comb, a wait or an event control observes.
//
// IEEE 1800-2009 9.2.2.2.1 (SystemVerilog-1800-2009.txt L11184-11188): "The
//   implicit sensitivity list of an always_comb includes the expansions of the
//   longest static prefix of each variable or select expression that is read
//   within the block or within any function called within the block with the
//   following exceptions: a) any expansion of a variable declared within the
//   block or within any function called within the block". The other forms
//   have no such sentence; they follow the same rule.
module sub(input int i);
endmodule

module tb;
  int a, c, m, x;
  wire [31:0] d, e, f;
  function int count(input bit inc);
    static int k = 0;
    if (inc) k = k + 1;
    return k;
  endfunction
  function int times4(input int v);
    integer i;
    times4 = 0;
    for (i = 0; i < 4; i = i + 1) times4 = times4 + v;
  endfunction

  assign a = count(0);
  assign d = count(0) + 1;
  always_comb c = count(0);
  sub s(count(0));
  initial begin wait (count(0) == 2); $display("wait"); end
  initial begin @(count(0)); $display("event"); end

  assign e = times4(x);
  assign f = times4(x + 1);
  initial begin wait (times4(x) == 8); $display("wait %0t", $time); end
  initial begin wait (times4(x + 3) == 16); $display("wait %0t", $time); end

  initial begin
    #1 m = count(1);
    #1 $display("%0d %0d %0d %0d", a, d, c, s.i);
    #1 m = count(1);
    #1 $display("%0d %0d %0d %0d", a, d, c, s.i);
    #1 count.k = 7;
    #1 $display("%0d %0d %0d %0d", a, d, c, s.i);
    #1 x = 1;
    #1 $display("%0d %0d", e, f);
    #1 x = 2;
    #1 $display("%0d %0d", e, f);
    $finish;
  end
endmodule
