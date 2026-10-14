// IEEE 1800-2009 18.3 L29236-29237: "Constraints support only 2-state values.
// The 4-state values (X or Z) or 4-state operators (e.g., ===, !== ) are
// illegal and shall result in an error." 18.6.1 L30402-30403: randomize()
// "returns 1 if it successfully sets all the random variables and objects to
// valid values; otherwise, it returns 0."
// Decision (llg reading; the text does not say whether an X/Z *value* of a
// state variable read by an active constraint is the error it names): the
// call fails at run time. It returns 0, leaves the random variables
// unchanged, reports an error and the run ends with exit status 1 after
// $finish (decision S29-D5). The 2-state reading would convert X/Z to 0 as
// for seeds (S28-D3, S28-D8) and solve.
class item_c;
  rand bit [3:0] a;
  logic [3:0] limit;
  constraint c { a < limit; }
endclass
module tb;
  initial begin
    static item_c item = new;
    int ok;
    item.a = 4'd9;
    item.limit = 4'b1x00;
    ok = item.randomize();
    $display("unknown limit: ok=%0d a=%0d", ok, item.a);
    item.limit = 4'd3;
    ok = item.randomize();
    $display("known limit: ok=%0d a_below=%0d", ok, item.a < 4'd3);
    $finish;
  end
endmodule
