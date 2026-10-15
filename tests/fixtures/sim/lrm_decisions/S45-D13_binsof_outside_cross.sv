// Decision S45-D13 (negative): binsof naming a coverpoint that the cross
// does not cross is an error. Compilation fails.
//
// IEEE 1800-2009 19.6 (SystemVerilog-1800-2009.txt L33093-33095): "The
// binsof construct yields the bins of its expression, which can be either a
// coverage point (explicitly defined or implicitly defined for a single
// variable) or a coverage point bin." L33051-33054: "A cross coverage bin
// associates a name and a count with a set of cross products." A coverpoint
// outside the cross has no coordinate in its products; the text does not say
// what such a condition selects, so llg rejects it.
module tb;
  bit [1:0] a, b, d;
  covergroup g;
    ca: coverpoint a;
    cb: coverpoint b;
    cd: coverpoint d;
    ab: cross ca, cb { bins sel = binsof(cd) intersect {0}; }
  endgroup
  g c = new;
  initial begin
    a = 0; b = 0; d = 0; c.sample();
    $display("%0.2f", c.ab.get_inst_coverage());
    $finish;
  end
endmodule
