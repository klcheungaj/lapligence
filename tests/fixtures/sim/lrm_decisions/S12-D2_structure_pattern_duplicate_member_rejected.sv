// Decision S12-D2: a structure pattern that names one member more than once
// is a compile error, reported at the repeated member name.
//
// IEEE 1800-2009 12.6 (SystemVerilog-1800-2009.txt L17808-17810): "A
// structure pattern succeeds if, recursively, each of the nested member
// patterns matches the corresponding member values in V. In structure
// patterns with named members, the textual order of members does not matter,
// and members can be omitted." The text does not say whether a member may be
// named twice. 10.9.1 (L13676-13677) makes a repeated index of an array
// pattern an error: "It shall be an error to specify the same index more than
// once in a single array pattern expression", and an assignment pattern that
// names a structure member twice is rejected as well.
module tb;
  typedef struct packed { logic [7:0] data; logic [3:0] tag; } payload_t;
  payload_t value;
  initial begin
    value = '{data: 8'h5a, tag: 4'h3};
    if (value matches '{data: 8'h5a, tag: 4'h3, data: .*})
      $display("matched");
    else
      $display("not matched");
    $finish;
  end
endmodule
