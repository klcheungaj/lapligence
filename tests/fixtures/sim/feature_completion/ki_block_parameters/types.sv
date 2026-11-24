`timescale 1ns/1ps
module tb;
  initial begin : b
    typedef enum logic [2:0] { RED = 2, GREEN, BLUE } col_t;
    typedef struct packed { logic [3:0] a; logic [3:0] b; } st_t;
    typedef union packed { logic [7:0] w; st_t s; } un_t;
    localparam col_t C = GREEN;
    localparam st_t ST = '{a: 4'h3, b: 4'h9};
    localparam un_t UN = 8'h5a;
    localparam type T = logic [11:0];
    parameter type T2 = int;
    localparam int A [3] = '{4, 5, 6};
    localparam logic [1:0][3:0] PA = 8'h7c;
    enum {X1, X2 = 5, X3} ev;
    T t; T2 t2; col_t c;
    int i;
    t = '1; t2 = -1; c = C; ev = X3; i = 1;
    $display("%0d %s %h %h %h", C, C.name(), ST, ST.a, ST.b);
    $display("%h %h %h", UN, UN.w, UN.s.b);
    $display("%0d %0d %0d %s", $bits(t), t2, ev, ev.name());
    $display("%0d %0d %0d", A[0], A[i], A[2]);
    $display("%h %h", PA[1], PA[i]);
    $display("%p", A);
    c = c.next(); $display("%s", c.name());
    foreach (A[k]) $display("A[%0d]=%0d", k, A[k]);
  end
endmodule
