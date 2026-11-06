// SIM-025 A03: a class method's formals and locals are automatic, so a deferred
// report cannot name them (SV 13.3.2). Its properties are not (class_method_reports.sv).
module tb;
  class C;
    int x;
    function void f(int a);
      $strobe("x=%0d a=%0d", x, a);
    endfunction
  endclass
  C c;
  initial begin
    c = new;
    c.f(1);
    #1 $finish(0);
  end
endmodule
