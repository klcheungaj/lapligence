// S30-D3: randc variables wider than 16 bits are rejected (18.4.2 permits a
// limit of at least 8 bits).
class c_c; randc bit [16:0] r; endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
