// IEEE 1800-2009 18.6.3 L30471-30472: "If randomize() fails, the constraints
// are infeasible, and the random variables retain their previous values."
// 18.15 L31270: "Each object maintains its own internal RNG, which is used
// exclusively by its randomize() method." The text does not say whether a
// failed call advances the object's RNG.
// Decision S29-D6: a failed call leaves the random variables unchanged.
// Whether it advances the object's RNG depends on the draws the solver made
// before it found the constraints infeasible, which is implementation
// specific; llg keeps those draws (llg_policy/S29-D6_failed_call_consumes_draws).
class item_c;
  rand bit [15:0] a;
  rand bit [3:0] z;
endclass
module tb;
  initial begin
    static item_c item = new;
    bit [15:0] kept_a;
    int ok;
    item.srandom(11);
    void'(item.randomize());
    kept_a = item.a;
    ok = item.randomize() with { z > 4'd15; };
    $display("failed ok=%0d unchanged=%0d", ok, item.a == kept_a);
    $finish;
  end
endmodule
