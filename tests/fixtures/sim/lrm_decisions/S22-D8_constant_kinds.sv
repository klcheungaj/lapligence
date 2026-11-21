// Decision S22-D8: constant initializers of every kind hold their value from
// the start of their scope's initialization, as industry-standard simulators
// preset them: a function called by the scope's first initializer reads
// structure member defaults, a structure pattern with member keys, a
// `default:` array pattern, an enumeration constant, a real, a `const`
// variable, a `'1` fill, a packed structure pattern and a four-state literal.
// Every variable read early is written afterwards, so the result does not
// depend on a simulator treating a never-written variable as a constant.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". The text leaves the order among static
// initializers open.
module tb;
  typedef struct { int a = 5; int b = 6; } d_t;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } p_t;
  typedef enum int {RED = 4, BLUE = 9} color_t;
  int z = rd();
  d_t s;
  d_t s2 = '{a: 1, b: 2};
  int arr[3] = '{default: 7};
  color_t c = BLUE;
  real r = 2.5;
  const int ci = 3;
  bit [7:0] bb = '1;
  p_t pp = '{4'h3, 4'h4};
  logic [7:0] lv = 8'hzx;
  function int rd();
    $display("rd: s=%0d,%0d s2=%0d,%0d arr=%0d c=%0d r=%0.1f ci=%0d bb=%h pp=%h lv=%h",
             tb.s.a, tb.s.b, tb.s2.a, tb.s2.b, tb.arr[2], tb.c, tb.r, tb.ci, tb.bb, tb.pp, tb.lv);
    return 1;
  endfunction
  initial begin
    s.a = 0; s2.a = 0; arr[2] = 0; c = RED; r = 0.0; bb = 0; pp = 0; lv = 0;
    $finish;
  end
endmodule
