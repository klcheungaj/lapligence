// RTL-101: inactive members of a column-layout tagged union stay guarded.
module tb;
  typedef union tagged {
    logic [1023:0] w [0:2047];
    bit [7:0] t;
  } tu_t;
  tu_t v;
  initial begin
    v = tagged t 8'd4;
    $display("A %0d", v.w[0][7:0]);
    v.w[1] = 1024'd3;
    $display("B %0d", v.t);
    $finish(0);
  end
endmodule
