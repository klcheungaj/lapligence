// RTL-101b: an inactive member of a tagged-union call result reads X with a
// runtime error, like an inactive member of a variable.
module tb;
  typedef union tagged {
    logic [1023:0] w [0:2047];
    bit [7:0] t;
  } tu_t;
  function automatic tu_t g(input bit [7:0] d);
    return tagged t d;
  endfunction
  initial begin
    $display("A %0d", g(8'd3).w[0][7:0]);
    $display("B %0d", g(8'd3).t);
    $finish(0);
  end
endmodule
