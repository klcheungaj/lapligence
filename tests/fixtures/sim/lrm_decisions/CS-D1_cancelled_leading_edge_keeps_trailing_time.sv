// Decision CS-D1: a module path transition is timed when it is scheduled,
// from the value the destination was last scheduled to. When that earlier
// transition is then cancelled (the new one is due no later) or rejected as
// a pulse (the new one follows it by less than the reject limit) and the new
// value still differs from the destination's value, the new transition
// stays where it was scheduled; it is not timed again as a transition from
// the destination's present value.
//
// IEEE 1800-2009 30.7 (SystemVerilog-1800-2009.txt L50256-50257): "Two
// consecutive scheduled transitions closer together in time than the module
// path delay are deemed a pulse." 30.7 L50268-50269: "Pulses less than the
// reject limit are rejected, and no pulse emerges." 30.7.4.2 L50483-50485:
// "Under normal operation, if the schedule for a trailing pulse edge is
// earlier than the schedule for a leading pulse edge, then the leading edge
// is cancelled. No transition takes place when the initial and final states
// of the pulse are the same". 30.5.2 Table 30-3 L50112-50133 gives the
// delays of the x transitions used below. The text cancels the leading edge
// and does not say when the destination then takes a final state that
// differs from the initial one; keeping the trailing edge's own schedule is
// the industry-standard behaviour.
`timescale 1ns / 1ns
module buf_cell (
    output y,
    input  a
);
  buf (y, a);
  specify
    (a => y) = (4, 9);
  endspecify
endmodule

module tri_cell (
    output y,
    input  a,
    input  en
);
  bufif1 (y, a, en);
  specify
    // 0->1, 1->0, 0->z, z->1, 1->z, z->0
    (a => y) = (4, 9, 3, 5, 7, 6);
    (en => y) = (4, 9, 3, 5, 7, 6);
  endspecify
endmodule

module tb;
  reg a, b, c, d, e, f, en_e, en_f;
  wire ya, yb, yc, yd, ye, yf;
  buf_cell ua (ya, a);
  buf_cell ub (yb, b);
  buf_cell uc (yc, c);
  buf_cell ud (yd, d);
  tri_cell ue (ye, e, en_e);
  tri_cell uf (yf, f, en_f);
  initial begin
    a = 1;
    b = 0;
    c = 1;
    d = 0;
    e = 0;
    f = 1;
    en_e = 1;
    en_f = 1;
    #50 $display("settled %b%b%b%b%b%b", ya, yb, yc, yd, ye, yf);
    #50;
    a = 0;  // 1->0 due 109
    b = 1;  // 0->1 due 104
    c = 0;  // 1->0 due 109
    d = 1;  // 0->1 due 104
    e = 1;  // 0->1 due 104
    f = 0;  // 1->0 due 109
    #2;
    a = 1'bx;  // 0->x in 4: due 106, before 109: the fall is cancelled
    b = 1'bx;  // 1->x in 9: due 111, 7 after 104: the rise is rejected
    en_e = 0;  // 1->z in 7: due 109, 5 after 104: the rise is rejected
    en_f = 0;  // 0->z in 3: due 105, before 109: the fall is cancelled
    #1;
    c = 1'bx;  // 0->x in 4: due 107, before 109: the fall is cancelled
    #3;
    c = 0;  // x->0 in 9: due 115, 8 after 107: the x is rejected
    #1;
    d = 1'bx;  // the rise matured at 104: 1->x in 9, due 116
    #100 $finish;
  end
  always @(ya) if ($time > 50) $display("ya=%b at %0t", ya, $time);
  always @(yb) if ($time > 50) $display("yb=%b at %0t", yb, $time);
  always @(yc) if ($time > 50) $display("yc=%b at %0t", yc, $time);
  always @(yd) if ($time > 50) $display("yd=%b at %0t", yd, $time);
  always @(ye) if ($time > 50) $display("ye=%b at %0t", ye, $time);
  always @(yf) if ($time > 50) $display("yf=%b at %0t", yf, $time);
endmodule
