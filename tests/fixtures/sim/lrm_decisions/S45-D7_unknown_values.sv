// Decision S45-D7: X and Z bits. A bins value with X or Z bits (outside
// wildcard bins) does not participate in the bin's values, and a sampled
// value with X or Z bits counts in no value bin. In a transition, such a
// sample matches no transition item: it ends a consecutive sequence, and may
// fall in the gap of a goto or nonconsecutive repetition.
//
// IEEE 1800-2009 19.5.3 (SystemVerilog-1800-2009.txt L32767-32769): "By
// default, a value or transition bin definition can specify 4-state values.
// When a bin definition includes an X or Z, it indicates that the bin count
// should only be incremented when the sampled value has an X or Z in the
// same bit positions". 19.5.6 L32868-32874, L32876-32878: an implementation
// shall issue a warning "If b yields a value with any x or z bits", and "If
// an element of a bins open_range_list is a singleton value b, the element
// shall not participate in the bins values." The two clauses conflict; llg
// follows 19.5.6. 19.5.1 L32596-32599 (goto) and L32618-32622
// (nonconsecutive) let "any number of sample points" occur between
// occurrences.
module tb;
  logic [3:0] v;
  covergroup g;
    xval: coverpoint v { bins x = {4'b1x00}; bins two = {2}; }
    next: coverpoint v { bins t = (1 => 2); }
    later: coverpoint v { bins t = (1 => 2 [-> 1]); }
  endgroup
  g c = new;
  initial begin
    v = 4'b1x00;
    c.sample();
    $display("after 1x00: %0.2f", c.xval.get_inst_coverage());
    v = 4'd1;
    c.sample();
    v = 4'bxx01;
    c.sample();
    v = 4'd2;
    c.sample();
    $display("after 1 x 2: %0.2f %0.2f %0.2f", c.xval.get_inst_coverage(),
             c.next.get_inst_coverage(), c.later.get_inst_coverage());
    $finish;
  end
endmodule
