`timescale 1ns/1ps
module c2;
  typedef struct { int a; logic [7:0] b; } t;
  parameter t US = '{1, 8'hab};
  parameter t UA [2] = '{'{1, 8'h2}, '{3, 8'h4}};
  t v; int i = 1;
  initial begin #0;
    $display("%0d %0h", US.a, US.b);
    v = US; $display("%0d %0h", v.a, v.b);
    $display("%0d %0d", UA[1].a, UA[0].b);
    $display("%0d %0d", UA[i].a, UA[i].b);
    v = UA[i]; $display("%0d %0d", v.a, v.b);
  end
endmodule
module c3;
  typedef struct { int a; string s; } t;
  parameter t UA [2] = '{'{1, "p"}, '{3, "q"}};
  int i = 1;
  initial begin #10;
    $display("%0d %s", UA[1].a, UA[0].s);
    $display("%0d %s", UA[i].a, UA[i].s);
  end
endmodule
module c4;
  typedef struct { int a; int arr [3]; struct { real r; int q; } in; } t;
  parameter t N = '{1, '{2,3,4}, '{1.5, 7}};
  int i = 2;
  initial begin #20;
    $display("%0d %0d %0.2f %0d", N.a, N.arr[1], N.in.r, N.in.q);
    $display("%0d", N.arr[i]);
  end
endmodule
module g3;
  typedef struct { logic [7:0] arr [2]; string s [2]; real r; } t;
  parameter t TT = '{'{8'h1, 8'h2}, '{"x", "y"}, 4.5};
  parameter t TA [2] = '{'{'{8'h1, 8'h2}, '{"x", "y"}, 4.5}, '{'{8'h3, 8'h4}, '{"z", "w"}, 5.5}};
  int i = 1;
  initial begin #30;
    $display("%0d %s %0.1f", TT.arr[i], TT.s[i], TT.r);
    $display("%0d %s %0.1f", TT.arr[0], TT.s[0], TA[i].r);
    $display("%0d %s", TA[i].arr[0], TA[i].s[1]);
    $display("%0d %s", TA[1].arr[i], TA[0].s[i]);
  end
endmodule
module tb;
  c2 u_c2 ();
  c3 u_c3 ();
  c4 u_c4 ();
  g3 u_g3 ();
endmodule
