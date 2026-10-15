// A03: `default` dist items are later-edition syntax, rejected under sv2009.
class c_c; rand bit [3:0] x; constraint d { x dist { 1 := 1, default :/ 2 }; } endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
