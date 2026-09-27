module tb;
  typedef enum logic signed [3:0] {NEG = -1, ZERO = 0, ONE = 1} state_t;
  state_t mem [0:3];
  initial begin
    $readmemh("q02_enum.mem", mem, 0, 3);
    $display("Q02.enum_numeric values=%b,%b,%b,%b", mem[0], mem[1], mem[2], mem[3]);
  end
endmodule
