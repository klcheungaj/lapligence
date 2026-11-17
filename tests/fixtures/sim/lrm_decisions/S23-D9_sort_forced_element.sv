// Decision S23-D9: the array ordering methods leave a forced element at its
// forced value; the other elements take their places from the method.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13388-13390):
//   "A force statement to a variable shall override a procedural assignment,
//   continuous assignment or an assign procedural continuous assignment to
//   the variable until a release procedural statement is executed on the
//   variable."
// IEEE 1800-2009 7.12.2 (L9348-9349):
//   "Array ordering methods reorder the elements of any unpacked array
//   (fixed or dynamically sized) except for associative arrays."
// The text does not exempt them from a force. Industry-standard behaviour
// keeps a forced element through `reverse`, but `sort` and `rsort` overwrite
// it until its expression is next evaluated: it prints "1 5 40 41 100",
// "2 7 40 41 100", "3 7 40 8 100" and "4 100 40 50 7".
module tb;
  int a[4];
  int b[4];
  int v = 7;
  initial begin
    a = '{40, 41, 101, 100};
    force a[2] = 5;
    a.sort();
    #1 $display("1 %0d %0d %0d %0d", a[0], a[1], a[2], a[3]);
    b = '{40, 41, 101, 100};
    force b[2] = v;
    b.sort();
    #1 $display("2 %0d %0d %0d %0d", b[0], b[1], b[2], b[3]);
    v = 8;
    #1 $display("3 %0d %0d %0d %0d", b[0], b[1], b[2], b[3]);
    v = 50;
    b.rsort();
    #1 $display("4 %0d %0d %0d %0d", b[0], b[1], b[2], b[3]);
    $finish;
  end
endmodule
