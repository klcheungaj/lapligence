// A randomize() in a wait condition would run again at every wake; it stays
// rejected, unlike a loop condition, which the program evaluates itself.
class c_c; rand bit [3:0] a; endclass
module tb;
  initial begin
    static c_c c = new;
    wait (c.randomize() == 1);
    $finish(0);
  end
endmodule
