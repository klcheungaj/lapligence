module tb;
  typedef enum logic [7:0] {ZERO = 8'h00, ONE = 8'h01, UNKNOWN = 8'hxx} state_t;
  bit [7:0] bits [0:4];
  state_t states [0:4];
  initial begin
    $readmemh("q02_hex.mem", bits, 0, 4);
    $readmemh("q02_hex.mem", states, 0, 4);
    $display("Q02.types bit=%b,%b,%b,%b,%b enum=%b,%b,%b,%b,%b", bits[0], bits[1], bits[2], bits[3], bits[4], states[0], states[1], states[2], states[3], states[4]);
  end
endmodule
