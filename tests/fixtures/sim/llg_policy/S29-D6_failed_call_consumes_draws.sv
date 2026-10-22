// llg policy case: not in the portable lrm_decisions set, because which
// draws a call makes before it finds the constraints infeasible depends on
// the solver; the portable part is lrm_decisions/S29-D6_failed_call_keeps_values.
// IEEE 1800-2009 18.15 L31270: "Each object maintains its own internal RNG,
// which is used exclusively by its randomize() method." 18.6.3 L30471-30472:
// a failed randomize() leaves the random variables unchanged. The text does
// not say whether a failed call advances the object's RNG.
// Decision S29-D6 (llg policy): a failed call keeps the draws it made before it
// found the constraints infeasible, so the call after it does not repeat the
// values the same call would have drawn without the failed one. Here the
// unconstrained `a` is drawn before the infeasible `z` is found.
class item_c;
  rand bit [15:0] a;
  rand bit [3:0] z;
endclass
module tb;
  initial begin
    static item_c item = new;
    bit [15:0] plain_a, kept_a, after_a;
    int ok;
    item.srandom(11);
    void'(item.randomize());
    void'(item.randomize());
    plain_a = item.a;
    item.srandom(11);
    void'(item.randomize());
    kept_a = item.a;
    ok = item.randomize() with { z > 4'd15; };
    $display("failed ok=%0d unchanged=%0d", ok, item.a == kept_a);
    void'(item.randomize());
    after_a = item.a;
    $display("stream_moved=%0d", plain_a != after_a);
    $finish;
  end
endmodule
