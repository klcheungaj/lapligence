// Force and release of whole unpacked net arrays (IEEE 1800-2009 10.6.2:
// "a net"). Each cell is overridden by the forced value and takes its
// drivers' value again at release.
module tb;
  logic [3:0] d = 4'd1;
  wire [3:0] w [0:1];
  assign w[0] = d;
  assign w[1] = d + 4'd1;
  wire [3:0] v [1:0];
  assign v = '{4'd3, 4'd4};
  wire [1:0] g [0:1][0:1];
  assign g[0][0] = 2'd0;
  assign g[0][1] = 2'd1;
  assign g[1][0] = 2'd2;
  assign g[1][1] = 2'd3;
  initial begin
    #1 force w = '{4'd7, 4'd8};
    #1 $display("1 w=%0d %0d", w[0], w[1]);
    d = 4'd5;
    force v = w;
    #1 $display("2 w=%0d %0d v[1]=%0d v[0]=%0d", w[0], w[1], v[1], v[0]);
    release w;
    #1 $display("3 w=%0d %0d v[1]=%0d v[0]=%0d", w[0], w[1], v[1], v[0]);
    d = 4'd9;
    #1 $display("4 w=%0d %0d v[1]=%0d v[0]=%0d", w[0], w[1], v[1], v[0]);
    release v;
    #1 $display("5 v[1]=%0d v[0]=%0d", v[1], v[0]);
    force g = '{'{2'd3, 2'd2}, '{2'd1, 2'd0}};
    #1 $display("6 g=%0d %0d %0d %0d", g[0][0], g[0][1], g[1][0], g[1][1]);
    force w[1] = 4'd2;
    release g;
    #1 $display("7 g=%0d %0d %0d %0d w=%0d %0d", g[0][0], g[0][1], g[1][0], g[1][1], w[0], w[1]);
    release w;
    #1 $display("8 w=%0d %0d", w[0], w[1]);
    $finish(0);
  end
endmodule
