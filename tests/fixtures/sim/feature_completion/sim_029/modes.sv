// SIM-029: rand_mode() and constraint_mode() (IEEE 1800-2009 18.8, 18.9,
// 18.5.10): task and function forms, whole-object forms through a base
// handle, and static members shared by every instance.
class base_c;
  rand bit [7:0] a;
  rand bit [7:0] b;
  static rand bit [3:0] s;
  constraint a_c { a < 10; }
  constraint b_c { b > 250; }
  static constraint s_c { s == 5; }
endclass

class derived_c extends base_c;
  rand bit [7:0] d;
  constraint a_c { a > 245; }       // overrides base_c::a_c
  constraint d_c { d == 77; }
endclass

module tb;
  initial begin
    static base_c x = new, y = new;
    static derived_c dv = new;
    static base_c bh = dv;
    int ok;

    $display("initial modes: %0d %0d %0d", x.a.rand_mode(), x.a_c.constraint_mode(), x.s.rand_mode());
    x.a = 123;
    x.a.rand_mode(0);
    // An inactive variable is state: a_c (a < 10) now fails (18.8).
    ok = x.randomize();
    $display("a off: ok=%0d a=%0d mode=%0d b=%0d", ok, x.a, x.a.rand_mode(), x.b);
    x.a_c.constraint_mode(0);
    ok = x.randomize() with { a == 123 || a < 10; };
    $display("a off, a_c off: ok=%0d a=%0d", ok, x.a);
    x.a.rand_mode(1);
    x.b_c.constraint_mode(0);
    ok = x.randomize() with { a == 200; b == 3; };
    $display("blocks off: ok=%0d a=%0d b=%0d cmode=%0d", ok, x.a, x.b, x.a_c.constraint_mode());
    x.constraint_mode(1);
    ok = x.randomize();
    $display("blocks on: ok=%0d a_ok=%0d b_ok=%0d", ok, x.a < 10, x.b > 250);

    // Static members: one mode bit and one value for every instance.
    y.s_c.constraint_mode(0);
    $display("static block mode seen from x: %0d", x.s_c.constraint_mode());
    x.s_c.constraint_mode(1);
    y.s.rand_mode(0);
    base_c::s = 9;
    ok = x.randomize();
    $display("static rand off: ok=%0d s=%0d mode=%0d", ok, base_c::s, x.s.rand_mode());
    x.s_c.constraint_mode(0);
    ok = x.randomize() with { s == 9; };
    $display("static state, block off: ok=%0d s=%0d", ok, base_c::s);
    x.s_c.constraint_mode(1);
    y.s.rand_mode(1);
    ok = y.randomize();
    $display("static rand on: ok=%0d s=%0d", ok, base_c::s);

    // Whole-object task forms reach every member of the dynamic class.
    dv.a = 250; dv.b = 255; dv.d = 77; base_c::s = 5;
    bh.rand_mode(0);
    ok = bh.randomize();
    $display("all off: ok=%0d a=%0d b=%0d d=%0d dmode=%0d", ok, dv.a, dv.b, dv.d, dv.d.rand_mode());
    dv.d = 1;
    ok = bh.randomize();
    $display("all off, d_c fails: ok=%0d d=%0d", ok, dv.d);
    bh.rand_mode(1);
    bh.constraint_mode(0);
    ok = dv.randomize() with { d == 3; a == 4; };
    $display("all blocks off: ok=%0d d=%0d a=%0d", ok, dv.d, dv.a);
    bh.constraint_mode(1);
    ok = bh.randomize();
    $display("override active: ok=%0d a_ok=%0d d=%0d", ok, dv.a > 245, dv.d);
    bh.a_c.constraint_mode(0);   // the block in effect: derived_c::a_c
    ok = bh.randomize() with { a == 1; };
    $display("override off: ok=%0d a=%0d", ok, dv.a);
    $finish;
  end
endmodule
