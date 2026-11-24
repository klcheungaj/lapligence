// FIN-002 gate-level scenario 2: pad cells with tri-state drivers on one
// bus under a pull-up, wired nets, continuous assignments with strengths,
// tri0/tri1 nets, a driver with an unknown enable and a sequential UDP that
// captures the resolved bus on a behavioural clock. A VPI application
// (gl_strength_bus_vpi.c) reads the strength values of the same nets.
// Every expected line is derived in readme.md.
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

module PAD (
    inout  pad,
    input  oe,
    input  out,
    output in
);
  bufif1 (strong1, strong0) drv (pad, out, oe);
  buf rcv (in, pad);
endmodule

module tb;
  reg        clk;
  reg  [2:0] oe;
  reg  [2:0] out;
  reg        ex;
  integer    step;

  tri        bus;
  wire [2:0] in;
  wire       mixed;
  wand       all;
  wor        any;
  tri0       low;
  tri1       high;
  wire       maybe;
  wire       q;
  supply1    vdd;
  supply0    gnd;
  wire       rail;

  PAD p0 (
      .pad(bus),
      .oe (oe[0]),
      .out(out[0]),
      .in (in[0])
  );
  PAD p1 (
      .pad(bus),
      .oe (oe[1]),
      .out(out[1]),
      .in (in[1])
  );
  PAD p2 (
      .pad(bus),
      .oe (oe[2]),
      .out(out[2]),
      .in (in[2])
  );
  pullup (pull1) pu (bus);

  // A pull driver beats a weak one; equal strengths conflict.
  assign (weak1, weak0) mixed = out[0];
  assign (pull1, pull0) mixed = out[1];
  assign all = out[0];
  assign all = out[1];
  assign any = out[0];
  assign any = out[1];
  bufif1 d_low (low, out[2], oe[2]);
  bufif0 d_high (high, out[2], oe[2]);
  // An unknown enable gives a value between the data and high impedance.
  bufif1 d_maybe (maybe, out[0], ex);
  pulldown (weak0) pd (maybe);
  // Supplies beat strong drivers.
  assign (strong1, strong0) rail = out[0];
  buf (supply1, supply0) (rail, out[1]);

  udp_capture cap (q, bus, clk);

  initial begin
    clk = 1'b1;
    forever #5 clk = ~clk;
  end

  // {ex, oe, out} per step.
  reg [6:0] rom[0:9];
  initial begin
    rom[0] = 7'b0_000_000;
    rom[1] = 7'b0_001_000;
    rom[2] = 7'b0_011_010;
    rom[3] = 7'b0_011_011;
    rom[4] = 7'b1_100_101;
    rom[5] = 7'bx_000_001;
    rom[6] = 7'bx_000_000;
    rom[7] = 7'b0_x00_100;
    rom[8] = 7'b0_x00_000;
    rom[9] = 7'b0_111_x11;
    step   = 0;
    {ex, oe, out} = rom[0];
  end

  always @(negedge clk) begin
    $display("[%0t] step %0d bus=%v in=%b mixed=%v all=%v any=%v low=%v high=%v maybe=%v rail=%v q=%b",
             $time, step, bus, in, mixed, all, any, low, high, maybe, rail, q);
    $strength("tb.bus");
    $strength("tb.mixed");
    $strength("tb.maybe");
    if (step == 9) $finish;
    step = step + 1;
    {ex, oe, out} = rom[step];
  end
endmodule
