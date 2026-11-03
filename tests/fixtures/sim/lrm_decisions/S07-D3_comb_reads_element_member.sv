// Decision S07-D3: an `always_comb` that reads a queue member of a record
// element of a fixed array runs again whenever that member changes, including
// through a method call such as `push_back`.
//
// IEEE 1800-2009 9.2.2.2.1 (SystemVerilog-1800-2009.txt L11184-11186): "The
// implicit sensitivity list of an always_comb includes the expansions of the
// longest static prefix of each variable or select expression that is read
// within the block". `fa[0].q.size()` reads `fa[0].q`, whose longest static
// prefix is the member itself. Some simulators do not run the block again
// when the member is changed by a method; llg follows the clause text.
module tb;
  typedef struct { string s; int q[$]; } item_t;
  item_t fa[2];
  int c;
  always_comb c = fa[0].q.size();
  initial begin
    #1 $display("start: %0d", c);
    fa[0].q.push_back(4);
    #1 $display("push: %0d", c);
    fa[0].q = '{1, 2, 3};
    #1 $display("assign: %0d", c);
    fa[1].q.push_back(9);
    void'(fa[0].q.pop_front());
    #1 $display("pop: %0d", c);
    $finish;
  end
endmodule
