// IEEE 1800-2009 10.9.1, 10.10, 7.6: dense (below-threshold) array rows are
// items of descriptor-stored pattern sources (positional, keyed, default and
// replicated) and targets of descriptor row scatter (blocking, nonblocking,
// a selected row of a dense two-dimensional array, descending bounds and a
// continuous assignment); each row is copied cell for cell in storage order.
module tb;
  localparam int M = 3000;
  logic [7:0] src [3][M];
  logic [7:0] dsc [2:0][M-1:0];
  logic [7:0] a [M];
  logic [7:0] b [M];
  logic [7:0] t [M-1:0];
  logic [7:0] w [2][M];
  logic [7:0] c [M];
  logic [7:0] cw [M];
  int j;
  assign '{c, cw} = w;
  initial begin
    foreach (a[k]) begin a[k] = 8'(k); b[k] = ~8'(k); end
    a[7] = 8'hzx;
    src = '{a, b, a};
    $display("A %h %h %h %h", src[0][5], src[1][5], src[2][M-1], src[0][7]);
    src = '{default: b};
    $display("B %h %h", src[0][5], src[2][5]);
    src = '{1: a, default: b};
    $display("C %h %h %h", src[0][5], src[1][5], src[2][5]);
    src = '{3{a}};
    $display("D %h", src[2][6]);
    dsc = '{a, b, a};
    $display("E %h %h", dsc[2][M-1], dsc[1][M-1]);
    // scatter into dense rows, one two-state, then from a descending source
    src = '{a, b, a};
    '{b, t, a} = src;
    $display("F %h %h %h %h", b[5], t[5], t[7], a[5]);
    foreach (a[k]) a[k] = 8'(k);
    dsc = '{a, a, a};
    '{a, b, t} = dsc;
    $display("G %h %h %h", a[M-1], b[0], t[3]);
    // nonblocking scatter, selected target row of a dense 2-D array
    j = 1;
    '{w[j], a, b} <= src;
    a[0] = 8'h99;
    #1 $display("H %h %h %h", w[1][5], a[0], b[0]);
    #1 $display("I %h %h", c[5], cw[5]);
    $finish(0);
  end
endmodule
