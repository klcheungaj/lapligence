module tb;
  typedef enum logic [3:0] {ZERO = 4'h0, ONE = 4'h1, UNKNOWN = 4'hx} state_t;
  bit [3:0] bits [0:4];
  state_t states [0:4];
  initial begin
    $readmemb("q02_binary.mem", bits, 0, 4);
    $readmemb("q02_binary.mem", states, 0, 4);
    $display("Q02.binary_types bit=%b,%b,%b,%b,%b enum=%b,%b,%b,%b,%b", bits[0], bits[1], bits[2], bits[3], bits[4], states[0], states[1], states[2], states[3], states[4]);
  end
endmodule
