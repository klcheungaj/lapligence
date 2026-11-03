// Decision S07-D2: `%p` prints a multidimensional fixed array of strings or
// of records with string members as nested assignment patterns, one level per
// unpacked dimension.
//
// IEEE 1800-2009 21.2.1.7 (SystemVerilog-1800-2009.txt L36390-36403): "For
// unpacked array data types, it shall print the value as an assignment
// pattern which may include the use of index labels. The use of white space is
// implementation dependent; however the output shall be a legal
// interpretation of the assignment pattern syntax (see 10.8)." A flat list of
// every element is not a legal pattern for a two-dimensional array (10.8,
// 10.9.1); some simulators print one anyway, llg follows the clause text.
module tb;
  typedef struct { string s; int n; } rec_t;
  string g[2][2];
  rec_t r[2][1];
  initial begin
    g = '{'{"a", "b"}, '{"c", "d"}};
    r = '{'{'{s: "x", n: 1}}, '{'{s: "y", n: 2}}};
    $display("strings: %p", g);
    $display("records: %p", r);
    $display("row: %p", g[1]);
    $finish;
  end
endmodule
