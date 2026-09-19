// R04: every intermediate lane retains its own bounds.
module tb;
  logic [1:0][7:0] mem [0:0];
  bit [1:0][7:0] two_state_mem [0:0];
  logic signed [31:0] lane, index;
  logic [127:0] wide_index;
  bit [3:0] converted;
  initial begin
    mem[0] = 16'ha500;
    mem[0][0][6 +: 4] = 4'hf;
    if (mem[0] !== 16'ha5c0) $fatal(1, "upper boundary escaped lane");
    if (mem[0][0][6 +: 4] !== 4'bxx11) $fatal(1, "upper boundary read");
    mem[0] = 16'ha500;
    mem[0][0][-2 +: 4] = 4'hf;
    if (mem[0] !== 16'ha503) $fatal(1, "lower boundary write");
    if (mem[0][0][-2 +: 4] !== 4'b11xx) $fatal(1, "lower boundary read");
    // These coordinates are in the parent word but outside the selected lane.
    mem[0][0][9:8] = 2'b00;
    if (mem[0] !== 16'ha503 || mem[0][0][9:8] !== 2'bxx)
      $fatal(1, "ordinary fully outside selection");
    if (mem[0][2][0] !== 1'bx) $fatal(1, "invalid constant outer lane");
    mem[0][2][0] = 1'b1;
    if (mem[0] !== 16'ha503) $fatal(1, "invalid outer lane write");
    // The invalid outer lane must not become valid after adding an inner offset.
    lane = -1; index = 8;
    mem[0][lane][index] = 1'b0;
    if (mem[0] !== 16'ha503 || mem[0][lane][index] !== 1'bx)
      $fatal(1, "invalid prefix was resurrected");
    lane = 0; index = 'x;
    mem[0][lane][index +: 4] = 4'hf;
    if (mem[0] !== 16'ha503 || mem[0][lane][index +: 4] !== 4'bxxxx)
      $fatal(1, "unknown index");
    lane = 'z; index = 0;
    if (mem[0][lane][index] !== 1'bx) $fatal(1, "high impedance lane");
    mem[0][lane][index] = 1'b1;
    wide_index = 128'h10000000000000000;
    mem[0][0][wide_index +: 4] = 4'hf;
    if (mem[0] !== 16'ha503) $fatal(1, "wide index truncated");
    if (mem[0][0][wide_index +: 4] !== 4'bxxxx) $fatal(1, "wide index read");
    two_state_mem[0] = 16'h5a00;
    two_state_mem[0][0][6 +: 4] = 4'bxx11;
    if (two_state_mem[0] !== 16'h5ac0) $fatal(1, "two-state partial write");
    converted = mem[0][0][-2 +: 4];
    if (converted !== 4'b1100) $fatal(1, "two-state destination conversion");
    $display("packed bounds passed");
    $finish(0);
  end
endmodule
