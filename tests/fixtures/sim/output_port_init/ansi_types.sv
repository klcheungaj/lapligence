// ANSI output variable port initializers of real, enum, struct, string,
// integer and unpacked array types.
typedef enum logic [1:0] {A, B, C} e_t;
typedef struct packed { logic [3:0] hi; logic [3:0] lo; } s_t;

module child #(parameter int W = 4, parameter logic [W-1:0] K = '1)
  (output real r = 2.5, output e_t e = C, output s_t s = '{hi: 4'h3, lo: 4'h4},
   output logic [W-1:0] k = K, output string str = "hi", output integer i = -3,
   output logic [3:0] ua [2] = '{4'h1, 4'h2});
endmodule

module tb;
  real pr;
  e_t pe;
  s_t ps;
  logic [5:0] pk;
  string pstr;
  integer pi;
  logic [3:0] pua [2];
  child #(.W(6)) c(.r(pr), .e(pe), .s(ps), .k(pk), .str(pstr), .i(pi), .ua(pua));
  initial #1 begin
    $display("r=%f e=%s s=%h k=%b str=%s i=%0d ua=%h,%h", pr, pe.name(), ps, pk, pstr, pi,
             pua[0], pua[1]);
    $finish;
  end
endmodule
