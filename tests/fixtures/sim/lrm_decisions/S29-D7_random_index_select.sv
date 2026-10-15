// IEEE 1800-2009 18.5.7.1 L29926-29927: "Invalid or out or bound array indices
// are not automatically eliminated; users must explicitly exclude these
// indices using predicates." The clause covers array indices in foreach; the
// text is silent on a packed bit select whose index is itself random.
// Decision S29-D7, superseded by S30-D4 in SIM-030: a select whose index is a
// random variable is solved; an index outside the vector selects x, so such
// an assignment is not a solution.
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
