// Randomize keeps the entries of an associative array (18.4 constrains the
// size of dynamic arrays and queues only), so its size in a constraint
// rejects instead of failing or resizing.
class c_c;
  rand bit [7:0] aa [int];
  constraint c { aa.size() == 3; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
