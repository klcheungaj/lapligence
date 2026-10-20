// Decision S45-D14: a cross counts a sample only through bins its crossed
// coverpoints hold in that sample: a crossed coverpoint whose iff is false,
// or whose value has X or Z bits, gives the cross nothing to count. A
// stopped crossed coverpoint counts nothing itself, but the cross still
// counts through the bins its value falls in.
//
// IEEE 1800-2009 19.5 (SystemVerilog-1800-2009.txt L32384-32385): "If the
// guard expression evaluates to false at a sampling point, the coverage
// point is ignored." 19.6 L33051-33054: the count of a cross bin "is
// incremented every time any of the cross products match, i.e., every
// coverage point in the cross matches its corresponding bin in the cross
// product." 19.8 Table 19-5 L33565: stop() "Stops collecting coverage". The
// text does not say how an ignored or stopped coverpoint affects a cross;
// llg follows industry-standard behaviour. option.per_instance = 1 keeps the
// instance coverage (19.7 Table 19-1) portable.
module tb;
  logic [1:0] a;
  bit [1:0] b;
  bit en;
  covergroup g;
    option.per_instance = 1;
    ca: coverpoint a;
    cb: coverpoint b iff (en);
    ab: cross ca, cb;
  endgroup
  g c = new;
  initial begin
    a = 1; b = 1; en = 0; c.sample();
    $display("cb disabled: %0.2f", c.ab.get_inst_coverage());
    c.cb.stop();
    a = 2; b = 2; en = 1; c.sample();
    $display("cb stopped: %0.2f", c.ab.get_inst_coverage());
    c.cb.start();
    a = 2'bx1; b = 3; c.sample();
    $display("a unknown: %0.2f", c.ab.get_inst_coverage());
    a = 0; b = 0; c.sample();
    $display("both: %0.2f", c.ab.get_inst_coverage());
    $finish;
  end
endmodule
