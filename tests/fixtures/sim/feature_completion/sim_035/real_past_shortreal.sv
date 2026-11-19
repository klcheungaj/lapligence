// past-real: `$past`, `$stable` and `$changed` of a shortreal and of a
// shortreal array cell (IEEE 1800-2009 16.9.3; decision AA-D8). The timeline
// is in readme.md.
module tb;
  logic clk = 1'b0;
  shortreal s = 1.5;
  shortreal sa[2] = '{0.25, 0.75};
  int k = 0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    s <= s * 2.0;
    if (k == 1) sa[1] <= 4.5;
    k <= k + 1;
  end
  always @(posedge clk)
    $display("%0d s=%0.2f st=%0d cell=%0.2f ch=%0d", $time, $past(s), $stable(s), $past(sa[1]),
             $changed(sa[1]));
  a_s: assert property (@(posedge clk) $past(s) < s) else $display("%0d a_s fail", $time);
  initial #37 $finish;
endmodule
