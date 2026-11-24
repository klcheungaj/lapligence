// FIN-002 gate-level scenario 5: an array of scan flip-flop cells (UDP
// multiplexer, sequential UDP with a notifier, edge-sensitive path and
// conditioned setup checks) and generated majority cells (combinational
// UDP with a multiple-source path), annotated from gl_scan_chain.sdf. A
// behavioural clock shifts a pattern in, captures and shifts out beside a
// reference model; a late scan input makes one flip-flop x and the x
// travels down the chain and through the majority cells. A VPI application
// (gl_scan_chain_vpi.c) inspects the primitives, paths and timing checks of
// three cell instances. Every expected line is derived in readme.md.
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

primitive udp_dff(q, d, clk, notifier);
  output q;
  reg q;
  input d, clk, notifier;
  table
    // d clk ntf : q : q+
    0 (01) ? : ? : 0;
    1 (01) ? : ? : 1;
    x (01) ? : ? : x;
    ? (1?) ? : ? : -;
    ? (?0) ? : ? : -;
    * ? ? : ? : -;
    ? ? * : ? : x;
  endtable
endprimitive

primitive udp_maj3(y, a, b, c);
  output y;
  input a, b, c;
  table
    1 1 ? : 1;
    1 ? 1 : 1;
    ? 1 1 : 1;
    0 0 ? : 0;
    0 ? 0 : 0;
    ? 0 0 : 0;
  endtable
endprimitive

module SDFFX1 (
    output Q,
    input  D,
    input  SI,
    input  SE,
    input  CK
);
  reg  notifier;
  wire dm;
  wire q_int;
  udp_mux2 m (dm, D, SI, SE);
  udp_dff f (q_int, dm, CK, notifier);
  buf b (Q, q_int);
  specify
    (posedge CK => (Q +: D)) = (0.30, 0.35);
    $setup(D, posedge CK &&& ~SE, 0.4, notifier);
    $setup(SI, posedge CK &&& SE, 0.4, notifier);
    $hold(posedge CK, D, 0.1, notifier);
  endspecify
endmodule

module MAJ3X1 (
    output Y,
    input  A,
    input  B,
    input  C
);
  udp_maj3 u (Y, A, B, C);
  specify
    (A, B, C *> Y) = (0.12, 0.14);
  endspecify
endmodule

module chain #(
    parameter N = 4
) (
    output [N-1:0] q,
    output         vote,
    input  [N-1:0] d,
    input          si,
    input          se,
    input          ck
);
  wire [N:0] s;
  assign s[0]   = si;
  assign s[N:1] = q;
  SDFFX1 ff[N-1:0] (
      .Q (q),
      .D (d),
      .SI(s[N-1:0]),
      .SE(se),
      .CK(ck)
  );
  genvar i;
  generate
    for (i = 0; i < N - 2; i = i + 1) begin : v
      wire y;
      MAJ3X1 m (
          .Y(y),
          .A(q[i]),
          .B(q[i+1]),
          .C(q[i+2])
      );
    end
  endgenerate
  or (vote, v[0].y, v[1].y);
endmodule

module tb;
  reg        ck;
  reg        se;
  reg        si;
  reg  [3:0] d;
  reg  [3:0] model;
  wire [3:0] q;
  wire       vote;
  integer    notified;

  chain #(
      .N(4)
  ) dut (
      .q(q),
      .vote(vote),
      .d(d),
      .si(si),
      .se(se),
      .ck(ck)
  );

  initial $sdf_annotate("gl_scan_chain.sdf", dut);

  initial begin
    ck = 1'b1;
    forever #5 ck = ~ck;
  end

  always @(posedge ck) if ($time > 0) model <= se ? {model[2:0], si} : d;

  always @(negedge ck)
    $display("[%0.3f] q=%b vote=%b model=%b same=%b", $realtime, q, vote, model, q === model);
  always @(dut.ff[0].notifier or dut.ff[1].notifier or dut.ff[2].notifier or dut.ff[3].notifier)
    begin
    notified = notified + 1;
    $display("[%0.3f] notifiers %b%b%b%b", $realtime, dut.ff[3].notifier, dut.ff[2].notifier,
             dut.ff[1].notifier, dut.ff[0].notifier);
  end

  initial begin
    notified = 0;
    se = 1'b1;
    si = 1'b1;
    d  = 4'b0000;
    // Shift in 1, 0, 1, 1 on the edges at 10, 20, 30 and 40.
    #12 si = 1'b0;
    #10 si = 1'b1;
    #20 se = 1'b0;
    d = 4'b0110;
    #1 $inspect("tb.dut.ff[2]");
    $inspect("tb.dut.v[0].m");
    $inspect("tb.dut.v[1].m");
    // Capture at 50, then shift again; the scan input changes 0.2 before
    // the edge at 60.
    #9 se = 1'b1;
    #7.8 si = 1'b0;
    #32.2 $display("[%0.3f] notified=%0d", $realtime, notified);
    $finish;
  end
endmodule
