// Decision S47-D5: dump controls act on the dump's state. Before the first
// $dumpvars, $dumpoff/$dumpon/$dumpall/$dumpflush do nothing ($dumplimit and
// $dumpfile are remembered). $dumpon resumes only a dump that $dumpoff
// stopped; otherwise it has no effect. A $dumpoff while off writes no second
// checkpoint, and $dumpall while off writes none. A $dumpoff in the step of
// the first $dumpvars starts the dump in the off state.
//
// IEEE 1800-2009 21.7.1.3 (SystemVerilog-1800-2009.txt L37844-37846): "When
// the $dumpoff task is executed, a checkpoint is made in which every selected
// variable is dumped as an x value. When the $dumpon task is later executed,
// each variable is dumped with its value at that time. In the interval
// between $dumpoff and $dumpon, no value changes are dumped." 21.7.1.6
// Example 2 (L37961): "$dumpon; //no effect the first time through". The
// text does not cover controls before $dumpvars or repeated controls.
// Compare S47-D5.vcd: `#1 $dumpvars`, `#2 $dumpoff`, `#4 $dumpon b0100`;
// nothing at time 0 or 3.
module tb;
  logic [3:0] v;
  initial begin
    v = 4'h1;
    $dumpfile("S47-D5.vcd");
    $dumpoff;
    $dumpall;
    $dumpon;
    #1 $dumpvars(0, tb);
    $dumpon;
    v = 4'h2;
    #1 $dumpoff;
    v = 4'h3;
    #1 $dumpoff;
    $dumpall;
    v = 4'h4;
    #1 $dumpon;
    $strobe("end of step 4: v=%h", v);
    #1 $finish;
  end
endmodule
