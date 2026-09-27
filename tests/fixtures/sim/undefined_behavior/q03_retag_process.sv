module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } item_t;
  item_t value;
  logic [7:0] seed;
  event issued;
  initial begin
    value = tagged A(8'h11);
    seed = 8'h20;
    #1;
    seed = seed + 8'h13;
    value.A <= seed;
    -> issued;
    #1;
    $display("Q03.retag_process value=%b", value);
  end
  initial begin
    @issued;
    value = tagged B(8'h55);
    $display("Q03.retag_process blocking=%b", value);
  end
endmodule
