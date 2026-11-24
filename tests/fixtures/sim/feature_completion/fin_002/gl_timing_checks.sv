// FIN-002 gate-level scenario 4: a set/reset flip-flop cell (sequential UDP
// with a notifier) with edge-sensitive paths and the timing checks
// $setuphold (conditioned), $recrem, $recovery, $removal, $width and
// $period. A behavioural testbench produces one violation per phase; each
// toggles the notifier, the flip-flop goes to x and a reset recovers it. A
// last phase shows the condition switching a check off. Every expected
// line is derived in readme.md.
`timescale 1ns / 1ps

primitive udp_dffsr(q, d, clk, sn, rn, notifier);
  output q;
  reg q;
  input d, clk, sn, rn, notifier;
  table
    // d  clk  sn  rn  ntf : q : q+
    0 (01) 1 1 ? : ? : 0;
    1 (01) 1 1 ? : ? : 1;
    ? (1?) 1 1 ? : ? : -;
    ? (?0) 1 1 ? : ? : -;
    * ? 1 1 ? : ? : -;
    ? ? ? 0 ? : ? : 0;
    ? ? 0 1 ? : ? : 1;
    ? ? (01) 1 ? : ? : -;
    ? ? 1 (01) ? : ? : -;
    ? ? ? ? * : ? : x;
  endtable
endprimitive

module DFFSRX1 (
    output Q,
    input  D,
    input  CK,
    input  SN,
    input  RN
);
  reg  notifier;
  wire q_int;
  wire chk_en;
  udp_dffsr (q_int, D, CK, SN, RN, notifier);
  buf (Q, q_int);
  and (chk_en, SN, RN);
  specify
    (posedge CK => (Q +: D)) = 0.2;
    (negedge SN => (Q +: 1'b1)) = 0.2;
    (negedge RN => (Q +: 1'b0)) = 0.2;
    $setuphold(posedge CK &&& chk_en, D, 0.5, 0.3, notifier);
    $recrem(posedge RN, posedge CK, 0.6, 0.4, notifier);
    $recovery(posedge SN, posedge CK, 0.6, notifier);
    $removal(posedge SN, posedge CK, 0.4, notifier);
    $width(negedge RN, 1.0, 0, notifier);
    $width(posedge CK, 2.0, 0, notifier);
    $period(posedge CK, 8.0, notifier);
  endspecify
endmodule

module tb;
  reg d, ck, sn, rn;
  wire q;
  integer violations;

  DFFSRX1 dut (
      .Q (q),
      .D (d),
      .CK(ck),
      .SN(sn),
      .RN(rn)
  );

  always @(q) $display("[%0.3f] q=%b", $realtime, q);
  always @(dut.notifier) begin
    violations = violations + 1;
    $display("[%0.3f] notifier", $realtime);
  end

  task reset;
    begin
      rn = 1'b0;
      #2 rn = 1'b1;
    end
  endtask

  initial begin
    violations = 0;
    d = 1'b0;
    ck = 1'b0;
    sn = 1'b1;
    rn = 1'b1;
    #1 reset;
    // A: data 0.3 before the clock (setup limit 0.5).
    #6.7 d = 1'b1;
    #0.3 ck = 1'b1;
    #3 ck = 1'b0;
    #2 reset;
    // B: data 0.2 after the clock (hold limit 0.3).
    #3 ck = 1'b1;
    #0.2 d = 1'b0;
    #2.8 ck = 1'b0;
    #2 reset;
    // C: clock 0.3 after the reset release (recovery limit 0.6).
    #3 reset;
    #0.3 ck = 1'b1;
    #2.7 ck = 1'b0;
    #2 reset;
    // D: reset release 0.2 after the clock (removal limit 0.4).
    #2 rn = 1'b0;
    #3 ck = 1'b1;
    #0.2 rn = 1'b1;
    #2.8 ck = 1'b0;
    #2 reset;
    // E: a clock pulse of 1.0 (width limit 2.0).
    #3 ck = 1'b1;
    #1 ck = 1'b0;
    #2 reset;
    // F: clock edges 6.0 apart (period limit 8.0).
    #3 ck = 1'b1;
    #2.5 ck = 1'b0;
    #3.5 ck = 1'b1;
    #3 ck = 1'b0;
    #2 reset;
    // G: a reset pulse of 0.6 (width limit 1.0).
    #5 rn = 1'b0;
    #0.6 rn = 1'b1;
    #2.4 reset;
    // H: clock 0.4 after the set release (recovery limit 0.6).
    #3 sn = 1'b0;
    #2 sn = 1'b1;
    #0.4 ck = 1'b1;
    #3 ck = 1'b0;
    #1.6 reset;
    // I: data 0.1 before the clock while reset holds the check off.
    #3 rn = 1'b0;
    #4.9 d = 1'b1;
    #0.1 ck = 1'b1;
    #3 rn = 1'b1;
    #0.5 ck = 1'b0;
    #5 $display("[%0.3f] violations=%0d q=%b", $realtime, violations, q);
    $finish;
  end
endmodule
