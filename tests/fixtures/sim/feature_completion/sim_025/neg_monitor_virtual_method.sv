// SIM-025: a virtual method call in a `$monitor` argument. The Postponed
// evaluator does not dispatch virtual methods yet, so the design rejects.
module tb;
  class B;
    int v;
    virtual function int get();
      return v;
    endfunction
  endclass
  class D extends B;
    virtual function int get();
      return v + 1;
    endfunction
  endclass
  B h;
  D d;
  initial begin
    d = new;
    h = d;
    $monitor("%0t get=%0d", $time, h.get());
    #1 d.v = 1;
    #1 $finish(0);
  end
endmodule
