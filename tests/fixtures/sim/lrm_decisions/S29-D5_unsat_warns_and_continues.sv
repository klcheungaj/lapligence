// IEEE 1800-2009 18.6.1 L30402-30403: randomize() "returns 1 if it
// successfully sets all the random variables and objects to valid values;
// otherwise, it returns 0." 18.6.3 L30471-30472: "If randomize() fails, the
// constraints are infeasible, and the random variables retain their previous
// values."
// Decision (llg choice; the text only gives the return value): infeasible
// constraints are not a run-time error. The call returns 0, a warning is
// printed on stderr, the variables keep their values and the run continues
// with exit status 0. (Evaluation errors, S29-D1, and an exhausted solver
// budget are run-time errors: the run ends with exit status 1.)
class item_c;
  rand bit [3:0] a;
  constraint c { a > 4'd5; a < 4'd3; }
endclass
module tb;
  initial begin
    static item_c item = new;
    int ok;
    item.a = 4'd4;
    ok = item.randomize();
    $display("ok=%0d a=%0d", ok, item.a);
    ok = item.randomize() with { a == 4'd1; };
    $display("still running ok=%0d a=%0d", ok, item.a);
    $finish;
  end
endmodule
