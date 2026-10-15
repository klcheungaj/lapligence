// dist is planned for SIM-030 and must not be dropped.
class c_c; rand bit [3:0] r; constraint d { r dist { 0 := 1, [1:15] :/ 1 }; } endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
