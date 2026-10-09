// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-D5_match_multiplicity.sv
// Decision AA-D5: a sequence has one match per distinct way it matches. Two
// `or` operands ending on one tick are two matches; every pair of `and`
// operand matches is a match; `first_match` keeps every match at the
// earliest end; match items run once per match. Ways that differ only in how
// the empty word is matched (empty iterations, several empty-admitting
// operands) are one match: an empty match consumes no clock tick, the text is
// silent on it, and read literally `r[*0:$]` of an empty-admitting `r` would
// have infinitely many matches.
//
// IEEE 1800-2009 16.9.7 (SystemVerilog-1800-2009.txt L23414-23416):
//   "the first operand sequence matches at clock ticks 9, 10, 11, 12, and 13,
//   while the second operand matches at clock tick 12. The composite sequence,
//   therefore, has one match at each of clock ticks 9, 10, 11, and 13 and has
//   two matches at clock tick 12."
// 16.9.5 (L23159-23163): "Each match of the first operand sequence is
//   combined with the single match of the second operand sequence [...] The
//   result of this computation is five matches of the composite sequence,
//   four of them ending at clock tick 12".
// 16.9.8 (L23475-23476): "If there are multiple matches of seq with the same
//   ending clock tick as the earliest one, then all those matches are matches
//   of first_match (seq)."
// 16.15.3 (L26625-26627): matches "shall be counted, with multiplicity, toward
//   the total number of times matched for the attempt. [...] The pass
//   statement [...] shall be executed, with multiplicity, for each match".
// 16.11 (L24156-24157): subroutines "can be called at the end of a successful
//   non-empty match of a sequence".
//
// `go` is 1 on tick 1 only, so each sequence below has one attempt that
// starts on tick 1. a: ticks 1-2; b: tick 2; c: ticks 1 and 3.
// - or:          `(a ##1 b)` and `(a ##1 a)` both end on tick 2: 2 matches.
// - and:         `a[*0:1]` ends empty or on tick 1, `(c ##2 c)` on tick 3:
//                2 pairs, both ending on tick 3: 2 matches.
// - first_match: the two `or` matches on tick 2 are the earliest: 2 matches.
// - match item:  the `or` with an attached call: 2 calls, 2 matches.
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic a = 1'b0, b = 1'b0, c = 1'b0;
  int t = 0;
  int or_hits = 0, and_hits = 0, first_hits = 0, item_hits = 0;
  // Tick:               1 2 3 4 5
  bit [1:5] wave_a = 5'b1_1_0_0_0;
  bit [1:5] wave_b = 5'b0_1_0_0_0;
  bit [1:5] wave_c = 5'b1_0_1_0_0;

  function automatic void note();
    $display("match item call t=%0d", t);
  endfunction

  c_or: cover sequence (@(posedge clk) go ##0 ((a ##1 b) or (a ##1 a))) or_hits++;
  c_and: cover sequence (@(posedge clk) go ##0 (a[*0:1] and (c ##2 c))) and_hits++;
  c_first: cover sequence (@(posedge clk)
      go ##0 first_match((a ##1 b) or (a ##1 a) or (c ##2 c))) first_hits++;
  c_item: cover sequence (@(posedge clk) go ##0 ((a ##1 b) or (a ##1 a), note()))
    item_hits++;

  initial begin
    for (int k = 1; k <= 5; k++) begin
      t = k;
      go = k == 1;
      a = wave_a[k];
      b = wave_b[k];
      c = wave_c[k];
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $display("or=%0d and=%0d first_match=%0d match_item=%0d", or_hits, and_hits,
             first_hits, item_hits);
    $finish;
  end
endmodule
