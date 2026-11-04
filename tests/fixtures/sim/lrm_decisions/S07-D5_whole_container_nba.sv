// Decision S07-D5: a whole queue, dynamic or associative array, and a record
// with such a member, takes nonblocking writes; the source is read when the
// statement executes.
//
// IEEE 1800-2009 6.21 (SystemVerilog-1800-2009.txt L7007-7008): "Automatic
// variables and members or elements of dynamic variables--class properties
// and dynamically sized variables--shall not be written with nonblocking,
// continuous, or procedural continuous assignments." 10.4.2 L13048 adds only
// automatic variables. A whole dynamically sized variable is neither. Some
// simulators reject nonblocking writes to variables of dynamic types; llg
// follows the clause text.
module tb;
  typedef struct { string s; int q[$]; } rec_t;
  rec_t m, n;
  int q[$], r[$] = '{1, 2};
  int d[];
  int a[string];
  initial begin
    #1 m.s = "m";
    m.q.push_back(5);
    #1 $display("%0d %0d %0d %0d [%s]", q.size(), d.size(), a.size(), n.q.size(), n.s);
    #1 $display("%0d %0d %0d %0d [%s]", q.size(), d.size(), a.size(), n.q.size(), n.s);
    $finish(0);
  end
  initial begin
    #2 q <= r;
    d <= r;
    a <= '{"k":7};
    n <= m;
  end
endmodule
