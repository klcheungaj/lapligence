// Objects of many kinds that come and go, for vpi_handles.c.
`timescale 1ns/1ns
primitive udp_and(output y, input a, input b);
  table 0 ? : 0; ? 0 : 0; 1 1 : 1; endtable
endprimitive
primitive udp_dff(output reg q, input c, input d);
  initial q = 0;
  table (01) 0 : ? : 0; (01) 1 : ? : 1; (10) ? : ? : -; ? (??) : ? : -; endtable
endprimitive
package pk;
  typedef enum logic [1:0] {RED, GREEN, BLUE} color_t;
  typedef struct packed { logic [3:0] a; color_t c; } pair_t;
  typedef struct { int n; string s; logic [7:0] v[2]; } rec_t;
  int shared = 5;
  parameter int PW = 7;
  function automatic int twice(int x); return 2 * x; endfunction
endpackage
interface bus_if(input logic clk);
  logic req, gnt;
  logic [7:0] data;
  modport m(output req, data, input gnt);
  modport s(input req, data, output gnt);
  clocking cb @(posedge clk); input gnt; output req; endclocking
endinterface
module leaf #(parameter int W = 4, parameter string NAME = "leaf") (input wire clk, input wire [W-1:0] d, output wire [W-1:0] q, inout wire io);
  logic [W-1:0] qr;
  assign q = qr;
  localparam L = W * 2;
  specparam tp = 3;
  always_ff @(posedge clk) qr <= d;
  assign io = q[0] ? 1'bz : 1'b0;
  wire w1, w2, w3;
  and #2 g1(w1, d[0], clk);
  udp_and u1(w2, d[1], clk);
  udp_dff u2(w3, clk, d[0]);

  specify
    (clk *> q) = (tp, 4);
    $setup(d, posedge clk, 1);
  endspecify
endmodule
module tb;
  import pk::*;
  logic clk = 0;
  logic [3:0] d = 0;
  wire [3:0] q;
  wire io;
  logic [99:0] wide = 100'h123456789abcdef;
  bit [7:0] two;
  int i32 = -5;
  longint i64;
  shortint i16;
  byte i8;
  integer ig;
  time t;
  real r = 1.5;
  shortreal sr;
  realtime rt;
  string str = "hello";
  event ev;
  color_t col;
  pair_t pr;
  rec_t rec;
  union packed { logic [7:0] a; logic [7:0] b; } un;
  logic [7:0] mem[4][2];
  int dyn[];
  int que[$];
  string asoc[string];
  int iasoc[int];
  chandle ch;
  wire [7:0] nets[3];
  wire (strong0, weak1) st;
  supply0 gnd;
  tri1 pull;
  assign nets[0] = d;
  assign #1 st = d[1];
  leaf #(.W(4), .NAME("x")) u(.clk(clk), .d(d), .q(q), .io(io));
  leaf u2(.clk, .d(4'd3), .q(), .io());
  bus_if bi(clk);
  virtual bus_if vif = bi;
  class base;
    int x = 1;
    static int count;
    string n = "b";
    rand bit [3:0] rb;
    base next;
    constraint c { rb < 9; }
    function new(); count++; endfunction
    virtual function int get(int a); return x + a; endfunction
    task wait_a_bit(input int k); int loc; loc = k; #1 x += loc; endtask
  endclass
  class derived extends base;
    int y[$];
    covergroup cg; coverpoint x { bins lo = {[0:3]}; bins hi = {[4:9]}; } endgroup
    function new(); super.new(); cg = new; y = '{1, 2}; endfunction
    virtual function int get(int a); return super.get(a) * 2; endfunction
  endclass
  base b, b2;
  derived dd;
  mailbox #(int) mb = new(2);
  semaphore sem = new(1);
  process p;
  covergroup cgt @(posedge clk); cp: coverpoint d { bins z = {0}; bins o = {[1:15]}; } endgroup
  cgt cgi = new;
  always #5 clk = ~clk;
  always @(posedge clk) begin d <= d + 1; two++; end
  function automatic int fact(int n); int res; if (n <= 1) res = 1; else res = n * fact(n - 1); return res; endfunction
  task automatic auto_t(input int n, output int o);
    int l1; logic [3:0] l2;
    l1 = n; l2 = n[3:0];
    begin : in_auto int deep; deep = l1 * 2; #1 o = deep; end
    $step(2);
  endtask
  task stat_t(input int n); int s1; s1 = n; #1 $step(1); endtask
  ap: assert property (@(posedge clk) d != 15 |-> ##1 1);
  cvp: cover property (@(posedge clk) d == 3 ##1 d == 4);
  sequence sq; @(posedge clk) d[0] ##1 d[1]; endsequence
  for (genvar gi = 0; gi < 2; gi++) begin : g
    int gv = gi;
    if (gi == 1) begin : inner logic [gi:0] z; assign z = d[1:0]; end
  end
  initial begin : main
    int o;
    b = new; dd = new; b2 = dd; b.next = dd;
    dyn = new[4]; que = '{1, 2, 3}; asoc["a"] = "x"; iasoc[4] = 4; iasoc[9] = 9;
    rec.n = 3; rec.s = "rs";
    p = process::self();
    $step(5);
    fork
      begin : br1 #3 $step(2); end
      begin : br2 auto_t(3, o); end
      stat_t(4);
      b.wait_a_bit(2);
      forever begin #4; $step(1); if ($time > 60) break; end
    join_none
    #2 dyn = new[2]; que.delete(0); asoc.delete("a"); iasoc.delete(4);
    $step(3);
    void'(b.randomize());
    b = null; dd.y.push_back(7);
    #3 -> ev;
    i32 = fact(4);
    force two = 8'h3;
    #4 release two;
    $step(3);
    dyn = new[8](dyn); que.push_back(9); b2 = null; dd.next = dd;
    #7 $step(3);
    dd = null;
    #1 begin base junk; for (int k = 0; k < 50; k++) junk = new; end
    $step(3);
    disable br1;
    #20 dyn.delete(); que = '{}; iasoc.delete(); asoc.delete();
    $step(3);
    i32 = $value(3) + $value(i32);
    #30 $finish;
  end
  final begin $step(2); $display("ends"); end
endmodule
