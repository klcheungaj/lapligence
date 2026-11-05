// IEEE 1800-2009 6.3.2 L4553-4555 (SystemVerilog-1800-2009.txt): "The language
// includes strength information in addition to the basic value information
// for nets." "The additional strength information associated with bits of a
// net is not considered part of the data type."
// 9.4.2 L11767-11769: "The value changes on nets and variables can be used as
// events to trigger the execution of a statement."
// 21.2.3 L36516-36518: "each time a variable or an expression in the argument
// list changes value ... the entire argument list is displayed".
// The text does not say whether a strength change is a value change.
// Decision: a net's strength belongs to its value for implicit events. An
// event control on a net or a constant projection of nets, an implicit event
// list (@*, always_comb, always_latch), a wait and a $monitor argument naming
// a net occur on a strength-only change of the bits they name. Edges, derived
// nets, operators in an event expression or $monitor argument do not.
`timescale 1ns/1ns

module child(i);
  input i;
endmodule

module tb;
  reg a, b, g, en, c, e0;
  wire w, y, x2;
  wire [1:0] v;
  assign (strong1, strong0) w = a;
  assign (pull1, pull0) w = b;
  // Edges and operators watch a net of their own.
  wire e;
  assign (strong1, strong0) e = a;
  assign (pull1, pull0) e = b;
  assign y = w;
  buf (x2, w);
  child ch(w);
  bufif1 (strong0, strong1) v0 (v[0], 1'b1, e0);
  bufif1 (pull0, pull1) v1 (v[0], 1'b1, 1'b1);
  buf v2 (v[1], c);

  // Other kinds of net with strength-only changes.
  tri0 t0;
  assign t0 = g;
  wire eo;
  bufif1 b3 (eo, g, en);
  wand wa;
  assign (strong1, strong0) wa = a;
  assign (pull1, pull0) wa = b;
  wire n[1:0];
  assign (strong1, strong0) n[1] = a;
  assign (pull1, pull0) n[1] = b;
  wire p;
  assign (pull1, pull0) p = b;
  // Strong drivers alone give one strength per value.
  wire u;
  assign u = a;
  assign u = b;

  integer calls = 0;
  function automatic bit side(input logic value);
    calls = calls + 1;
    return value === 1'b0;
  endfunction

  localparam int N = 25;
  int seen[N];
  string names[N] = '{"@(w)", "@*", "comb", "latch", "or", "iff", "posedge", "negedge", "and", "@(y)", "@(x2)", "@(v)", "@(v[0])", "@(v[1])", "@(concat)", "@(t0)", "@(eo)", "@(wa)", "@(n[1])", "@(p)", "@(u)", "@(ch.i)", "@* v[1]", "intra", "wait"};

  always @(w) seen[0]++;
  always @* begin : star
    reg r;
    r = w;
    seen[1]++;
  end
  // A procedure of these kinds owns what it writes, so an event counts its runs.
  event comb_ran, latch_ran;
  always @(comb_ran) seen[2]++;
  always @(latch_ran) seen[3]++;
  always_comb begin : comb
    reg r;
    r = w;
    -> comb_ran;
  end
  always_latch begin : latch
    reg r;
    r = w;
    -> latch_ran;
  end
  always @(w or c) seen[4]++;
  always @(w iff e0 === 1'b1) seen[5]++;
  always @(posedge e) seen[6]++;
  always @(negedge e) seen[7]++;
  always @(e & 1'b1) seen[8]++;
  always @(y) seen[9]++;
  always @(x2) seen[10]++;
  always @(v) seen[11]++;
  always @(v[0]) seen[12]++;
  always @(v[1]) seen[13]++;
  always @({v[1], w}) seen[14]++;
  always @(t0) seen[15]++;
  always @(eo) seen[16]++;
  always @(wa) seen[17]++;
  always @(n[1]) seen[18]++;
  always @(p) seen[19]++;
  always @(u) seen[20]++;
  always @(ch.i) seen[21]++;
  always @* begin : star_one
    reg r;
    r = v[1];
    seen[22]++;
  end

  initial begin : wait_side
    wait (side(w));
    seen[24]++;
  end
  reg sampled;
  initial begin
    #1;
    forever begin
      sampled = @(w) a;
      seen[23]++;
    end
  end

  // One line per step, in a fixed order: which observers ran, and the values.
  task automatic report;
    string line;
    line = "";
    for (int i = 0; i < N; i++) begin
      if (seen[i] != 0) line = {line, " ", names[i]};
      seen[i] = 0;
    end
    $display("%0d:%s", $time, line);
    $display("  w=%b %v v=%b %v %v t0=%v eo=%v wa=%v n=%v p=%v u=%v y=%v x2=%v i=%v calls=%0d",
             w, w, v, v[1], v[0], t0, eo, wa, n[1], p, u, y, x2, ch.i, calls);
  endtask

  initial begin
    a = 1'bz; b = 1'bz; g = 1'bz; en = 0; c = 0; e0 = 0;
    #1 report;
    #1 a = 1; g = 1;
    #1 report;
    #1 b = 1;
    #1 report;
    #1 a = 1'bz;
    #1 report;
    #1 a = 1;
    #1 report;
    #1 e0 = 1;
    #1 report;
    #1 e0 = 0;
    #1 report;
    #1 c = 1;
    #1 report;
    #1 g = 1'bz;
    #1 report;
    #1 g = 0;
    #1 report;
    #1 en = 1'bx;
    #1 report;
    #1 g = 1;
    #1 report;
    #1 a = 1'bz;
    #1 report;
    #1 force p = 1'b1;
    #1 report;
    #1 release p;
    #1 report;
    $finish;
  end

  // Reports print at the end of a time slot, so no line order depends on the
  // order of processes.
  initial begin
    #2 $monitor("%0d monitor %b", $time, w);
    #10 $monitor("%0d monitor v0 %b", $time, v[0]);
    #4 $monitor("%0d monitor v1 %b", $time, v[1]);
    #4 $monitor("%0d monitor and %b", $time, w & 1'b1);
  end
endmodule
