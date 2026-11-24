// Allocation failure matrix: a waveform dump of scalars, vectors, reals,
// arrays and nested scopes, switched off and on and flushed.
`timescale 1ns / 1ns
module leaf #(
    parameter int WIDTH = 8
) (
    input logic clk,
    input logic [WIDTH-1:0] d,
    output logic [WIDTH-1:0] q
);
  logic [WIDTH-1:0] stage[2];
  always_ff @(posedge clk) begin
    stage[0] <= d;
    stage[1] <= stage[0];
  end
  assign q = stage[1];
endmodule

module tb;
  logic clk = 1'b0;
  logic [7:0] count = '0;
  logic [99:0] wide = '0;
  logic [7:0] narrow_q;
  logic [99:0] wide_q;
  real level = 0.0;
  wire tri_net;
  integer steps = 0;

  leaf #(8) narrow (
      .clk(clk),
      .d  (count),
      .q  (narrow_q)
  );
  leaf #(100) broad (
      .clk(clk),
      .d  (wide),
      .q  (wide_q)
  );
  assign tri_net = count[0] ? 1'bz : count[1];

  always #5 clk = ~clk;
  always @(posedge clk) begin
    count <= count + 8'd3;
    wide  <= {wide[91:0], count};
    level <= level + 0.25;
    steps <= steps + 1;
  end

  initial begin
    $dumpfile("alloc_waves.vcd");
    $dumpvars(0, tb);
    repeat (6) @(posedge clk);
    $dumpoff;
    repeat (2) @(posedge clk);
    $dumpon;
    $dumpflush;
    repeat (4) @(posedge clk);
    $dumpall;
    $display("values %h %h %0.2f %0d", narrow_q, wide_q[31:0], level, steps);
    $display("done");
    $finish;
  end
endmodule
