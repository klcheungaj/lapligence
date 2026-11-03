// ADV-019 frame automatics of structure type (decision A19-D76). A packed
// structure formal or local is listed with its value; an unpacked structure
// is listed as a vpiStructVar whose size is its number of fields and which
// has no value (37.17 details 9 and 11). Neither breaks a model built with
// or without the VPI catalog.
module tb;
  typedef struct { int a; logic [99:0] w; string s; } rec_t;
  typedef struct packed { logic [3:0] h; logic [3:0] l; } pk_t;
  int g;
  task automatic t(input rec_t ri, input pk_t pi);
    rec_t rl = ri;
    pk_t pl = pi;
    rl.a = rl.a + 1;
    $frames(1);
    g = rl.a + pl.l;
  endtask
  initial begin
    rec_t r;
    r.a = 5;
    r.w = 100'h1;
    r.s = "x";
    t(r, 8'h3c);
    $display("g=%0d", g);
    $finish;
  end
endmodule
