// A recursive production saves its arguments and implicit variables for
// each active generation; llg saves packed, real and string values only.
module tb;
  class Item;
    int v;
  endclass
  initial begin
    Item it = new;
    randsequence (top)
      top : walk(it, 3) ;
      walk (Item h, int n) : if (n > 0) step(h, n) ;
      step (Item h, int n) : { h.v++; } walk(h, n - 1) ;
    endsequence
  end
endmodule
