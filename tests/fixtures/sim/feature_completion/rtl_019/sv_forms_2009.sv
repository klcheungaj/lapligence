// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/sv_forms_2009.sv
// IEEE 1800-2009 forms that IEEE 1364-2001 lacks. Each one executes here and
// has a single-form neg_2001_*.v companion; this whole file also rejects
// under --edition v2001. Values in sv_forms_2009.out are derived by hand.
module leaf #(parameter int W = 4, localparam int D = W * 2)
             (input logic [W-1:0] a, output logic [D-1:0] y);
  assign y = {a, a};
endmodule

module tb;
  timeunit 1ns;
  timeprecision 1ns;
  logic [3:0] a = 4'h5;
  logic [7:0] y;
  leaf #(.W(4)) u (.a, .y);
  logic [1:0][3:0] packed2 = {4'hA, 4'h5};
  logic [7:0] mem [4];
  int q[$];
  int assoc[int];
  logic clk = 1'b0;
  int edges = 0;
  always @(edge clk) edges = edges + 1;
  for (genvar i = 0; i < 2; i = i + 1) begin : g
    localparam int V = i + 1;
  end
  task automatic bump(output int r, input int by = 3);
    r = by;
    r = r + 1;
  endtask
  function automatic int twice(input int x, output int seen);
    seen = x;
    return x * 2;
  endfunction
  function automatic int one();
    return 1;
  endfunction
  initial begin
    automatic int local_init = 7;
    int r, s, t;
    mem[3] = 8'h11;
    q.push_back(9);
    assoc[5] = 6;
    bump(r);
    t = twice(5, s);
    #1ns clk = 1'b1;
    #1 clk = 1'b0;
    #1;
    lbl: begin
      $display("y=%h hi=%h mem3=%h q=%0d assoc=%0d", y, packed2[1], mem[3], q[0], assoc[5]);
    end : lbl
    $display("r=%0d s=%0d t=%0d one=%0d init=%0d g1=%0d", r, s, t, one(), local_init, g[1].V);
    $display("cast=%0d signed=%0d edges=%0d", 8'(a) + 8'd250, signed'(4'hF), edges);
    $finish;
  end
endmodule : tb
