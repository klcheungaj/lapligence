// SIM-030 A03: an executable witness of every IEEE 1800-2009 constraint
// production (A.1.10) and of the randomize_call forms (A.8.2). Each value
// printed is forced by the constraints.
virtual class shape_c;
  rand bit [3:0] w;
  // constraint_prototype with the pure qualifier
  pure constraint sized;
endclass

class box_c extends shape_c;
  static rand bit [3:0] s;
  rand bit [3:0] x, y, d;
  rand bit [3:0] a [3];
  rand bit       g;
  // constraint_declaration
  constraint sized { w == 4'd3; }
  // constraint_prototype with the extern qualifier, and without a qualifier
  extern constraint ext_c;
  constraint proto_c;
  // [ static ] constraint_declaration
  static constraint st_c { s == 4'd5; }
  // solve_before_list with implicit_class_handle, class_scope and plain
  // primaries
  constraint ord_c { solve this.g, box_c::s before d; solve g before x; }
  // expression_or_dist: an expression and a dist with := and :/ weights over
  // a value and a value_range
  constraint dist_c { d dist { 4'd7 := 1, [4'd8:4'd9] :/ 0 }; }
  // expression -> constraint_set, if/else with constraint sets and foreach
  constraint set_c {
    g -> { x == 4'd2; }
    if (g) { y == 4'd1; } else { y == 4'd2; }
    foreach (a[i]) { a[i] == i; }
  }
endclass

// extern_constraint_declaration, also for a prototype without a qualifier
constraint box_c::ext_c { g == 1'b1; }
constraint box_c::proto_c { y != x; }

module tb;
  box_c b;
  int x, ok;
  bit [3:0] v;

  initial begin
    b = new;
    // randomize_call: the plain form
    ok = b.randomize();
    $display("plain: ok=%0d w=%0d s=%0d x=%0d y=%0d d=%0d a=%p", ok, b.w, b.s, b.x, b.y, b.d, b.a);
    // randomize_call: with a variable_identifier_list and with null
    ok = b.randomize(x) with { x == 4'd2; };
    $display("list: ok=%0d x=%0d", ok, b.x);
    $display("null: ok=%0d", b.randomize(null));
    // randomize_call: with ( identifier_list ) restricts name lookup in the
    // object to the listed names; x is the module variable (18.7)
    x = 2;
    $display("restricted: ok=%0d x=%0d", b.randomize() with (x) { x == local::x; }, b.x);
    // std::randomize with a constraint block (18.12)
    $display("scope: ok=%0d v=%0d", std::randomize(v) with { v inside {[4'd6:4'd6]}; }, v);
    $finish;
  end
endmodule
