// Decision S07-D4: a nonblocking write to a member of a record element keeps
// the element position evaluated when it executes, and stores into the element
// that position holds at commit.
//
// IEEE 1800-2009 10.4.2 (SystemVerilog-1800-2009.txt L13070-13072): "If
// variable_lvalue requires an evaluation, it shall be evaluated at the same
// time as the expression on the right-hand side." 10.4.2 bars only automatic
// variables as targets. Some simulators reject nonblocking writes to variables
// of dynamic types; llg follows the clause text.
module tb;
  typedef struct { string s; int n; } rec_t;
  rec_t nb[3];
  int k = 1;
  initial begin
    #1 nb[k].n <= 11;
    #2 nb[k].s <= "x";
  end
  initial begin
    #1;
    #0 nb[k] = '{"w", 2};
    k = 2;
    $display("%0d [%s] %0d", nb[1].n, nb[1].s, nb[2].n);
    #1 $display("%0d [%s] %0d", nb[1].n, nb[1].s, nb[2].n);
    k = 7;
    #2 $display("[%s] [%s] [%s]", nb[0].s, nb[1].s, nb[2].s);
    $finish(0);
  end
endmodule
