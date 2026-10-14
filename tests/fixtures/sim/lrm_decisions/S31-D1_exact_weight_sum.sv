// Decision S31-D1: the sum of the randcase weights is exact; it does not wrap
// at the precision of the widest weight, so each branch is taken with
// probability weight / sum.
//
// IEEE 1800-2009 18.16 (SystemVerilog-1800-2009.txt L31311-31312, L31329-31330):
//   "An item's weight divided by the sum of all weights gives the probability
//   of taking that branch."
// and (L31346-31351):
//   "The sum of the weights is computed using standard addition semantics
//   (maximum precision of all weights), where each summand is unsigned. ...
//   The resulting weights are added as unsigned values using 12-bit
//   precision. The weight selection then uses unsigned 12-bit comparison."
//
// The two sentences disagree for byte weights 200 and 100: an 8-bit sum is
// 300 mod 256 = 44, which would never reach the second branch (200 >= 44),
// while the probability rule gives the second branch a share of 1/3. llg
// follows the probability rule and sums without wrapping. Over 600 executions
// the second branch is taken with certainty for practical purposes
// (probability of none: (2/3)^600).
module tb;
  byte unsigned a = 200;
  byte unsigned b = 100;
  int first, second;
  initial begin
    repeat (600) begin
      randcase
        a : first++;
        b : second++;
      endcase
    end
    $display("picks: %0d", first + second);
    $display("first branch taken: %s", first > 0 ? "yes" : "no");
    $display("second branch taken: %s", second > 0 ? "yes" : "no");
    $finish;
  end
endmodule
