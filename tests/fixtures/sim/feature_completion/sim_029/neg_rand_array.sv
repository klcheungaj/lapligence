// Random arrays are planned for SIM-030.
class c_c; rand bit [3:0] v [4]; endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
