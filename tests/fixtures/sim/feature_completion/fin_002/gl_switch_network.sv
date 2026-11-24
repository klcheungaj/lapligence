// FIN-002 gate-level scenario 3: bidirectional switch networks. A pass-gate
// multiplexer of tranif0/tranif1, a chain of resistive switches that
// reduces a strong driver stage by stage against a weak driver at its far
// end, two tri-state drivers joined by a tran, a supply reached through a
// switch and a switch with an unknown control, all stepped by a behavioural
// clock and captured by a sequential UDP. Every expected line is derived in
// readme.md.
`timescale 1ns / 1ns

primitive udp_capture(q, d, clk);
  output q;
  reg q;
  input d, clk;
  table
    // d clk : q : q+
    0 (01) : ? : 0;
    1 (01) : ? : 1;
    x (01) : ? : x;
    ? (1?) : ? : -;
    ? (?0) : ? : -;
    * ? : ? : -;
  endtable
endprimitive

module PASSMUX (
    inout y,
    inout a,
    inout b,
    input s
);
  tranif0 (a, y, s);
  tranif1 (b, y, s);
endmodule

module tb;
  reg        clk;
  reg  [3:0] v;
  reg        s;
  reg        en_l;
  reg        en_r;
  reg        cx;
  integer    step;

  wire a, b, y;
  wire r0, r1, r2, r3;
  wire left, right;
  wire pwr;
  wire tx;
  wire q;
  supply1 vdd;

  assign a = v[0];
  assign (pull1, pull0) b = v[1];
  PASSMUX m (
      .y(y),
      .a(a),
      .b(b),
      .s(s)
  );

  // Strong at r0; each resistive switch passes one level less.
  assign r0 = v[2];
  rtran (r0, r1);
  rtran (r1, r2);
  rtran (r2, r3);
  assign (weak1, weak0) r3 = v[3];

  bufif1 dl (left, v[0], en_l);
  bufif1 (pull1, pull0) dr (right, v[1], en_r);
  tran (left, right);

  tranif1 (vdd, pwr, s);
  pulldown (pwr);

  tranif1 (a, tx, cx);
  pulldown (weak0) (tx);

  udp_capture cap (q, y, clk);

  initial begin
    clk = 1'b1;
    forever #5 clk = ~clk;
  end

  // {cx, en_l, en_r, s, v} per step.
  reg [7:0] rom[0:9];
  initial begin
    rom[0] = 8'b0_00_0_0000;
    rom[1] = 8'b0_10_0_0101;
    rom[2] = 8'b1_01_1_0110;
    rom[3] = 8'b1_11_1_1001;
    rom[4] = 8'b0_11_0_0011;
    rom[5] = 8'bx_10_x_1101;
    rom[6] = 8'bx_01_0_0010;
    rom[7] = 8'b1_x1_1_1011;
    rom[8] = 8'b0_00_1_x1x0;
    rom[9] = 8'b1_11_0_0100;
    step = 0;
    {cx, en_l, en_r, s, v} = rom[0];
  end

  always @(negedge clk) begin
    $display("[%0t] step %0d y=%v r=%v,%v,%v,%v left=%v right=%v pwr=%v tx=%v q=%b", $time, step, y,
             r0, r1, r2, r3, left, right, pwr, tx, q);
    if (step == 9) $finish;
    step = step + 1;
    {cx, en_l, en_r, s, v} = rom[step];
  end
endmodule
