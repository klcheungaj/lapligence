// FIN-002 gate-level scenario 6: two bidirectional pad cells (tri-state
// drivers with rise, fall and turn-off delays, a receiver behind a module
// path with pulse limits) share a pulled-up pad net; a capture cell
// (sequential UDP with a notifier and a setup check) samples one receiver
// on a behavioural clock. The stimulus produces a pulse the path shows as
// x, a driver conflict, and a setup violation; a VPI application
// (shared_force_vpi.c) forces and releases the resolved pad net between
// them and counts its changes. The suite also dumps the run as VCD and FST.
// Every expected line is derived in readme.md.
`timescale 1ns / 1ps

primitive udp_capture(q, d, clk, notifier);
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

module IOPAD (
    inout  PAD,
    input  OE,
    input  A,
    output Y
);
  bufif1 #(0.5, 0.6, 0.3) drv (PAD, A, OE);
  buf rcv (Y, PAD);
  specify
    specparam PATHPULSE$PAD$Y = (0.3, 0.8);
    (PAD => Y) = (1.0, 1.0);
  endspecify
endmodule

module CAPX1 (
    output Q,
    input  D,
    input  CK
);
  reg  notifier;
  wire q_int;
  udp_capture u (q_int, D, CK, notifier);
  buf (Q, q_int);
  specify
    (posedge CK => (Q +: D)) = 0.2;
    $setup(D, posedge CK, 0.5, notifier);
  endspecify
endmodule

module tb;
  reg        ck;
  reg  [1:0] oe;
  reg  [1:0] a;
  tri        pad;
  wire [1:0] y;
  wire       q;

  pullup (pull1) pu (pad);
  IOPAD p0 (
      .PAD(pad),
      .OE (oe[0]),
      .A  (a[0]),
      .Y  (y[0])
  );
  IOPAD p1 (
      .PAD(pad),
      .OE (oe[1]),
      .A  (a[1]),
      .Y  (y[1])
  );
  CAPX1 cap (
      .Q (q),
      .D (y[0]),
      .CK(ck)
  );

  initial begin
    ck = 1'b1;
    forever #5 ck = ~ck;
  end

  always @(pad) $display("[%0.3f] pad=%v", $realtime, pad);
  always @(y) $display("[%0.3f] y=%b", $realtime, y);
  always @(q) $display("[%0.3f] q=%b", $realtime, q);
  always @(cap.notifier) $display("[%0.3f] notifier", $realtime);

  initial begin
    oe = 2'b00;
    a  = 2'b00;
    $vwatch("tb.pad");
    #12 oe[0] = 1'b1;
    #10 a[0] = 1'b1;
    // A low pulse of 0.600 on the pad: between the 0.300 reject limit and
    // the 0.800 error limit of the receiver path.
    #10 a[0] = 1'b0;
    #0.7 a[0] = 1'b1;
    // Both pads drive, with opposite values.
    #9.3 oe[1] = 1'b1;
    #4 $vforce("tb.pad", 1);
    $display("[%0.3f] forced: pad=%v vpi=%0d", $realtime, pad, $vget("tb.pad"));
    #11 $vrelease("tb.pad");
    $display("[%0.3f] released: pad=%v", $realtime, pad);
    #7 oe = 2'b00;
    // The receiver changes 0.300 before the clock edge at 80.
    #14.1 oe[1] = 1'b1;
    #13.9 $vreport;
    $finish;
  end
endmodule
