module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } item_t;
  item_t value;
  logic [7:0] seed;
  initial begin
    value = tagged A(8'h11);
    seed = 8'h20;
    #1;
    seed = seed + 8'h13;
    value.A <= seed;
    #1;
    $display("Q03.control value=%b member=%b", value, value.A);
  end
endmodule
