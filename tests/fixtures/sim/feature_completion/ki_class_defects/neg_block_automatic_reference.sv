// A variable of a named block in a class method is automatic (SV 8.6), so a
// hierarchical name cannot reach it (SV 6.21).
class C;
  task t();
    begin : fb
      int x;
      #1 x = 1;
    end
  endtask
  function int peek();
    return t.fb.x;
  endfunction
endclass

module tb;
  initial begin
    static C c = new;
    $display("%0d", c.peek());
  end
endmodule
