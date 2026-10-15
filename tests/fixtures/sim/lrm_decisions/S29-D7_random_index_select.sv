// IEEE 1800-2009 18.5.7.1 L29926-29927: "Invalid or out or bound array indices
// are not automatically eliminated; users must explicitly exclude these
// indices using predicates." The clause covers array indices in foreach; the
// text is silent on a packed bit select whose index is itself random.
// Decision (llg deferral, until SIM-030): a select whose index is a random
// variable is rejected at compile time with a located error, rather than
// solved under a guessed rule. This is a negative case: llg prints nothing.
// A simulator that solves it prints "holds=1".
class item_c;
  rand bit [7:0] v;
  rand bit [2:0] i;
  constraint c { v[i] == 1'b1; v == 8'h10; }
endclass
module tb;
  initial begin
    static item_c item = new;
    void'(item.randomize());
    $display("holds=%0d", item.i == 3'd4);
    $finish;
  end
endmodule
