module tb;
  typedef enum logic [1:0] {IDLE, RUN, DONE} state_t;
  typedef struct packed { logic [3:0] hi; bit [3:0] lo; } pk_t;
  typedef struct packed { pk_t inner; logic signed [7:0] s; state_t st; } nest_t;
  typedef union packed { logic [7:0] w; pk_t p; } pu_t;
  typedef struct { int a; logic [7:0] b; real r; pk_t p; } up_t;
  typedef struct { int a; logic [7:0] b; } ui_t;

  nest_t ns;
  pu_t pu;
  up_t up;
  ui_t uarr [1:0];
  logic [7:0] plain;

  initial begin
    ns = '0;
    ns.inner.hi = 4'ha;
    ns.s = -3;
    ns.st = DONE;
    pu = 8'h96;
    up.a = 7;
    up.b = 8'h5x;
    up.r = 1.5;
    up.p = 8'h3c;
    uarr[0] = '{a: 1, b: 2};
    uarr[1].a = 11;
    uarr[1].b = 8'h22;
    plain = 8'h0f;
    $members;
    #1;
    $display("hdl: ns=%h inner.lo=%h s=%0d pu=%h up.a=%0d up.p=%h uarr[1].b=%h",
             ns, ns.inner.lo, ns.s, pu, up.a, up.p, uarr[1].b);
    ns.inner.hi = 4'h1;
    #1;
    $finish;
  end
endmodule
