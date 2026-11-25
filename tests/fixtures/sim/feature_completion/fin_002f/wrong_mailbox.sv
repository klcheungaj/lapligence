// Messages of every type read into variables of every type: `+` the message
// was read, `-` another type (15.4.6). A refused message stays in the mailbox
// and the variable keeps its value.
`timescale 1ns/1ns
`define GET(VARIABLE) \
    r = m.try_get(VARIABLE); \
    $write("%s", r > 0 ? "+" : r < 0 ? "-" : "0"); \
    if (r < 0 && m.num() != 1) $write("!"); \
    if (r > 0) void'(m.try_put(0)); \
    void'(m.try_get(drop)); \
    m = new;
`define ROW(NAME, VALUE) \
    $write("%-10s ", NAME); \
    for (int k = 0; k < 19; k++) begin \
      m.put(VALUE); \
      get(k); \
    end \
    $display;

module tb;
  typedef enum {A, B} e_t;
  typedef enum {C, D} f_t;
  typedef struct {
    int a;
    string s;
  } st_t;
  typedef struct {
    int a;
    string s;
  } st2_t;
  typedef struct packed {logic [3:0] h, l;} ps_t;
  class base;
    int v = 3;
  endclass
  class oth;
  endclass
  mailbox m = new;
  int r, drop;
  int i;
  byte b;
  int unsigned iu;
  logic [31:0] l32;
  bit [31:0] b32;
  integer ig;
  longint li;
  time tm;
  real re;
  shortreal sr;
  string s;
  e_t e;
  f_t f;
  st_t st;
  st2_t st2;
  ps_t ps;
  logic [7:0] l8;
  bit signed [7:0] sb;
  base bh;
  oth oh;

  task get(int k);
    case (k)
      0: begin `GET(i) end
      1: begin `GET(b) end
      2: begin `GET(iu) end
      3: begin `GET(l32) end
      4: begin `GET(b32) end
      5: begin `GET(ig) end
      6: begin `GET(li) end
      7: begin `GET(tm) end
      8: begin `GET(re) end
      9: begin `GET(sr) end
      10: begin `GET(s) end
      11: begin `GET(e) end
      12: begin `GET(f) end
      13: begin `GET(st) end
      14: begin `GET(st2) end
      15: begin `GET(ps) end
      16: begin `GET(l8) end
      17: begin `GET(sb) end
      18: begin `GET(oh) end
    endcase
  endtask

  initial begin
    $display("           i b iu l32 b32 ig li tm re sr s e f st st2 ps l8 sb oh");
    bh = new;
    oh = new;
    `ROW("int", i)
    `ROW("byte", b)
    `ROW("unsigned", iu)
    `ROW("logic32", l32)
    `ROW("bit32", b32)
    `ROW("integer", ig)
    `ROW("longint", li)
    `ROW("time", tm)
    `ROW("real", re)
    `ROW("shortreal", sr)
    `ROW("string", s)
    `ROW("enum 1", e)
    `ROW("enum 2", f)
    `ROW("struct 1", st)
    `ROW("struct 2", st2)
    `ROW("packed", ps)
    `ROW("logic8", l8)
    `ROW("signed8", sb)
    `ROW("base", bh)
    `ROW("other", oh)
    $finish;
  end
endmodule
