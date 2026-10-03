// SV2009 13.5.3-13.5.4: named and default arguments with fixed aggregate
// formals; a default is evaluated only when its actual is omitted.
package pkg;
  typedef logic [7:0] arr_t [0:2];
  typedef struct { logic [3:0] a; logic [3:0] b; } rec_t;
  localparam arr_t DEF = '{8'h11, 8'h22, 8'h33};
  int defaults_used = 0;
  function automatic rec_t note_default();
    defaults_used++;
    return '{4'h1, 4'h2};
  endfunction
  function automatic int f(input arr_t x = DEF, input rec_t r = note_default(), input int k = 1);
    return x[0] + x[2] + r.a * 16 + r.b + k;
  endfunction
endpackage

module tb;
  import pkg::*;
  function automatic void scale(output arr_t o, input arr_t src = DEF, input int by = 2);
    foreach (o[i]) o[i] = src[i] * by;
  endfunction
  arr_t a, b;
  initial begin
    a = '{8'h01, 8'h02, 8'h03};
    $display("all_defaults %0d", f());
    $display("named_tail %0d", f(.k(10)));
    $display("named_reordered %0d", f(.r('{4'h3, 4'h4}), .x(a)));
    $display("positional_gap %0d", pkg::f(a, , 0));
    $display("defaults_used %0d", defaults_used);
    scale(b);
    $display("output_default %h %h %h", b[0], b[1], b[2]);
    scale(.by(3), .o(b), .src(a));
    $display("output_named %h %h %h", b[0], b[1], b[2]);
    $finish(0);
  end
endmodule
