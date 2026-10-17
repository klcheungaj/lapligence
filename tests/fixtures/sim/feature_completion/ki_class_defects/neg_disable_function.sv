// A `disable` cannot name a function (SV 9.6.2), also inside a class.
class C;
  function int f();
    begin : fb
      int x;
      x = 1;
    end
    return 0;
  endfunction
  task t();
    disable f;
  endtask
endclass

module tb;
  initial begin
    static C c = new;
    c.t();
  end
endmodule
