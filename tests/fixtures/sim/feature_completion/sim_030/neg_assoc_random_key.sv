// A select of an associative array names an entry by its key, so the key
// cannot be a random variable: the solver would have to choose the entry.
class c_c;
  rand bit [7:0] aa [int];
  rand bit [1:0] x;
  constraint c { aa[x] == 1; }
endclass
module tb;
  initial begin
    static c_c c = new;
    c.aa[0] = 0;
    void'(c.randomize());
    $finish;
  end
endmodule
