// FIN-002 gate-level scenario 1: a standard-cell netlist (inverter, NAND,
// UDP multiplexer with state-dependent paths, UDP flip-flop with a
// notifier) with specify paths and timing checks, annotated from
// gl_cells_sdf.sdf, under a behavioural clock, stimulus and reference
// model. Setup, hold and width violations toggle the notifiers and the
// flip-flops go to x until the next clean capture. Every expected line is
// derived in readme.md.
`timescale 1ns / 1ps

primitive udp_mux2(y, a, b, s);
  output y;
  input a, b, s;
  table
    0 ? 0 : 0;
    1 ? 0 : 1;
    ? 0 1 : 0;
    ? 1 1 : 1;
    0 0 x : 0;
    1 1 x : 1;
  endtable
endprimitive

primitive udp_dff(q, d, clk, rn, notifier);
  output q;
  reg q;
  input d, clk, rn, notifier;
  table
    // d  clk  rn  ntf : q : q+
    0 (01) 1 ? : ? : 0;
    1 (01) 1 ? : ? : 1;
    0 (0x) 1 ? : 0 : 0;
    1 (0x) 1 ? : 1 : 1;
    ? (1?) 1 ? : ? : -;
    ? (x0) 1 ? : ? : -;
    * ? 1 ? : ? : -;
    ? ? 0 ? : ? : 0;
    ? ? (01) ? : ? : -;
    ? ? ? * : ? : x;
  endtable
endprimitive

module INVX1 (
    output Y,
    input  A
);
  not (Y, A);
  specify
    (A => Y) = (0.10, 0.12);
  endspecify
endmodule

module NAND2X1 (
    output Y,
    input  A,
    input  B
);
  nand (Y, A, B);
  specify
    (A => Y) = (0.15, 0.10);
    (B => Y) = (0.17, 0.11);
  endspecify
endmodule

module MUX2X1 (
    output Y,
    input  A,
    input  B,
    input  S
);
  udp_mux2 (Y, A, B, S);
  specify
    (A => Y) = 0.20;
    (B => Y) = 0.20;
    if (A == 1'b0 && B == 1'b1) (S => Y) = (0.30, 0.25);
    if (A == 1'b1 && B == 1'b0) (S => Y) = (0.25, 0.30);
    ifnone (S => Y) = 0.20;
  endspecify
endmodule

module DFFRX1 (
    output Q,
    output QN,
    input  D,
    input  CK,
    input  RN
);
  reg  notifier;
  wire q_int;
  udp_dff (q_int, D, CK, RN, notifier);
  buf (Q, q_int);
  not (QN, q_int);
  specify
    specparam tsu = 0.5, th = 0.3, tw = 1.0;
    (posedge CK => (Q +: D)) = (0.40, 0.45);
    (posedge CK => (QN -: D)) = (0.50, 0.50);
    (negedge RN => (Q +: 1'b0)) = 0.30;
    (negedge RN => (QN -: 1'b0)) = 0.35;
    $setup(D, posedge CK &&& RN, tsu, notifier);
    $hold(posedge CK &&& RN, D, th, notifier);
    $width(negedge RN, tw, 0, notifier);
  endspecify
endmodule

module core (
    output [1:0] q,
    output       zero,
    input        clk,
    input        rn,
    input        en,
    input  [1:0] d
);
  wire [1:0] dm;
  wire [1:0] qn;
  wire       nz;
  MUX2X1 m0 (
      .Y(dm[0]),
      .A(q[0]),
      .B(d[0]),
      .S(en)
  );
  MUX2X1 m1 (
      .Y(dm[1]),
      .A(q[1]),
      .B(d[1]),
      .S(en)
  );
  DFFRX1 f0 (
      .Q (q[0]),
      .QN(qn[0]),
      .D (dm[0]),
      .CK(clk),
      .RN(rn)
  );
  DFFRX1 f1 (
      .Q (q[1]),
      .QN(qn[1]),
      .D (dm[1]),
      .CK(clk),
      .RN(rn)
  );
  NAND2X1 g (
      .Y(nz),
      .A(qn[0]),
      .B(qn[1])
  );
  INVX1 i (
      .Y(zero),
      .A(nz)
  );
endmodule

module tb;
  reg        clk;
  reg        rn;
  reg        en;
  reg  [1:0] d;
  wire [1:0] q;
  wire       zero;
  reg  [1:0] model;

  core dut (
      .q(q),
      .zero(zero),
      .clk(clk),
      .rn(rn),
      .en(en),
      .d(d)
  );

  // CLI_SDF leaves the annotation to the command line.
`ifndef CLI_SDF
  initial $sdf_annotate("gl_cells_sdf.sdf", dut);
`endif

  initial begin
    clk = 1'b0;
    forever #5 clk = ~clk;
  end

  // The behavioural model has no delays and no timing checks.
  always @(posedge clk or negedge rn)
    if (!rn) model <= 2'b00;
    else if (en) model <= d;

  always @(q or zero) $display("[%0.3f] q=%b zero=%b", $realtime, q, zero);
  always @(dut.f0.notifier) $display("[%0.3f] f0 notifier", $realtime);
  always @(dut.f1.notifier) $display("[%0.3f] f1 notifier", $realtime);
  always @(negedge clk)
    if ($time > 0) $display("[%0.3f] sample q=%b model=%b same=%b", $realtime, q, model, q === model);

  initial begin
    rn = 1'b1;
    en = 1'b0;
    d  = 2'b00;
    #1 rn = 1'b0;
    #2 rn = 1'b1;
    #5 en = 1'b1;
    d = 2'b11;
    // 24.700: 0.050 before the edge at the flip-flop after the 0.250
    // multiplexer path; inside the annotated 0.600 setup limit of f0.
    #16.7 d = 2'b10;
    // 45.100: 0.350 after the edge at the flip-flop; inside the annotated
    // 0.450 hold limit of f1, outside its 0.300 specify limit.
    #20.4 d = 2'b00;
    // A reset pulse of 0.800, below the 1.000 width limit.
    #16.9 rn = 1'b0;
    #0.8 rn = 1'b1;
    #7.2 d = 2'b01;
    #20 en = 1'b0;
    #10 $finish;
  end
endmodule
