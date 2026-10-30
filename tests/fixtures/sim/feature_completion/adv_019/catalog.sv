// ADV-019 catalog walk (A03): one design with an object of every kind the
// VPI catalog lists as reachable. catalog.c walks it from the top modules
// and queries every catalogued property and relation on every object kind
// it reaches; the Rust test compares the answers with
// src/sim/rt/vpi_catalog.txt.
`timescale 1ns/1ps
// Packages and `$unit` are top-level instances (37.10).
package cat_pkg;
  int pkv = 1;
  parameter int PKP = 2;
  event pke;
  function automatic int pkf(int a);
    return a + PKP;
  endfunction
  task pkt;
    -> pke;
  endtask
endpackage

int unit_v = 3;

// A program instance and a program array (37.9, 37.11). The initial
// procedure outlives the walk, so the programs do not end the run.
program prog_unit (input logic clk);
  int pgv = 0;
  default clocking pcb @(negedge clk);
    input #1 pgv;
  endclocking
  initial #1000 pgv = cat_pkg::pkv + unit_v;
endprogram

primitive udp_and (out, a, b);
  output out;
  input a, b;
  table
    0 ? : 0;
    ? 0 : 0;
    1 1 : 1;
  endtable
endprimitive

interface bus_if (input logic clk);
  logic [7:0] data;
  modport src (output data, input clk);
  clocking icb @(posedge clk);
    input data;
  endclocking
  task automatic ping;
  endtask
endinterface

module leaf (input logic [3:0] a, output logic [3:0] y);
  assign y = ~a;
endmodule

module sink (bus_if.src b);
  always_ff @(posedge b.clk) b.data <= b.data + 8'd1;
endmodule

module dut (input a, input clk, input d, output y);
  reg notifier;
  buf ib (y, a);
  specify
    specparam tsu = 2;
    (a => y) = (3, 4);
    $setup(d, posedge clk, 2, notifier);
  endspecify
endmodule

// An FSM (40.5.3) through the state-vector pragma of decision A17-D11.
module fsm_unit (input logic clk);
  // tool state_vector st enum ST
  logic [1:0] st = 2'd0;
  parameter [1:0] /* tool enum ST */ IDLE = 0, BUSY = 1;
  always @(posedge clk) st <= st == IDLE ? BUSY : IDLE;
endmodule

module tb;
  typedef enum bit [1:0] {red, green, blue} color_t;
  typedef struct packed { logic [3:0] hi; bit [3:0] lo; } pk_t;
  typedef union packed { logic [7:0] w; pk_t p; } pu_t;
  typedef struct { int a; real r; } rec_t;
  typedef pk_t [1:0] pka_t;
  class Packet;
    int id;
    function new(int i);
      id = i;
    endfunction
  endclass

  parameter int P = 5;
  localparam logic [3:0] LP = 4'h9;
  logic clk = 1'b0;
  logic [3:0] r0 = 4'h3;
  logic sc = 1'b1;
  bit [7:0] bv = 8'h0f;
  int iv = 1;
  integer ig = 2;
  shortint si = 3;
  longint li = 4;
  byte by = 5;
  time tv = 6;
  real rl = 1.5;
  shortreal sr = 2.5;
  color_t c = green;
  pk_t pk = 8'h12;
  pu_t pu = 8'h34;
  rec_t rec;
  logic [1:0][3:0] pa = 8'h56;
  pka_t pka = 16'h789a;
  logic [3:0] mem [0:3];
  string s = "hi";
  chandle ch = null;
  Packet p;
  int da[];
  int q[$];
  int aa[int];
  event ev;
  wire [3:0] w;
  wire w1, g_out, u_out, y, a_in, d_in;
  wire [3:0] lw;
  wire [1:0] na;
  wire sw_a, sw_b;
  wire [1:0] wa [0:1];
  wire pk_t sn;
  bus_if bi (clk);
  bus_if bia [1:0] (clk);
  leaf u [1:0] (.a(r0), .y());
  leaf single (.a(r0), .y(lw));
  sink sk (.b(bi));
  dut dt (.a(a_in), .clk(clk), .d(d_in), .y(y));
  fsm_unit fu (.clk(clk));
  prog_unit pg (.clk(clk));
  prog_unit pga [1:0] (.clk(clk));
  and g1 (g_out, r0[0], r0[1]);
  udp_and ud (u_out, r0[2], r0[3]);
  nand ng [1:0] (na, r0[1:0], r0[3:2]);
  tran tr (sw_a, sw_b);
  assign w = r0;
  assign w1 = r0[0];
  assign a_in = sc;
  assign d_in = sc;
  assign wa[0] = r0[1:0];
  assign sn = pk;

  // Clocking blocks (37.44): io declarations with own and default skews,
  // an expression clockvar, declarations, the default and global clocking
  // and the default disable condition (37.5).
  logic rst = 1'b0;
  logic co;
  default disable iff (rst);
  clocking cb @(posedge clk);
    default input #1step output #2;
    input r0;
    output negedge #3 co;
    inout sc;
    input #1 rlo = r0[1:0];
    property cbp;
      r0 != 4'hf;
    endproperty
  endclocking
  default clocking cb;
  global clocking gcb @(posedge clk); endclocking

  for (genvar gi = 0; gi < 2; gi++) begin : gen
    logic [1:0] gl;
  end

  sequence s_ok;
    r0 != 4'hf;
  endsequence
  sequence s_eq(x);
    r0 == x;
  endsequence
  e_ok: cover property (@(posedge clk) s_eq(4'h5));
  property p_ok;
    @(posedge clk) s_ok;
  endproperty
  a_ok: assert property (p_ok);
  c_ok: cover property (@(posedge clk) r0 == 4'h4);
  m_ok: assume property (@(posedge clk) r0 != 4'he);

  function automatic int inc(input int v);
    return v + 1;
  endfunction
  task fire;
    -> ev;
  endtask
  task automatic work(input int n);
    int k;
    for (int i = 0; i < n; i++) k += i;
    case (k)
      0: k = 1;
      default: k = 2;
    endcase
    while (k > 3) k--;
    repeat (2) k++;
    fork : par
      k = k + 1;
    join
    @(posedge clk);
    do k--; while (k > 5);
    foreach (mem[j]) k += j;
    wait (k > 0);
    k = r0[1:0] + r0[1] + r0[k[0] +: 1] + $clog2(n) + pka[1].hi;
    force w1 = 1'b0;
    release w1;
    fork
      k++;
    join_none
    wait fork;
    disable fork;
    assume (k >= 0);
    cover (k >= 0);
    if (k == 0) disable work;
    else k = -k;
  endtask

  always @(posedge clk) r0 <= r0 + 4'd1;
  always #5 clk = ~clk;
  initial forever @(posedge clk) iv = iv + $catalog_value();

  initial begin : main
    p = new(7);
    da = new[2];
    q.push_back(1);
    aa[3] = 4;
    rec.a = 1;
    mem[1] = 4'h2;
    assert (iv == 1);
    begin
      static int ub = 0;
      ub = ub + iv;
    end
    fork
      static int fb = 0;
      fb = fb + 1;
    join
    if (iv > 0) iv = inc(iv);
    #1 $catalog;
    $display("%s %0d %0d %0d %0d %0d %0d", s, ch == null, rec.a, da.size(), q.size(),
             aa[3], p.id);
    work(2);
    fire;
    $finish;
  end
endmodule
