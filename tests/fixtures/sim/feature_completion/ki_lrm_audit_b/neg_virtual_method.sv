// A virtual method in a wait condition: the implementation it runs, and so
// the storage it reads, is chosen at run time.
module tb;
  class C;
    int x;
    virtual function int get();
      return x;
    endfunction
  endclass
  C h = new;
  initial begin
    wait (h.get() == 1);
    $display("woke");
  end
  initial #1 h.x = 1;
endmodule
