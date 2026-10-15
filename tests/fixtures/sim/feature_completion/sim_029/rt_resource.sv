// A satisfiable problem whose budgets are forced to run out: the outcome is a
// solver-resource error, distinct from an unsatisfiable problem (A03).
class f_c;
  rand bit [11:0] x, y;
  constraint c { x * y == 12'd391; x > 1; y > 1; }
endclass
class u_c;
  rand bit [3:0] a;
  constraint c { a > 4'd15; }
endclass
module tb;
  initial begin
    static f_c f = new;
    static u_c u = new;
    int ok;
    ok = u.randomize();
    $display("unsat ok=%0d", ok);
    ok = f.randomize();
    $display("resource ok=%0d x=%0d y=%0d", ok, f.x, f.y);
    $finish;
  end
endmodule
