// llg policy case: not in the portable lrm_decisions set, because it compares
// the rule choice with a $urandom_range draw in the same simulator, a mapping
// that is an llg choice, not LRM behaviour.
// Decision S16-D9: choosing among two or more rules evaluates every weight
// once, in source order, and then takes exactly one value from the calling
// thread's stream, the value $urandom_range(sum - 1, 0) would return at that
// point, just as randcase does (decision S31-D7). The first rule whose running
// weight total exceeds the value is generated. The mapping from a seed to
// values is the simulator's own, so only the equality of the two forms is
// compared. This is an llg policy for a clause that leaves the draw open.
//
// IEEE 1800-2009 18.17.1 (SystemVerilog-1800-2009.txt L31502-31503):
//   "If no weight is specified, the production shall use a weight of 1."
// IEEE 1800-2009 18.17.1 (L31491-31492):
//   "Weight expressions are evaluated when their enclosing production is
//   selected, thus allowing weights to change dynamically."
module tb;
  int chosen, by_range;
  int unsigned next_a, next_b, r;
  int mismatches, stream_mismatches, order_errors;
  string order;
  function int weight(string name, int value);
    order = {order, name};
    return value;
  endfunction
  initial begin
    for (int s = 1; s <= 200; s++) begin
      void'($urandom(s));
      order = "";
      randsequence (main)
        main : r0 := (weight("0", 3)) | r1 := (weight("1", 1)) | r2 := (weight("2", 0))
             | r3 := (weight("3", 4)) | r4 ;
        r0 : { chosen = 0; } ;
        r1 : { chosen = 1; } ;
        r2 : { chosen = 2; } ;
        r3 : { chosen = 3; } ;
        r4 : { chosen = 4; } ;
      endsequence
      next_a = $urandom;
      void'($urandom(s));
      r = $urandom_range(8);
      if (r < 3) by_range = 0;
      else if (r < 4) by_range = 1;
      else if (r < 8) by_range = 3;
      else by_range = 4;
      next_b = $urandom;
      if (chosen != by_range) mismatches = mismatches + 1;
      if (next_a != next_b) stream_mismatches = stream_mismatches + 1;
      if (order != "0123") order_errors = order_errors + 1;
    end
    $display("rule mismatches: %0d", mismatches);
    $display("stream mismatches: %0d", stream_mismatches);
    $display("weight order errors: %0d", order_errors);
    $finish;
  end
endmodule
