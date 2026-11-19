// A prefix increment of an associative array property of an object held in
// an associative array, inside an expression: a located limit
// (docs/known_issues.md, UVM example differences).
module tb;
  class counter;
    int count[string];
  endclass
  counter t[string];
  initial begin
    int v;
    t["a"] = new;
    t["a"].count["x"] = 4;
    v = ++t["a"].count["x"];
    $display("%0d %0d", v, t["a"].count["x"]);
    $finish(0);
  end
endmodule
