// IEEE 1800-2009 18.3 L29236-29237: "Constraints support only 2-state values.
// The 4-state values (X or Z) or 4-state operators (e.g., ===, !== ) are
// illegal and shall result in an error." 18.6.1 L30402-30403: randomize()
// "returns 1 if it successfully sets all the random variables and objects to
// valid values; otherwise, it returns 0."
// Decision S29-D1: an X or Z bit in a state variable (or an inactive random
// variable) that an active constraint reads is the error 18.3 names, and it
// ends the simulation at once: randomize() does not return, nothing after it
// runs and final procedures do not run (llg: an error on stderr, exit
// status 1). The text does not say what the error does; llg follows the
// industry-standard behaviour.
class item_c;
  rand bit [3:0] a;
  logic [3:0] limit;
  constraint c { a < limit; }
endclass
module tb;
  final begin
    $display("final block");
  end

  initial begin
    static item_c item = new;
    int ok;
    item.limit = 4'd3;
    ok = item.randomize();
    $display("known limit: ok=%0d a_below=%0d", ok, item.a < 4'd3);
    item.limit = 4'b1x00;
    ok = item.randomize();
    $display("unknown limit: ok=%0d a=%0d", ok, item.a);
    $finish;
  end
endmodule
