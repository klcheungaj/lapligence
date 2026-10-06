// RTL-101b limit: a whole-value binding beyond packed capacity needs
// lexical storage declared by a procedural statement.
module tb;
  typedef union tagged {
    logic [1023:0] w [0:2047];
    bit [7:0] t;
  } tu_t;
  tu_t v;
  logic [7:0] y;
  assign y = v matches tagged w .q ? q[0][7:0] : 8'd0;
  initial begin
    v = tagged t 8'd1;
    #1 $display("%0d", y);
    $finish(0);
  end
endmodule
