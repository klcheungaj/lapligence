// past-real: `$past`, `$stable` and `$changed` of real array cells (IEEE
// 1800-2009 16.9.3, 7.4.6; decisions S35-D1 and AA-D8): variable, X and
// out-of-range indices, depth 1-3, a never-written array, a cell in an
// assertion operand and in an implication. The timeline is in readme.md.
module tb;
  logic clk = 1'b0;
  real ra[4] = '{0.5, 1.5, 2.5, 3.5};
  real rz[3];
  real rs[3] = '{0.5, 1.5, 2.5};
  logic [2:0] i = 3'd1;
  logic [1:0] ix = 2'bx1;
  logic [1:0] j = 2'd0;
  int k = 0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    ra[i] <= ra[i] + 10.0;
    rz[k % 3] <= k;
    if (k == 2) rs[1] <= 7.0;
    j <= (k > 0) ? 2'd1 : 2'd0;
    i <= i + 3'd1;
    k <= k + 1;
  end
  always @(posedge clk) begin
    $display("%0d c1=%0.2f c2=%0.2f c3=%0.2f x=%0.2f rz=%0.2f i=%0d", $time, $past(ra[i]),
             $past(ra[i], 2), $past(ra[i], 3), $past(ra[ix]), $past(rz[1]), i);
    $display("%0d st=%0d ch=%0d st1=%0d", $time, $stable(rs[j]), $changed(rs[j]), $stable(rs[1]));
  end
  a_cell: assert property (@(posedge clk) $past(ra[1]) <= ra[1])
    else $display("%0d a_cell fail", $time);
  a_ch: assert property (@(posedge clk) $changed(rs[1]) |-> rs[1] > 5.0)
    else $display("%0d a_ch fail", $time);
  initial #77 $finish;
endmodule
