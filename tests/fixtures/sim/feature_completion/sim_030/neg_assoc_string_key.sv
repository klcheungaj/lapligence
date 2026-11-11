// A foreach index is an entry's key, which the solver holds as an integral
// value of at most 64 bits: a string index type rejects.
class c_c;
  rand bit [7:0] sa [string];
  constraint c { foreach (sa[s]) sa[s] < 4; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
