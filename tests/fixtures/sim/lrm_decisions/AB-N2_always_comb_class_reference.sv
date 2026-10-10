// AB-N2: an always_comb that reads a class property through a handle is not
// sensitive to the handle variable; always @* is.
//
// IEEE 1800-2009 9.2.2.2.1 (L11192-11194): "References to class objects and
// method calls of class objects do not add anything to the sensitivity list
// of an always_comb."
// 9.4.2.2 (L11899-11900): "All net and variable identifiers that appear in
// the statement will be automatically added to the event expression with the
// following exceptions: ..."
//
// Decision: in always_comb, `h.x` and `h.f()` add nothing, not even the
// handle variable `h`; a direct read of the handle (`h == null`) is an
// ordinary variable read and adds `h`. always @* keeps its own rule: the
// identifier `h` appears in the statement, so rebinding `h` wakes it, while
// writes to the object's property do not.
`timescale 1ns / 1ns
module tb;
  class C;
    int x;
    function int get();
      return x;
    endfunction
  endclass

  C h = new, h2 = new;
  int comb_prop, comb_call, star_prop, n_comb = 0, n_call = 0, n_star = 0, n_null = 0;
  bit is_null;

  always_comb begin
    comb_prop = h.x;
    n_comb++;
  end

  always_comb begin
    comb_call = h.get();
    n_call++;
  end

  always_comb begin
    is_null = h == null;
    n_null++;
  end

  always @* begin
    star_prop = h.x;
    n_star++;
  end

  initial begin
    h2.x = 7;
    #1 h.x = 2;
    #1 h = h2;
    #1 h2.x = 9;
    #1 $display("always_comb h.x: value %0d runs %0d", comb_prop, n_comb);
    $display("always_comb h.get(): value %0d runs %0d", comb_call, n_call);
    $display("always_comb h == null: runs %0d", n_null);
    $display("always @* h.x: value %0d runs %0d", star_prop, n_star);
    $finish(0);
  end
endmodule
