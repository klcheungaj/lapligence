// llg policy case: not in the portable lrm_decisions set, because it compares
// the randcase draw with a $urandom_range draw in the same simulator, a
// mapping that is an llg choice, not LRM behaviour.
// Decision S31-D7: a randcase takes exactly one value from the calling
// thread's stream, the value $urandom_range(sum - 1, 0) would return at that
// point, and the first branch whose running weight total exceeds the value is
// taken. A seed therefore replays the same branch, and the stream after the
// statement is the one after a single $urandom_range call. The mapping from
// a seed to values is the simulator's own, so only the equality of the two
// forms is compared. This is an llg policy for a sentence that leaves the
// range and the number of draws open.
//
// IEEE 1800-2009 18.16 (SystemVerilog-1800-2009.txt L31353-31359):
//   "Each call to randcase retrieves one random number in the range of 0 to
//   the sum of the weights. The weights are then selected in declaration
//   order: smaller random numbers correspond to the first (top) weight
//   statements."
//   "The random numbers are obtained from $urandom_range(); ..."
//
// llg reads "0 to the sum" as 0 to sum - 1, so a branch with weight w owns
// exactly w values.
module tb;
  int w0 = 3;
  int w1 = 1;
  int w2 = 0;
  int w3 = 4;
  int by_randcase, by_range, next_a, next_b, r;
  int mismatches, stream_mismatches;
  initial begin
    for (int s = 1; s <= 200; s++) begin
      void'($urandom(s));
      randcase
        w0 : by_randcase = 0;
        w1 : by_randcase = 1;
        w2 : by_randcase = 2;
        w3 : by_randcase = 3;
      endcase
      next_a = $urandom;
      void'($urandom(s));
      r = $urandom_range(7);
      if (r < 3) by_range = 0;
      else if (r < 4) by_range = 1;
      else by_range = 3;
      next_b = $urandom;
      if (by_randcase != by_range) mismatches++;
      if (next_a != next_b) stream_mismatches++;
    end
    $display("branch mismatches: %0d", mismatches);
    $display("stream mismatches: %0d", stream_mismatches);
    $finish;
  end
endmodule
